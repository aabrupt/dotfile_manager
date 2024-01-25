use clap::Parser;
use configparser::ini::Ini;
use pgp::types::SecretKeyTrait;
use pgp::{Deserializable, Message, SignedSecretKey};
use rand::RngCore;
use std::io::{prelude::*, BufWriter};
use std::ops::Deref;
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader},
    path::PathBuf,
};

use cli::{Cli, FileType, PrimaryAction, SyncDirection};
use error::ApplicationError;

mod cli;
mod error;

pub(crate) fn main() {
    if let Err(err) = inner_main() {
        eprintln!("{}", err)
    }
}

pub(crate) fn inner_main() -> Result<(), ApplicationError> {
    let options = Cli::parse();

    let home_dir =
        PathBuf::from(std::env::var("HOME").map_err(|_| ApplicationError::UndedfinedHomeVariable)?);
    let configuration_directories = [
        home_dir.join(".dotconf"),
        home_dir.join(".dotfiles.conf"),
        home_dir.join(".config").join("dotconf"),
        home_dir.join(".config").join("dotfiles.conf"),
    ];
    let mut config = Ini::new();
    for dir in configuration_directories {
        if !dir.exists() {
            continue;
        }

        config
            .load(&dir)
            .map_err(|_| ApplicationError::ConfigFileReadError(dir.clone()))?;
        break;
    }

    let dotfiles_dir = match config.get("options", "source_control_folder") {
        Some(dotfiles_dir) => PathBuf::from(
            shellexpand::full(&dotfiles_dir)
                .map_err(|err| ApplicationError::ErrorExpandingVariable(err))?
                .deref(),
        ),
        None => PathBuf::from(std::env::var("HOME").unwrap()).join(".dotfiles"),
    };

    /* Get the configuration file containing simple line by line paths to directories and
     * folders to be tracked and symlinked */
    let symlinks_cfg_path = dotfiles_dir.join("cfg").join("symlinks");
    let secrets_cfg_path = dotfiles_dir.join("cfg").join("secrets");

    match &options.primary_action {
        PrimaryAction::Sync => {
            let sync_direction = &options.sync_direction;
            match File::open(&symlinks_cfg_path) {
                Ok(symlinks_cfg_file) => {
                    let symlink_reader = BufReader::new(symlinks_cfg_file);
                    for line in symlink_reader.lines() {
                        let line = line.as_ref().map_err(|_| {
                            ApplicationError::ConfigFileReadError(symlinks_cfg_path.clone())
                        })?;
                        /* A tracked file contain two locations, one for the symlink and one for the real
                         * file */
                        let file = PathBuf::from(line);
                        let dotfile_path = dotfile_path(dotfiles_dir.join("symlinks"), &file)?;
                        if file.is_symlink() {
                            if dotfile_path.exists() {
                                println!(
                                    "'{}' already tracked",
                                    file.into_os_string().into_string().unwrap()
                                );
                                continue;
                            }
                        }

                        if !dotfile_path.parent().unwrap().exists() {
                            std::fs::create_dir(dotfile_path.parent().unwrap()).map_err(|err| {
                                ApplicationError::CouldNotCreateDirectories(
                                    dotfile_path.parent().unwrap().to_path_buf(),
                                    err,
                                )
                            })?;
                        }

                        match sync_direction {
                            SyncDirection::Dotfiles => {
                                if file.is_symlink() {
                                    return Err(ApplicationError::UntrackedSymlinkedFile(
                                        file.clone(),
                                    ));
                                }
                                if !file.exists() {
                                    eprintln!("{}", ApplicationError::FileNotFound(file));
                                    continue;
                                }
                                std::fs::rename(&file, &dotfile_path).map_err(|err| {
                                    ApplicationError::FailedRenamingFile {
                                        err,
                                        from: file.clone(),
                                        to: dotfile_path.clone(),
                                    }
                                })?;
                            }
                            SyncDirection::Filesystem => {
                                if file.exists() {
                                    let bkp_file = bkp_file(&file)?;
                                    std::fs::rename(&file, &bkp_file).map_err(|err| {
                                        ApplicationError::FailedRenamingFile {
                                            err,
                                            from: file.clone(),
                                            to: bkp_file,
                                        }
                                    })?;
                                } else {
                                    continue;
                                }
                            }
                        }

                        std::os::unix::fs::symlink(&dotfile_path, &file).map_err(|err| {
                            ApplicationError::FailedRenamingFile {
                                err,
                                from: dotfile_path.to_path_buf(),
                                to: file.clone(),
                            }
                        })?;
                    }
                }
                Err(err) => eprintln!("{err}"),
            };

            let maybe_key = key_or_cfg(&options.secret_key, config);

            match File::open(&secrets_cfg_path)
                .map_err(|err| ApplicationError::CouldNotOpenFile(secrets_cfg_path.clone(), err))
            {
                Ok(secrets_cfg_file) => {
                    match maybe_key {
                        Ok(key_path) => {
                            let secrets_reader = BufReader::new(&secrets_cfg_file);
                            for line in secrets_reader.lines() {
                                let line = line.as_ref().map_err(|_| {
                                    ApplicationError::ConfigFileReadError(secrets_cfg_path.clone())
                                })?;

                                let file_path = PathBuf::from(line);

                                let dotfile_path =
                                    dotfile_path(dotfiles_dir.join("secrets"), &file_path)?;

                                let key_file = File::open(&key_path).map_err(|err| {
                                    ApplicationError::CouldNotOpenFile(key_path.clone(), err)
                                })?;
                                let key = SignedSecretKey::from_armor_single(key_file)
                                    .map_err(|err| {
                                        ApplicationError::FailedReadingKey(key_path.clone(), err)
                                    })?
                                    .0;

                                match sync_direction {
                                    SyncDirection::Dotfiles => {
                                        let message = Message::new_literal(
                                            "none",
                                            fs::read_to_string(&file_path)
                                                .map_err(|err| {
                                                    ApplicationError::CouldNotOpenFile(
                                                        file_path.clone(),
                                                        err,
                                                    )
                                                })?
                                                .as_str(),
                                        );
                                        let encrypted_content = message
                                            .encrypt_to_keys(
                                                &mut rand::thread_rng(),
                                                pgp::crypto::sym::SymmetricKeyAlgorithm::AES128,
                                                &[&key.public_key()],
                                            )
                                            .map_err(|err| {
                                                ApplicationError::FailedEncryptingContent(
                                                    file_path.clone(),
                                                    err,
                                                )
                                            })?;
                                        let mut dotfile = OpenOptions::new()
                                            .create(true)
                                            .write(true)
                                            .open(&dotfile_path)
                                            .map_err(|err| {
                                                ApplicationError::CouldNotOpenFile(
                                                    dotfile_path.clone(),
                                                    err,
                                                )
                                            })?;

                                        encrypted_content
                                            .to_armored_writer(&mut dotfile, None)
                                            .map_err(|err| {
                                                ApplicationError::PGPWriterError(
                                                    dotfile_path.clone(),
                                                    err,
                                                )
                                            })?;
                                    }
                                    SyncDirection::Filesystem => {
                                        let dotfile = File::open(&dotfile_path).map_err(|err| {
                                            ApplicationError::CouldNotOpenFile(
                                                dotfile_path.clone(),
                                                err,
                                            )
                                        })?;
                                        let (message, _) = Message::from_armor_single(dotfile)
                                            .map_err(|err| {
                                                ApplicationError::PGPMessageReadError(
                                                    dotfile_path.clone(),
                                                    err,
                                                )
                                            })?;
                                        /* let password = rpassword::prompt_password(
                                            "Please input password to unlock the key\n> ",
                                        )
                                        .map_err(|_| ApplicationError::PasswordRequired)?; */

                                        let (decryptor, _) = message
                                            .decrypt(|| String::new(), &[&key])
                                            .map_err(|_| {
                                                ApplicationError::FailedDecryptingContent(
                                                    dotfile_path.clone(),
                                                )
                                            })?;

                                        for msg in decryptor {
                                            let bytes = msg.map_err(|err| ApplicationError::FailedDecryptingMessageInContent(err))?
                                                    .get_content().map_err(|err| ApplicationError::ErrorReadingContentInMessage(err))?
                                                    .ok_or(ApplicationError::NoContentInPGPMessage)?;

                                            let clear = String::from_utf8(bytes).map_err(|_| {
                                                ApplicationError::MessageNotUTF8Encoded
                                            })?;
                                            if clear.len() > 0 {
                                                let bkp_file = bkp_file(&file_path)?;
                                                if file_path.exists() {
                                                    fs::rename(&file_path, &bkp_file).map_err(
                                                        |err| {
                                                            ApplicationError::FailedRenamingFile {
                                                                err,
                                                                from: file_path.clone(),
                                                                to: bkp_file.clone(),
                                                            }
                                                        },
                                                    )?;
                                                }
                                                fs::write(&file_path, &clear).map_err(|err| {
                                                    ApplicationError::FailedWritingToFile(
                                                        file_path.clone(),
                                                        err,
                                                    )
                                                })?;
                                                break;
                                            }
                                        }
                                    }
                                };
                            }
                        }
                        Err(err) => eprintln!("{err}"),
                    }
                }
                Err(err) => eprintln!("{err}"),
            };
        }
        PrimaryAction::Add => {
            // TODO: Implement fix for edge case where file already is added to configuration
            let cfg_file_path = match &options.file_type {
                FileType::Config => symlinks_cfg_path,
                FileType::Secret => secrets_cfg_path,
            };
            let file = &options.file.ok_or(ApplicationError::FileInputRequired)?;
            let abs_path = expand_variables_in_path(file)?;
            let abs_path_str = abs_path
                .to_str()
                .ok_or(ApplicationError::PathConversionError(abs_path.clone()))?;
            let cfg_file_parent = cfg_file_path.parent().ok_or(ApplicationError::FileInRoot(cfg_file_path.clone()))?;
            if cfg_file_parent.exists() {
                fs::create_dir_all(cfg_file_parent).map_err(|err| ApplicationError::CouldNotCreateDirectories(cfg_file_parent.to_path_buf(), err))?;
            }
            {
                let cfg_file = OpenOptions::new()
                    .create(true)
                    .read(true)
                    .append(true)
                    .open(&cfg_file_path)
                    .map_err(|err| {
                        ApplicationError::CouldNotOpenFile(cfg_file_path.clone(), err)
                    })?;
                let reader = BufReader::new(&cfg_file);
                for line in reader.lines() {
                    let line = line
                        .as_ref()
                        .map_err(|_| ApplicationError::ErrorReadingFile(cfg_file_path.clone()))?;
                    if line.contains(&abs_path_str) {
                        println!("'{}' is already tracked", abs_path_str);
                        return Ok(());
                    }
                }
            }
            {
                let mut cfg_file = OpenOptions::new()
                    .write(true)
                    .append(true)
                    .open(&cfg_file_path)
                    .map_err(|err| {
                        ApplicationError::CouldNotOpenFile(cfg_file_path.clone(), err)
                    })?;
                cfg_file
                    .write(format!("{}\n", abs_path_str).as_bytes())
                    .map_err(|err| {
                        ApplicationError::FailedWritingToFile(cfg_file_path.clone(), err)
                    })?;
                println!(
                    "'{}' has been added to '{}'",
                    abs_path_str,
                    cfg_file_path.file_name().unwrap().to_str().unwrap(),
                );
            }
        }
        PrimaryAction::Remove => {
            let cfg_file_path = match &options.file_type {
                FileType::Config => symlinks_cfg_path,
                FileType::Secret => secrets_cfg_path,
            };
            let file = options.file.ok_or(ApplicationError::FileInputRequired)?;
            let abs_path = expand_variables_in_path(&file)?;
            let out_path = dotfiles_dir.join("cfg.tmp");
            {
                let cfg_file = File::open(&cfg_file_path).map_err(|err| {
                    ApplicationError::CouldNotOpenFile(cfg_file_path.clone(), err)
                })?;
                let cfg_out_file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(&out_path)
                    .map_err(|err| ApplicationError::CouldNotOpenFile(out_path.clone(), err))?;
                let cfg_file_reader = BufReader::new(&cfg_file);
                let mut cfg_file_writer = BufWriter::new(&cfg_out_file);
                for line in cfg_file_reader.lines() {
                    let line = line
                        .as_ref()
                        .map_err(|_| ApplicationError::ErrorReadingFile(cfg_file_path.clone()))?;
                    if !line.contains(
                        abs_path
                            .to_str()
                            .ok_or(ApplicationError::PathConversionError(abs_path.clone()))?,
                    ) {
                        writeln!(cfg_file_writer, "{}", line).map_err(|err| {
                            ApplicationError::FailedWritingToFile(cfg_file_path.clone(), err)
                        })?;
                    }
                }
            }
            fs::rename(&out_path, &cfg_file_path).map_err(|err| {
                ApplicationError::FailedRenamingFile {
                    err,
                    from: out_path,
                    to: cfg_file_path.to_path_buf(),
                }
            })?;

            println!(
                "'{}' has been removed from '{}'",
                abs_path.to_str().unwrap(),
                cfg_file_path.file_name().unwrap().to_str().unwrap(),
            );
        }
        PrimaryAction::CreateKey => {
            let key_path = key_or_cfg(&options.secret_key, config)?;

            let key_parent = key_path.parent().ok_or(ApplicationError::FileInRoot(key_path.clone()))?;

            fs::create_dir_all(key_parent).map_err(|err| ApplicationError::CouldNotCreateDirectories(key_parent.to_path_buf(), err))?;

            let mut key_file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&key_path)
                .map_err(|err| ApplicationError::CouldNotOpenFile(key_path.clone(), err))?;

            let key_params = pgp::SecretKeyParamsBuilder::default()
                .key_type(pgp::KeyType::Rsa(2048))
                .primary_user_id("".to_string())
                .can_create_certificates(false)
                .can_sign(true)
                .preferred_symmetric_algorithms(
                    vec![pgp::crypto::sym::SymmetricKeyAlgorithm::AES256].into(),
                )
                .preferred_hash_algorithms(vec![pgp::crypto::hash::HashAlgorithm::SHA2_256].into())
                .preferred_compression_algorithms(
                    vec![pgp::types::CompressionAlgorithm::ZLIB].into(),
                )
                .build()
                .unwrap();

            /* let password =
            rpassword::prompt_password("Please input a password to sign the PGP key\n> ")
                .map_err(|_| ApplicationError::PasswordRequired)?; */

            let secret_key = key_params
                .generate()
                .map_err(|err| ApplicationError::KeyGenerationFailed(err))?;
            let signed_secret_key = secret_key
                .sign(|| String::new())
                .map_err(|_| ApplicationError::PGPKeySignError(key_path.clone()))?;

            signed_secret_key
                .to_armored_writer(&mut key_file, None)
                .map_err(|err| ApplicationError::PGPWriterError(key_path.clone(), err))?;
        }
    }
    Ok(())
}

