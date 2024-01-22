use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser};

#[derive(Parser, Debug)]
#[clap(name = "Dotfile Manager")]
pub(crate) struct Cli {
    #[clap(flatten)]
    pub(crate) primary_action: PrimaryActionGroup,
    #[clap(flatten)]
    pub(crate) sync_direction: SyncDirectionGroup,
    #[clap(flatten)]
    pub(crate) file_type: FileTypeGroup,
    /// File input, used to define a file to be added or removed from dotfiles
    #[clap(short='f', long, requires_all=["file_type", "file_action"])]
    pub(crate) file: Option<PathBuf>,
    /// PGP key which has different use cases depending on the function
    #[clap(short = 'k', long = "secret-key")]
    pub(crate) secret_key: Option<PathBuf>,
}

#[derive(Debug, Args)]
#[group(required = true, multiple = false, id = "primary_action")]
#[clap(group(ArgGroup::new("file_action").args(["add", "remove"]).requires_all(["file_type", "file"])))]
pub(crate) struct PrimaryActionGroup {
    /// Sync files between your computer or dotfiles source, '-k' is the private key which you have locked your secret file with
    #[clap(short = 'S', long, requires = "sync_direction")]
    pub(crate) sync: bool,
    /// Add a file to the dotfiles source control, <--secret|--config> define which type of file is
    /// added
    #[clap(short = 'A', long)]
    pub(crate) add: bool,
    /// Remove a file from the dotfiles source control, <--secret|--config> define which type of file is
    /// removed
    #[clap(short = 'R', long)]
    pub(crate) remove: bool,
    /// Create a new private key to encrypt your secrets. '-k' is the new location and name of the key.
    #[clap(short = 'K', long, requires = "secret_key")]
    pub(crate) create_key: bool,
}

#[derive(Debug, Args)]
#[group(requires = "sync", multiple = false, id = "sync_direction")]
pub(crate) struct SyncDirectionGroup {
    /// Sync from the filesystem, updating the files within the dotfiles source control.
    #[clap(short = 'F', long)]
    pub(crate) from_filesystem: bool,
    /// Sync from the dotfiles, updating the machine configuration.
    #[clap(short = 'D', long)]
    pub(crate) from_dotfiles: bool,
}

#[derive(Debug, Args)]
#[group(requires_all = ["file_action", "file"], multiple = false, id="file_type")]
pub(crate) struct FileTypeGroup {
    /// A secret which stored using key and password within the source control.
    #[clap(short = 's', long)]
    pub(crate) secret: bool,
    /// A configuration file used to configure the machine.
    #[clap(short = 'c', long)]
    pub(crate) config: bool,
}

pub(crate) enum PrimaryAction {
    Sync,
    Add,
    Remove,
    CreateKey,
}

impl From<&PrimaryActionGroup> for PrimaryAction {
    fn from(value: &PrimaryActionGroup) -> Self {
        if value.sync {
            return Self::Sync;
        } else if value.create_key {
            return Self::CreateKey;
        } else if value.add {
            return Self::Add;
        } else if value.remove {
            return Self::Remove;
        } else {
            unreachable!();
        }
    }
}

pub(crate) enum SyncDirection {
    FromFilesystem,
    FromDotfiles,
}

impl From<&SyncDirectionGroup> for SyncDirection {
    fn from(value: &SyncDirectionGroup) -> Self {
        if value.from_dotfiles {
            return Self::FromDotfiles;
        } else if value.from_filesystem {
            return Self::FromFilesystem;
        } else {
            unreachable!();
        }
    }
}

pub(crate) enum FileType {
    Secret,
    Config,
}

impl From<&FileTypeGroup> for FileType {
    fn from(value: &FileTypeGroup) -> Self {
        if value.secret {
            return Self::Secret;
        } else if value.config {
            return Self::Config;
        } else {
            unreachable!();
        }
    }
}
