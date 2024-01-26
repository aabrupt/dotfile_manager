# Dotfile Manager

This is a simple manager for your dotfiles. The features are following:
- Manage the list of symlinked files and secrets to be tracked in a source control (Github, Gitlab, etc.)
- Save secret files with pgp encryption (such as ssh keys)
- Automatically symlink files (and move them to source control)
- Generate PGP key (which you have to keep track of)

## Installation
This application is built in rust and is simply installed by a simple `cargo install --git`.
```bash
cargo install --git https://github.com/aabrupt/dotfile_manager
```

## Usage
Command below is the fastest way to get up and running with the dotfile manager.

```bash
dotfiles add --file-type config --file "/path/to/file"
dotfiles sync --sync-direction dotfiles
```

The dotfiles command has a simple file management system where you add or remove a file from the _tracked file register_ responsible for tracking that file.
Then you simply have to do a sync action to use the information within those registers and add or remove your files from the source control.

There is two types of registers within the program which is separately managed:
1. Config register which is tracking files to be symlinked within your filesystem (internally symlinks)
1. Secret register which is tracking files to be encrypted/decrypted into the source control/filesystem

The `--sync-direction` is responsible for providing information about if the program should pull the information while syncing from the source control or filesystem.
The difference being between adding a new file from the computer (`--sync-direction dotfiles`) or adding a new, possibly pulled down, file from the source control (`--sync-direction filesystem`)

To allow for encryption and decryption a pgp key must be provided.
Within the application is a command which allow you to create a secret key, which you will have to manage yourself in order to decrypt secrets within the source control.
```bash
dotfiles create-key -k "path/to/key"
```
**OBS**: `-k` is used to override the config file, if a secret key is already set within the config file you can omit this option.

### **Important**
The application assumes that you either have the source control located in "$HOME/.dotfiles" or have [configured](#configuration) another directory.

## Configuration
There is two locations where you can store your configuration file:
1. ~/.dotconf
1. ~/.dotfiles.conf
1. ~/.config/dotconf
1. ~/.config/dotfiles.conf

The syntax of the configuration file is similar to a desktop file or the windows `.ini` file. The file include a label and a key. Environment variables and other shell specific syntax is allowed. Syntax such as '~' or '$HOME'.

- Options
    - source\_control\_folder : /path/to/source-control
    - secret\_key : /path/to/pgp/secret-key
