use log::{error, info};
use path_clean::PathClean;
use simple_logger::SimpleLogger;
use std::{
    env::current_dir,
    fs::{self, read_to_string, OpenOptions},
    io::Write,
    path::PathBuf,
};

mod cli;

use cli::Direction;

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Variable '{0}' is unset.\nPlease set it in the configuration file for your shell.")]
    UnsetDirectoryVariable(&'static str),
    #[error("Failed retrieving metadata from file: {0}")]
    FailedRetrievingFileMetadata(PathBuf),
    #[error("Failed expanding path: {0}")]
    FailedExpandingPath(PathBuf),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Unset home directory")]
    UnsetHomeDirectory,
    #[error("Failed converting from OS specific resource: {0:?}")]
    OSConversionError(std::ffi::OsString),
    #[error("Invalid subcommand passed to application")]
    InvalidSubcommand,
    #[error("Invalid argument passed to application: {0}")]
    InvalidArgument(&'static str),
}

impl Error {
    fn recover(self) {
        error!("{}", self)
    }
}

type Result<R> = std::result::Result<R, Error>;

const DIRECTORY_VARIABLE_NAME: &'static str = "DOTFILES_DIRECTORY";
const TRACKING_FILE_NAME: &'static str = ".tracking";
const REMOVED_FILE_NAME: &'static str = ".deleted";

fn main() {
    let logger = SimpleLogger::new();
    log::set_max_level(logger.max_level());
    if let Err(err) = log::set_boxed_logger(Box::new(logger)) {
        eprintln!("Failed initializing logger with error: {}", err);
    }
    if let Err(err) = _main() {
        error!("{}", err);
        info!("Fatal error, performing cleanup...");
    }
}

fn _main() -> Result<()> {
    let arg_matches = cli::parse_args();

    match arg_matches.subcommand().ok_or(Error::InvalidSubcommand)? {
        ("sync", args) => {
            let arg = "direction";
            sync(
                args.get_one::<Direction>(arg)
                    .ok_or(Error::InvalidArgument(arg))?
                    .to_owned(),
            )
        }
        ("add", args) => {
            let arg = "file";
            add(args
                .get_one::<PathBuf>(arg)
                .ok_or(Error::InvalidArgument(arg))?
                .to_owned())
        }
        ("remove", args) => {
            let arg = "file";
            remove(
                args.get_one::<PathBuf>(arg)
                    .ok_or(Error::InvalidArgument(arg))?
                    .to_owned(),
            )
        }
        _ => Err(Error::InvalidSubcommand),
    }?;

    Ok(())
}

/// # Sync files
/// There is two options when syncing: from filesystem or from dotfiles. One is
/// for including new files into the source control while the other is for
/// updating the filesystem with the pre-existing files within the source
/// control (usually when cloning it down to a new computer).
///
/// ## From Filesystem
/// Using the directives set up by `remove` and `add`, perform a sync operation
/// moving all files into source control and symlinking them to the original
/// location.
///
/// ### Actions
/// Locate files in the file system using the directives defined within the
/// file with its name described in the TRACKING_FILE_NAME constant. This file
/// is located within the toplevel of the source control directory defined
/// in an environment variable which name is described within the
/// DIRECTORY_VARIABLE_NAME constant.
///
/// ### Logic tree
/// - Does the file exist? False -> Skip
/// - Is the file a symlink? True ->
///     Is the file pointing to the source control?
///         False -> Log Error
///         True -> Skip
///
/// Move file from original location to source control. Symlink file in source
/// control to original location.
///
/// ## From Dotfiles
/// Using the directives set up by `remove` and `add`, perform a sync operation
/// symlinking all files within the source control to the specified location
/// within the filesystem.
///
/// ### Actions
/// Locate files in the source control using the directives defined within the
/// file with its name described in the TRACKING_FILE_NAME constant. This file
/// is located within the toplevel of the source control directory defined
/// in an environment variable which name is described within the
/// DIRECTORY_VARIABLE_NAME constant.
///
/// ### Logic tree
/// - Does the file in source control exist? False -> Log Error
/// - Does the file matching file in file system exist?
///     True -> Is the matching file a symlink?
///         True -> is the file pointing to the file in source control?
///             True -> Skip
///             False -> Delete and Replace
///         False -> Move and Replace
///     False -> Symlink to location
///
/// ## Cleanup
/// Since the remove command does no actually remove the file within the source
/// control, this step is required. Locate files using the file with its name
/// described within the DELETED_FILE_NAME constant. The file is located in the
/// source control directory defined in an environment variable which name is
/// described within the DIRECTORY_VARIABLE_NAME constant. Move those files to.
fn sync(direction: Direction) -> Result<()> {
    match direction {
        Direction::Filesystem => {
            info!("Syncing from filesystem");
            let dotfile_dir = dotfiles_directory();
        }
        Direction::Dotfiles => todo!("Sync files from dotfiles"),
    };

    todo!("Cleanup");
}

/// # Add file
/// Function that adds a new file to the list of tracked files. The function does not modify filesystem in any
/// way outside the configuration files.
fn add(file: PathBuf) -> Result<()> {
    let file = clean_path_to_store(file)?;
    info!("Adding file '{:?}'", file);

    push_tracked_file(&file, &dotfiles_directory()?, TRACKING_FILE_NAME)?;
    info!("File successfully tracked");

    Ok(())
}

/// # Remove file
/// Function that remove a file from the list of tracked files and add it to a new list tracking
/// the deleted files within the repository. The function does not modify the filesystem in any way
/// outside the configuration files.
fn remove(file: PathBuf) -> Result<()> {
    let file = clean_path_to_store(file)?;
    info!("Removing file '{:?}'", file);

    let dotfiles_directory = dotfiles_directory()?;
    remove_tracked_file(&file, &dotfiles_directory, TRACKING_FILE_NAME)?;
    info!("Successfully removed file from list of tracked files");
    push_tracked_file(&file, &dotfiles_directory, REMOVED_FILE_NAME)?;
    info!("Successfully added file to list of newly untracked files");

    Ok(())
}

fn dotfiles_directory() -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var(DIRECTORY_VARIABLE_NAME).map_err(|_| {
            Error::UnsetDirectoryVariable(DIRECTORY_VARIABLE_NAME)
        })?,
    ))
}

