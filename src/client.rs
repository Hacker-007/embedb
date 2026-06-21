use std::path::{Path, PathBuf};

use crate::{
    error::{EmbedBResult, EmbedbError, StoreError},
    header::EmbedBHeader,
    metadata::MetadataTable,
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
    header: EmbedBHeader,
    /// The handle to the stored mmap'ed vector store.
    store: GrowableMmap,
    /// A connection to the metadata SQLite database.
    #[allow(unused)]
    metadata: MetadataTable,
}

impl EmbedBClient {
    /// Initializes the client with a database at `base`,
    /// creating it if it doesn't exist.
    pub fn new(base: impl AsRef<Path>, dimensionality: u32) -> EmbedBResult<Self> {
        let base = base.as_ref().to_owned();
        let test_path = base.join("store.embedb");
        if test_path.exists() {
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
        let store = GrowableMmap::open(base.join("store.embedb"))?;
        let metadata = MetadataTable::open(base.join("metadata.db3"))?;
        store
            .acquire_read()
            .and_then(|mut guard| guard.read(&mut buffer))?;

        let header = EmbedBHeader::parse(&buffer)?;
        if header.dimensionality != dimensionality {
            return Err(StoreError::InvalidHeader.into());
        }

        Ok(Self {
            base,
            header,
            store,
            metadata,
        })
    }

    /// Creates a new store at `base`, writing the header to the
    /// store file to mark it as a valid EmbedB store.
    fn create(base: PathBuf, dimensionality: u32) -> EmbedBResult<Self> {
        std::fs::create_dir_all(&base)?;
        let header = EmbedBHeader::new(dimensionality);
        let mut store = GrowableMmap::create(base.join("store.embedb"))?;
        let metadata = MetadataTable::create(base.join("metadata.db3"))?;
        store
            .acquire_write()
            .and_then(|mut guard| guard.write(header.to_bytes()))?;

        Ok(Self {
            base,
            header,
            store,
            metadata,
        })
    }

    /// Inserts an embedding into the store.
    pub fn insert(&mut self, embedding: impl AsRef<[f32]>) -> EmbedBResult<()> {
        let embedding = embedding.as_ref();
        if embedding.len() != self.header.dimensionality as usize {
            return Err(EmbedbError::DimensionMismatch {
                expected: self.header.dimensionality as usize,
                actual: embedding.len(),
            });
        }

        self.store
            .acquire_write()
            .and_then(|mut guard| guard.write(bytemuck::cast_slice(embedding)))
            .map(|_| ())
    }
}
