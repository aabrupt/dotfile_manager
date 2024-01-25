use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub(crate) enum ApplicationError {
    #[error("Unable to read config file: '{0}'")]
    ConfigFileReadError(PathBuf),
    #[error("File not found: '{0}'")]
    FileNotFound(PathBuf),
    #[error("File is not a dotfile, but is a symlink: '{0}'")]
    UntrackedSymlinkedFile(PathBuf),
    #[error("Unable to move/rename file '{from}' to '{to}'")]
    FailedRenamingFile {
        err: std::io::Error,
        from: PathBuf,
        to: PathBuf,
    },
    #[error("Root cannot contain configuration files: '{0}'")]
    FileInRoot(PathBuf),
    #[error("Could not open file '{0}' error '{1}'")]
    CouldNotOpenFile(PathBuf, std::io::Error),
    #[error("An error occured while writing to file '{0}'")]
    FailedWritingToFile(PathBuf, std::io::Error),
    #[error("Failed converting Path object to a string '{0}'")]
    PathConversionError(PathBuf),
    #[error("Failed reading file '{0}'")]
    ErrorReadingFile(PathBuf),
    #[error("Could not create directories leading to path: '{0}'")]
    CouldNotCreateDirectories(PathBuf, std::io::Error),
    #[error("Secret key is required")]
    SecretKeyRequired,
    #[error("Failed signing key '{0}'")]
    PGPKeySignError(PathBuf),
    #[error("Failed reading password from tty")]
    FailedReadingPassword,
    #[error("Plain key generation failed with error '{0}'")]
    KeyGenerationFailed(pgp::errors::Error),
    #[error("An error has occured while expanding variables within a string '{0}'")]
    ErrorExpandingVariable(shellexpand::LookupError<std::env::VarError>),
    #[error("File input is required for the program to function")]
    FileInputRequired,
    #[error("$HOME is not defined")]
    UndedfinedHomeVariable,
    #[error("An error has occured while encrypting content of '{0}': '{1}'")]
    FailedEncryptingContent(PathBuf, pgp::errors::Error),
    #[error("An error has occured while reading you pgp key: {0}")]
    FailedReadingKey(PathBuf, pgp::errors::Error),
    #[error("Error reading '{0}' containing pgp message to be encrypted: {1}")]
    PGPMessageReadError(PathBuf, pgp::errors::Error),
    #[error("An error has occured while writing information to '{0}': {1}")]
    PGPWriterError(PathBuf, pgp::errors::Error),
    #[error("Error while decrypting content of '{0}': incorrect key")]
    FailedDecryptingContent(PathBuf),
    #[error("Error reading message in decryptor: {0}")]
    FailedDecryptingMessageInContent(pgp::errors::Error),
    #[error("Error has occured while reading content within decrypted message: {0}")]
    ErrorReadingContentInMessage(pgp::errors::Error),
    #[error("Empty content within decrypted message")]
    NoContentInPGPMessage,
    #[error("Content within decrypted message is not UTF8 encoded")]
    MessageNotUTF8Encoded,
    #[error("Failed confirming creation of empty password")]
    FailedConfirmingPasswordChoice,
    #[error("Failed unlocking private key")]
    FailedUnlockingPrivateKey,
}
