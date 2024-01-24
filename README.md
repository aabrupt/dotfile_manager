# Dotfile Manager

This is a simple manager for your dotfiles. The features are following:
- Manage the list of symlinked files and secrets to be tracked in a source control (Github, Gitlab, etc.)
- Save secret files with pgp encryption (such as ssh keys)
- Automatically symlink files (and move them to source control)
- Generate PGP key (which you have to keep track of)

## Installation
This application is built in rust and is simply installed by a simple `cargo install --git`.
```bash
cargo install --git **repository url here**
```

## Usage
There is two types of "Sync" actions:
1. From the filesystem which move files into the source control and the symlinks to the original location
2. From "dotfiles" which simply symlinks the files already inside the source control to your file system

The most simple way to get started is to add a file and the do a "Sync" action from the filesystem.

```bash
dotfiles -Acf "/path/to/file"
```

### **Important**
The application assumes that you either have the source control located in "$HOME/.dotfiles" or have [configured](#configuration) another directory.

## Configuration
There is two ways to configure the dotfiles.
1. Through the command line (limited)
2. Through a configuration file

While you can change some options on the fly using the command line, it is recommended that you use a persistant file to prevent mistakes.

### Using the command line
Configuring the secret key using the command line. Refer to `-h` to learn more about what you can configure using flags.
```bash
dotfiles -SFs /path/to/secret_key
```

### Using a configuration file
There is two locations where you can store your configuration file:
1. ~/.dotconf
2. ~/.config/dotconf

The syntax of the configuration file is similar to a desktop file or the windows `.ini` file. The file include a label and a key. Environment variables and other shell specific syntax is allowed. Syntax such as '~' or '$HOME'.

- Options
    - DotfilesDir : /path/to/source-control
    - SecretKey : /path/to/pgp/secret-key