fn dotfile_path<'a>(
    mut base_directory: PathBuf,
    file: &'a PathBuf,
) -> Result<PathBuf, ApplicationError> {
    let parent = file
        .parent()
        .ok_or(ApplicationError::FileInRoot(file.clone()))?;
    let parent_name = parent
        .file_name()
        .ok_or(ApplicationError::FileNotFound(parent.to_path_buf()))?;
    if &PathBuf::from(std::env::var("HOME").unwrap()) != parent && !file.is_dir() {
        base_directory.push(parent_name);
    }
    fs::create_dir_all(&base_directory)
        .map_err(|err| ApplicationError::CouldNotCreateDirectories(base_directory.clone(), err))?;
    base_directory.push(
        file.file_name()
            .ok_or(ApplicationError::FileNotFound(file.clone()))?,
    );
    Ok(base_directory)
}

fn key_or_cfg(key: &Option<PathBuf>, config: Ini) -> Result<PathBuf, ApplicationError> {
    match key {
        Some(key) => Ok(key.clone()),
        None => match config.get("options", "secret_key") {
            Some(config_key) => Ok(PathBuf::from(
                shellexpand::full(&config_key)
                    .map_err(|err| ApplicationError::ErrorExpandingVariable(err))?
                    .deref(),
            )),
            None => Err(ApplicationError::SecretKeyRequired),
        },
    }
}