fn clean_path_to_store(mut path: PathBuf) -> Result<PathBuf> {
    if !path.starts_with("/") {
        path = current_dir()?.join(path);
    }
    path = path.clean();
    Ok(
        match path.strip_prefix(
            std::env::var_os("HOME").ok_or(Error::UnsetHomeDirectory)?,
        ) {
            Ok(path) => PathBuf::from("~").join(path),
            Err(_) => path,
        },
    )
}

fn try_expand_path(path: PathBuf) -> Result<PathBuf> {
    let str_path = path
        .to_str()
        .ok_or(Error::FailedExpandingPath(path.clone()))?;
    let exp_path = shellexpand::full(str_path)
        .map_err(|_| Error::FailedExpandingPath(path.clone()))?;
    let mut exp_path = PathBuf::from(exp_path.to_string());
    Ok(exp_path.clean())
}

fn relative_path(path: &PathBuf) -> Result<PathBuf> {
    let home = PathBuf::from(
        std::env::var_os("HOME").ok_or(Error::UnsetHomeDirectory)?,
    );
    let dotfile_dir = dotfiles_directory()?;
    Ok(match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) if parent == PathBuf::from("/") => {
            PathBuf::from(dotfile_dir).join("root").join(name)
        }
        (Some(parent), Some(name)) if parent != home => {
            PathBuf::from(dotfile_dir).join(parent).join(name)
        }
        (Some(_), Some(name)) => PathBuf::from(dotfile_dir).join(name),
        _ => {
            return Err(Error::FailedRetrievingFileMetadata(path.to_path_buf()))
        }
    })
}

fn list_tracked_files(
    dotfiles_directory: &PathBuf,
    tracking_file: &'static str,
) -> Result<Vec<PathBuf>> {
    let str = read_to_string(dotfiles_directory.join(tracking_file))?;

    Ok(str
        .lines()
        .map(|line| PathBuf::from(line))
        .collect::<Vec<PathBuf>>())
}

fn push_tracked_file(
    path: &PathBuf,
    dotfiles_directory: &PathBuf,
    tracking_file: &'static str,
) -> Result<()> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(dotfiles_directory.join(tracking_file))?;

    file.write_all(
        format!(
            "{}\n",
            path.to_str()
                .ok_or(Error::OSConversionError(path.as_os_str().to_owned()))?
        )
        .as_bytes(),
    )?;

    Ok(())
}

