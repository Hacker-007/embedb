use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::EmbedBResult;

/// The metadata associated with a single vector
/// in the EmbedB store.
#[derive(Debug)]
#[allow(unused)]
pub struct VectorMetadata {
    /// The unique integer ID of the vector's metadata.
    id: i64,
    /// The unique, user-provided label for the vector.
    label: String,
    /// The byte offset of the vector in the mmap'ed store.
    offset: usize,
    /// A tombstone marker for soft-deleted vectors.
    is_deleted: bool,
}

/// A wrapper around a connection to the SQLite
/// metadata table.
#[derive(Debug)]
pub struct MetadataTable(Connection);

impl MetadataTable {
    /// Creates a connection to an existing table.
    /// to the table.
    pub fn open(path: impl AsRef<Path>) -> EmbedBResult<Self> {
        Connection::open(path).map(Self).map_err(Into::into)
    }

    /// Creates the metadata table and returns a fresh connection
    /// to the table.
    pub fn create(path: impl AsRef<Path>) -> EmbedBResult<Self> {
        let connection = Connection::open(path)?;
        connection.execute(
            "create table metadata (
                id         integer primary key,
                label      text    not null unique,
                offset     integer not null,
                is_deleted integer not null default 0
            )",
            [],
        )?;

        Ok(Self(connection))
    }

    /// Returns the metadata of the vector associated with
    /// `label`.
    #[allow(unused)]
    pub fn get(&self, label: &str) -> EmbedBResult<Option<VectorMetadata>> {
        self.0
            .query_row(
                "select id, label, offset, is_deleted from metadata where label = ?1",
                [label],
                |row| {
                    Ok(VectorMetadata {
                        id: row.get("id")?,
                        label: row.get("label")?,
                        offset: row.get("offset")?,
                        is_deleted: row.get("is_deleted")?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Inserts `metadata` into the table.
    #[allow(unused)]
    pub fn insert(&self, metadata: &VectorMetadata) -> EmbedBResult<()> {
        self.0.execute(
            "insert into metadata (label, offset, is_deleted) values (?1, ?2, ?3)",
            params![metadata.label, metadata.offset, metadata.is_deleted],
        )?;

        Ok(())
    }

    /// Marks the vector associated with `label` as deleted.
    #[allow(unused)]
    pub fn delete(&self, label: &str) -> EmbedBResult<()> {
        self.0.execute(
            "update metadata set is_deleted = 1 where label = ?1",
            [label],
        )?;

        Ok(())
    }
}
