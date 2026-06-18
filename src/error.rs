use thiserror::Error;

pub type EmbedBResult<T> = Result<T, EmbedbError>;

#[derive(Debug, Error)]
pub enum EmbedbError {
    #[error("DB does not exist at given path")]
    MissingDB,
    #[error("insufficient permissions when accessing DB")]
    PermissionDenied,
    #[error("failed to mmap the store")]
    MmapFailed,
    #[error("failed to flush the store")]
    FlushFailed,
    #[error("store has invalid magic bytes")]
    InvalidMagic,
}

impl From<std::io::Error> for EmbedbError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => Self::MissingDB,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            kind => unreachable!("found unhandled IO error `{kind}`"),
        }
    }
}