fn remove_tracked_file(
    path: &PathBuf,
    dotfiles_directory: &PathBuf,
    tracking_file: &'static str,
) -> Result<()> {
    let tracking_file_path = dotfiles_directory.join(tracking_file);
    let buf = read_to_string(&tracking_file_path)?;
    let mut new_buf = Vec::<String>::new();
    let path = path.as_os_str();
    for line in buf.lines() {
        if std::ffi::OsStr::new(line) == path {
            continue;
        }
        new_buf.push(line.to_string())
    }

    fs::write(tracking_file_path, new_buf.join("\n"))?;

    Ok(())
}

fn clear_tracked_files(
    dotfiles_directory: &PathBuf,
    tracking_file: &'static str,
) -> Result<()> {
    OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(dotfiles_directory.join(tracking_file))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, fs::read_to_string};

    use tmp_env::{create_temp_dir, TmpDir};

    use super::*;

    #[test]
    fn test_relative_path() {
        let _tmp_env = tmp_env::set_var(DIRECTORY_VARIABLE_NAME, "/");
        let path = PathBuf::from(PathBuf::from("/hello/world"));
        let relative_path = relative_path(&path).unwrap();

        assert_eq!(relative_path.to_str().unwrap(), "/hello/world");
    }

    #[test]
    fn test_relative_path_from_home() {
        let _tmp_env = tmp_env::set_var(DIRECTORY_VARIABLE_NAME, "/");
        let _tmp_home = tmp_env::set_var("HOME", "/hello");
        let path = PathBuf::from(PathBuf::from("/hello/world"));
        let relative_path = relative_path(&path).unwrap();

        assert_eq!(relative_path.to_str().unwrap(), "/world");
    }

    #[test]
    fn test_relative_path_from_root() {
        let _tmp_env = tmp_env::set_var(DIRECTORY_VARIABLE_NAME, "/");
        let path = PathBuf::from(PathBuf::from("/world"));
        let relative_path = relative_path(&path).unwrap();

        assert_eq!(relative_path.to_str().unwrap(), "/root/world");
    }

    #[test]
    fn test_tracking_push_read_remove_clear() {
        let tmp = create_temp_dir().unwrap();

        let dotfiles_directory = &tmp.to_path_buf();

        let path = ["file.txt"].into_iter().collect::<PathBuf>();
        let path2 = ["file2.txt"].into_iter().collect::<PathBuf>();

        push_tracked_file(&path, dotfiles_directory, TRACKING_FILE_NAME)
            .unwrap();
        push_tracked_file(&path2, dotfiles_directory, TRACKING_FILE_NAME)
            .unwrap();

        let buf = read_to_string(dotfiles_directory.join(TRACKING_FILE_NAME))
            .unwrap();
        assert_eq!(path.as_os_str(), OsStr::new(buf.lines().nth(0).unwrap()));

        let volatile_buf =
            list_tracked_files(dotfiles_directory, TRACKING_FILE_NAME).unwrap();
        assert_eq!(volatile_buf[0], PathBuf::from(buf.lines().nth(0).unwrap()));
        assert_eq!(volatile_buf[0], path.to_path_buf());

        remove_tracked_file(&path, dotfiles_directory, TRACKING_FILE_NAME)
            .unwrap();

        let buf =
            list_tracked_files(dotfiles_directory, TRACKING_FILE_NAME).unwrap();
        assert_eq!(buf.len(), 1);

        clear_tracked_files(dotfiles_directory, TRACKING_FILE_NAME).unwrap();

        let buf =
            list_tracked_files(dotfiles_directory, TRACKING_FILE_NAME).unwrap();
        assert_eq!(buf.len(), 0)
    }

    #[test]
    fn test_clean_path_to_store() {
        let path = std::env::var("HOME").unwrap();
        let path = clean_path_to_store(path.into()).unwrap();
        assert_eq!(path, PathBuf::from("~"));

        let path = PathBuf::from("/.././path");
        let path = clean_path_to_store(path).unwrap();

        assert_eq!(path, PathBuf::from("/path"));

        std::env::set_current_dir("/").unwrap();
        let path = PathBuf::from("path");
        let path = clean_path_to_store(path).unwrap();
        assert_eq!(path, current_dir().unwrap().join("path"));
    }

    #[test]
    fn test_try_expand_path() {
        let path = "~";
        let path = try_expand_path(path.into()).unwrap();

        assert_eq!(path, std::env::var_os("HOME").unwrap());

        let path = "$HOME";
        let path = try_expand_path(path.into()).unwrap();

        assert_eq!(path, std::env::var_os("HOME").unwrap());
    }
}
