use std::path::{Path, PathBuf};

use crate::{
    error::{EmbedBResult, EmbedbError},
    mmap::GrowableMmap,
};

const MAGIC: &[u8; 8] = b"embedb\0\0";

/// A client handle to an EmbedB store. Upon initialization,
/// the database is sized to 4 KB. Each subsequent allocation
/// doubles the size until a 1 GB soft-cap, and then in chunks of
/// 128 MB.
///
/// Each vector is stored contiguously with all metadata stored in
/// a separate SQLite store.
#[derive(Debug)]
pub struct EmbedBClient {
    /// The path to the database directory.
    #[allow(unused)]
    base: PathBuf,
    /// The handle to the stored mmap'ed vector store.
    #[allow(unused)]
    store: GrowableMmap,
}

impl EmbedBClient {
    /// Initializes the client with a database at `base`,
    /// creating it if it doesn't exist.
    pub fn new(base: impl AsRef<Path>) -> EmbedBResult<Self> {
        let base = base.as_ref().to_owned();
        if base.exists() {
            Self::open(base)
        } else {
            Self::create(base)
        }
    }

    /// Opens an existing store at `base`, reads the magic header to verify
    /// the directory contains a valid EmbedB store, and returns an error
    /// if the magic bytes are absent or corrupted.
    fn open(base: PathBuf) -> EmbedBResult<Self> {
        let mut buffer = [0u8; 8];
        let mut store = GrowableMmap::open(base.join("store.embedb"))?;
        store.read(&mut buffer)?;
        if &buffer != MAGIC {
            return Err(EmbedbError::InvalidMagic);
        }

        Ok(Self { base, store })
    }

    /// Creates a new store at `base`, writing the magic header to the
    /// store file to mark it as a valid EmbedB store.
    fn create(base: PathBuf) -> EmbedBResult<Self> {
        let mut store = GrowableMmap::create(base.join("store.embedb"))?;
        store.write(MAGIC)?;
        Ok(Self { base, store })
    }
}