fn bkp_file(file: &PathBuf) -> Result<PathBuf, ApplicationError> {
    let mut new_file = file.clone();
    new_file.set_file_name(format!(
        "{}.bkp-{}",
        file.file_name().unwrap().to_str().unwrap(),
        rand::thread_rng().next_u32()
    ));
    Ok(new_file)
}

fn expand_variables_in_path(file: &PathBuf) -> Result<PathBuf, ApplicationError> {
    Ok(fs::canonicalize(
        shellexpand::full(file.to_str().ok_or(ApplicationError::FileInputRequired)?)
            .map_err(|err| ApplicationError::ErrorExpandingVariable(err))?
            .deref(),
    )
    .map_err(|_| ApplicationError::PathConversionError(file.clone()))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn static_symlinc_dir<'a>(home: &'a str) -> PathBuf {
        [home.into(), ".dotfiles", "symlinks"].iter().collect()
    }

    #[test]
    fn test_dotfile_in_home() {
        let home = std::env::var("HOME").unwrap();
        let dotfile_path = dotfile_path(
            static_symlinc_dir(&home),
            &[&home, ".dotfile"].iter().collect(),
        )
        .unwrap();
        assert_eq!(
            dotfile_path,
            [&home, ".dotfiles", "symlinks", ".dotfile"]
                .iter()
                .collect::<PathBuf>()
        );
    }
    #[test]
    fn test_dotfile_folder() {
        let home = std::env::var("HOME").unwrap();
        let dotfile_path = dotfile_path(
            static_symlinc_dir(&home),
            &[&home, ".config", "dotfolder"].iter().collect()
        )
        .unwrap();
        assert_eq!(
            dotfile_path,
            [&home, ".dotfiles", "symlinks", "dotfolder"]
                .iter()
                .collect::<PathBuf>()
        );
    }
    #[test]
    fn test_dotfile_outside_home() {
        let home = std::env::var("HOME").unwrap();
        let dotfile_path = dotfile_path(
            static_symlinc_dir(&home),
            &[&home, ".config", "dotfile"].iter().collect()
        )
        .unwrap();
        assert_eq!(
            dotfile_path,
            [&home, ".dotfiles", "symlinks", ".config", "dotfile"]
                .iter()
                .collect::<PathBuf>()
        );
    }
    #[test]
    fn test_bkp_file() {
        let file = ["file.txt"].iter().collect::<PathBuf>();
        let bkp_file = bkp_file(&file).unwrap();

        assert_ne!(file, bkp_file);
    }
    #[test]
    fn test_expand_tilde() {
        let home = std::env::var("HOME").unwrap();
        let path = PathBuf::from("~");
        let expanded = expand_variables_in_path(&path).unwrap();
        assert_eq!(expanded, PathBuf::from(home));
    }
    #[test]
    fn test_expand_variable() {
        let home = std::env::var("HOME").unwrap();
        let path = PathBuf::from("$HOME");
        let expanded = expand_variables_in_path(&path).unwrap();
        assert_eq!(expanded, PathBuf::from(home));
    }
    #[test]
    #[should_panic]
    fn test_no_key() {
        let config = Ini::new();

        key_or_cfg(&None, config).unwrap();
    }
    #[test]
    fn test_input_key() {
        let config = Ini::new();

        let input_key = key_or_cfg(&Some(PathBuf::from("key")), config).unwrap();
        assert_eq!(input_key, PathBuf::from("key"));
    }
    #[test]
    fn test_cfg_key() {
        let mut config = Ini::new();
        config.set("options", "secret_key", Some("~".to_string()));

        let input_key = key_or_cfg(&None, config).unwrap();
        assert_eq!(input_key, PathBuf::from(std::env::var("HOME").unwrap()));
    }
}
