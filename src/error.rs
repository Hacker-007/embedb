use thiserror::Error;

pub type EmbedBResult<T> = Result<T, EmbedbError>;

/// Top-level error type for EmbedB operations.
#[derive(Debug, Error)]
pub enum EmbedbError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("embedding has dimensions `{actual}` but the store expected `{expected}`")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error(transparent)]
    Metadata(#[from] rusqlite::Error),
}

/// Errors arising from store-level file system operations.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("store not found at the given path")]
    NotFound,
    #[error("a store already exists at the given path")]
    AlreadyExists,
    #[error("failed to create the store directory")]
    DirectoryCreationFailed,
    #[error("insufficient permissions to access the store")]
    PermissionDenied,
    #[error("store header is absent, corrupted, or from an incompatible version")]
    InvalidHeader,
}

/// Errors arising from low-level mmap and file operations.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to memory-map the store file")]
    MmapFailed,
    #[error("failed to flush pending writes to disk")]
    FlushFailed,
    #[error("failed to resize the backing file")]
    ResizeFailed,
    #[error("failed to acquire a file lock")]
    LockFailed,
    #[error("got unknown I/O error `{0}`")]
    IO(std::io::Error),
}

impl From<std::io::Error> for EmbedbError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => StoreError::NotFound.into(),
            std::io::ErrorKind::AlreadyExists => StoreError::AlreadyExists.into(),
            std::io::ErrorKind::PermissionDenied => StoreError::PermissionDenied.into(),
            _ => StorageError::IO(error).into(),
        }
    }
}
