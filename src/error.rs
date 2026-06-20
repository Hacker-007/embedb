use thiserror::Error;

pub type EmbedBResult<T> = Result<T, EmbedbError>;

#[derive(Debug, Error)]
pub enum EmbedbError {
    #[error("store does not exist at the given path")]
    MissingDB,
    #[error("store already exists at the given path")]
    StoreAlreadyExists,
    #[error("insufficient permissions when accessing the store")]
    PermissionDenied,
    #[error("failed to mmap the store")]
    MmapFailed,
    #[error("failed to flush the store")]
    FlushFailed,
    #[error("store has an invalid header")]
    InvalidHeader,
    #[error(transparent)]
    IO(std::io::Error),
}

impl From<std::io::Error> for EmbedbError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => Self::MissingDB,
            std::io::ErrorKind::AlreadyExists => Self::StoreAlreadyExists,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            _ => Self::IO(error),
        }
    }
}
