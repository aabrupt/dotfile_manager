use clap::Parser;
use pgp::ser::Serialize;
use pgp::types::SecretKeyTrait;
use std::io::{prelude::*, BufWriter};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use cli::{Cli, FileType, PrimaryAction, SyncDirection};
use error::ApplicationError;

mod cli;
mod error;

pub(crate) fn main() -> Result<(), ApplicationError> {
    let options = Cli::parse();

    /* Get the systems dotfile directory
     * Default: $HOME/.dotfiles
     * Custom: $DOTFILES_DIR
     */
    let dotfiles_dir = match std::env::var("DOTFILES_DIR") {
        Ok(dotfiles_directory) => PathBuf::from(dotfiles_directory),
        Err(_) => PathBuf::from(std::env::var("HOME").unwrap()).join(".dotfiles"),
    };

    /* Get the configuration file containing simple line by line paths to directories and
     * folders to be tracked and symlinked */
    let symlinks_cfg_path = dotfiles_dir.join("cfg").join("symlinks");
    let secrets_cfg_path = dotfiles_dir.join("cfg").join("secrets");

    match PrimaryAction::from(&options.primary_action) {
        PrimaryAction::Sync => {
            if let Ok(symlinks_cfg_file) = File::open(&symlinks_cfg_path) {
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

                    let _ = std::fs::create_dir(dotfile_path.parent().unwrap());

                    match SyncDirection::from(&options.sync_direction) {
                        SyncDirection::FromFilesystem => {
                            if file.is_symlink() {
                                return Err(ApplicationError::UntrackedSymlinkedFile(file.clone()));
                            }
                            std::fs::rename(&file, &dotfile_path).map_err(|err| {
                                ApplicationError::FailedRenamingFile {
                                    err,
                                    from: file.clone(),
                                    to: dotfile_path.clone(),
                                }
                            })?;
                        }
                        SyncDirection::FromDotfiles => {
                            if file.exists() {
                                let mut new_file = file.clone();
                                new_file.set_file_name(format!("{}-{}",
                                   file.file_name().unwrap().to_str().unwrap(),
                                   SystemTime::now().duration_since(UNIX_EPOCH).expect("Error reading time: positive time after unix time epoch expected").as_millis()
                               ));
                                let _ = std::fs::rename(&file, &new_file).map_err(|err| {
                                    ApplicationError::FailedRenamingFile {
                                        err,
                                        from: file.clone(),
                                        to: new_file,
                                    }
                                });
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
            } else {
                println!("'{}' not found", symlinks_cfg_path.to_str().unwrap());
            }

            if let Ok(secrets_cfg_file) = File::open(&secrets_cfg_path) {
                let secrets_reader = BufReader::new(&secrets_cfg_file);
                for line in secrets_reader.lines() {
                    let line = line.as_ref().map_err(|_| {
                        ApplicationError::ConfigFileReadError(secrets_cfg_path.clone())
                    })?;

                    let file = PathBuf::from(line);
                    let dotfile_path = dotfile_path(dotfiles_dir.join("secrets"), &file);
                }
            } else {
                println!("'{}' not found", secrets_cfg_path.to_str().unwrap());
            }
        }
        PrimaryAction::Add => {
            // TODO: Implement fix for edge case where file already is added to configuration
            let cfg_file_path = match FileType::from(&options.file_type) {
                FileType::Config => symlinks_cfg_path,
                FileType::Secret => secrets_cfg_path,
            };
            let file = &options.file.expect("File is required");
            let abs_path = fs::canonicalize(file)
                .map_err(|_| ApplicationError::PathConversionError(file.clone()))?;
            let abs_path_str = abs_path
                .to_str()
                .ok_or(ApplicationError::PathConversionError(abs_path.clone()))?;
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
                    if line.contains(abs_path_str) {
                        println!("File is already tracked");
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
                cfg_file.write(abs_path_str.as_bytes()).map_err(|err| {
                    ApplicationError::FailedWritingToFile(cfg_file_path.clone(), err)
                })?;
                println!(
                    "'{}' has been added to tracked configuration files",
                    abs_path_str
                );
            }
        }
        PrimaryAction::Remove => {
            let cfg_file_path = match FileType::from(&options.file_type) {
                FileType::Config => symlinks_cfg_path,
                FileType::Secret => secrets_cfg_path,
            };
            let file = options.file.expect("File is required");
            let abs_path =
                fs::canonicalize(&file).map_err(|_| ApplicationError::FileNotFound(file))?;
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
                    to: cfg_file_path,
                }
            })?;

            println!(
                "'{}' has been removed from tracked configuration files",
                abs_path
                    .to_str()
                    .ok_or(ApplicationError::PathConversionError(abs_path.clone()))?
            );
        }
        PrimaryAction::CreateKey => {
            let key_file = OpenOptions::new().write(true).create_new(true).open(options.secret_key.expect("Secret key location is required")).expect("Unable to open file");
            let mut writer = BufWriter::new(key_file);

            let key_params = pgp::SecretKeyParamsBuilder::default()
                .key_type(pgp::KeyType::Rsa(2048))
                .can_create_certificates(false)
                .can_sign(true)
                .primary_user_id("Me <me@example.com>".into())
                .preferred_symmetric_algorithms(
                    vec![pgp::crypto::sym::SymmetricKeyAlgorithm::AES256].into(),
                )
                .preferred_hash_algorithms(vec![pgp::crypto::hash::HashAlgorithm::SHA2_256].into())
                .preferred_compression_algorithms(
                    vec![pgp::types::CompressionAlgorithm::ZLIB].into(),
                )
                .build()
                .expect("Unable to create secret key params!");

            let secret_key = key_params
                .generate()
                .expect("Failed generating a plain key!");
            let signed_secret_key = secret_key
                .sign(|| {
                    rpassword::prompt_password("Please input a password to sign the PGP key")
                        .expect("Password required!")
                })
                .expect("Unable to sign the metadata on key!");

            signed_secret_key.to_writer(&mut writer).expect("Failed writing to file");
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
    if PathBuf::from(std::env::var("HOME").unwrap()) != parent {
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
