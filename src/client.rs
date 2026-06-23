use std::{collections::BinaryHeap, path::{Path, PathBuf}};

use simsimd::SpatialSimilarity;

use crate::{
    error::{EmbedBResult, EmbedbError, StoreError},
    header::EmbedBHeader,
    metadata::MetadataTable,
    mmap::GrowableMmap,
    search::SearchResult,
};

macro_rules! try_optional {
    ($value: expr) => {
        match $value {
            Ok(Some(value)) => value,
            Ok(None) => return Ok(None),
            Err(err) => return Err(err),
        }
    };
}

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
    pub(crate) header: EmbedBHeader,
    /// The handle to the stored mmap'ed vector store.
    pub(crate) store: GrowableMmap,
    /// A connection to the metadata SQLite database.
    pub(crate) metadata: MetadataTable,
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
        let mut store = GrowableMmap::open(base.join("store.embedb"))?;
        let metadata = MetadataTable::open(base.join("metadata.db3"))?;
        let mut guard = store.acquire_write()?;
        let header = guard.read(16).and_then(EmbedBHeader::parse)?;
        if header.dimensionality != dimensionality {
            return Err(StoreError::InvalidHeader.into());
        }

        let offset = metadata.next_offset(&header)?;
        guard.advance(offset);
        drop(guard);
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
            .and_then(|mut guard| guard.append(header.to_bytes()))?;

        Ok(Self {
            base,
            header,
            store,
            metadata,
        })
    }

    /// Reads the vector associated with `label` from the store
    /// and copies it to a vector.
    pub fn get(&self, label: &str) -> EmbedBResult<Option<Vec<f32>>> {
        let metadata = try_optional!(self.metadata.get(label));
        let start = metadata.offset;
        let end = start + (self.header.dimensionality * 4) as usize;
        let guard = self.store.acquire_read()?;
        let embedding = guard.read_range(start..end)?;
        Ok(Some(bytemuck::cast_slice(embedding).to_vec()))
    }

    /// Searches the store for the `k` vectors most similar to `query` using
    /// brute-force cosine similarity, returning results in descending order
    /// of relevance. Returns an error if `query` has a different
    /// dimensionality than the store.
    pub fn search(&self, k: usize, query: impl AsRef<[f32]>) -> EmbedBResult<Vec<SearchResult>> {
        let query = query.as_ref();
        if query.len() != self.header.dimensionality as usize {
            return Err(EmbedbError::DimensionMismatch {
                expected: self.header.dimensionality as usize,
                actual: query.len(),
            });
        }

        let guard = self.store.acquire_read()?;
        let length = (self.header.dimensionality * 4) as usize;
        let mut matches = BinaryHeap::new();
        matches.reserve_exact(k + 1);
        self.metadata.for_each(|metadata| {
            let start = metadata.offset;
            let end = start + length;
            let buffer: &[f32] = guard.read_range(start..end).map(bytemuck::cast_slice)?;
            let relevance = f32::cosine(query, buffer).expect("query has the same dimensionality");
            matches.push(SearchResult { label: metadata.label, relevance });
            if matches.len() > k {
                matches.pop();
            }

            Ok(())
        })?;

        Ok(matches.into_sorted_vec())
    }

    /// Inserts an embedding into the store.
    pub fn insert(&mut self, label: &str, embedding: impl AsRef<[f32]>) -> EmbedBResult<()> {
        let embedding = embedding.as_ref();
        if embedding.len() != self.header.dimensionality as usize {
            return Err(EmbedbError::DimensionMismatch {
                expected: self.header.dimensionality as usize,
                actual: embedding.len(),
            });
        }

        let mut guard = self.store.acquire_write()?;
        let offset = guard.cursor();
        let n = guard.write(bytemuck::cast_slice(embedding))?;
        self.metadata.insert(label, offset)?;
        guard.advance(n);
        Ok(())
    }

    /// Mark the vector associated with `label` as deleted.
    pub fn delete(&mut self, label: &str) -> EmbedBResult<()> {
        self.metadata.delete(label)
    }
}
