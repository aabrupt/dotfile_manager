use std::path::PathBuf;

use clap::{Args, Parser, ValueEnum};

#[derive(Parser, Debug)]
#[clap(name = "Dotfile Manager")]
pub(crate) struct Cli {
    /// The primary action for the application
    pub(crate) primary_action: PrimaryAction,
    /// Sync to specified location
    #[clap(
        short = 'D',
        long,
        required = false,
        required_if_eq("primary_action", "sync")
    )]
    pub(crate) sync_direction: SyncDirection,
    /// File type to be added into tracked files
    #[clap(short='F', long, required=false, required_if_eq_any=[("primary_action", "add"), ("primary_action", "remove")])]
    pub(crate) file_type: FileType,
    /// File input, used to define a file to be added or removed from dotfiles
    #[clap(short = 'f', long, requires = "file_type", required_if_eq_any=[("primary_action", "add"), ("primary_action", "remove")])]
    pub(crate) file: Option<PathBuf>,
    /// PGP key which has different use cases depending on the function
    #[clap(short = 'k', long)]
    pub(crate) secret_key: Option<PathBuf>,
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

#[derive(Debug, ValueEnum, Clone)]
pub(crate) enum PrimaryAction {
    Sync,
    Add,
    Remove,
    CreateKey,
}

#[derive(Debug, ValueEnum, Clone)]
pub(crate) enum SyncDirection {
    Dotfiles,
    Filesystem,
}

#[derive(Debug, ValueEnum, Clone)]
pub(crate) enum FileType {
    Secret,
    Config,
}
