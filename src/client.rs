use std::path::{Path, PathBuf};

use crate::{
    error::{EmbedBResult, EmbedbError},
    header::EmbedBHeader,
    mmap::GrowableMmap,
};

/// A client handle to an EmbedB store. Upon initialization,
/// the database is sized to 4 KB. Each subsequent allocation
/// doubles the size until a 1 GB soft-cap, and then in chunks of
/// 1 MB.
///
/// Each vector is stored contiguously with all metadata stored in
/// a separate SQLite store.
#[derive(Debug)]
pub struct EmbedBClient {
    /// The path to the database directory.
    #[allow(unused)]
    base: PathBuf,
    /// The header of the vector store.
    #[allow(unused)]
    header: EmbedBHeader,
    /// The handle to the stored mmap'ed vector store.
    #[allow(unused)]
    store: GrowableMmap,
}

impl EmbedBClient {
    /// Initializes the client with a database at `base`,
    /// creating it if it doesn't exist.
    pub fn new(base: impl AsRef<Path>, dimensionality: u32) -> EmbedBResult<Self> {
        let base = base.as_ref().to_owned();
        if base.exists() {
            Self::open(base, dimensionality)
        } else {
            Self::create(base, dimensionality)
        }
    }

    /// Opens an existing store at `base`, reads the header to verify
    /// the directory contains a valid EmbedB store, and returns an error
    /// if the header are absent or corrupted.
    fn open(base: PathBuf, dimensionality: u32) -> EmbedBResult<Self> {
        let mut buffer = [0u8; 16];
        let mut store = GrowableMmap::open(base.join("store.embedb"))?;
        store.read(&mut buffer)?;

        let header = EmbedBHeader::parse(&buffer)?;
        if header.dimensionality != dimensionality {
            return Err(EmbedbError::InvalidHeader);
        }

        Ok(Self {
            base,
            header,
            store,
        })
    }

    /// Creates a new store at `base`, writing the header to the
    /// store file to mark it as a valid EmbedB store.
    fn create(base: PathBuf, dimensionality: u32) -> EmbedBResult<Self> {
        let header = EmbedBHeader::new(dimensionality);
        let mut store = GrowableMmap::create(base.join("store.embedb"))?;
        store.write(header.to_bytes())?;
        Ok(Self {
            base,
            header,
            store,
        })
    }
}
