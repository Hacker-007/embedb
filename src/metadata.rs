use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{error::EmbedBResult, header::EmbedBHeader};

/// The metadata associated with a single vector
/// in the EmbedB store.
#[derive(Debug)]
pub struct VectorMetadata {
    /// The unique integer ID of the vector's metadata.
    #[allow(unused)]
    id: i64,
    /// The unique, user-provided label for the vector.
    pub label: String,
    /// The byte offset of the vector in the mmap'ed store.
    pub offset: usize,
    /// A tombstone marker for soft-deleted vectors.
    #[allow(unused)]
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

    /// Executes `f` for each row in the metadata table that is
    /// currently active.
    pub fn for_each(
        &self,
        mut f: impl FnMut(VectorMetadata) -> EmbedBResult<()>,
    ) -> EmbedBResult<()> {
        let mut statement = self
            .0
            .prepare("select * from metadata where is_deleted = 0")?;

        let rows = statement.query_map([], |row| {
            Ok(VectorMetadata {
                id: row.get("id")?,
                label: row.get("label")?,
                offset: row.get("offset")?,
                is_deleted: row.get("is_deleted")?,
            })
        })?;

        for row in rows {
            f(row?)?;
        }

        Ok(())
    }

    /// Returns the byte offset from where new writes should begin.
    pub fn next_offset(&self, header: &EmbedBHeader) -> EmbedBResult<usize> {
        self.0
            .query_one(
                "select coalesce(max(offset) + ?1, ?2) from metadata",
                [header.dimensionality * 4, EmbedBHeader::SIZE as u32],
                |row| row.get::<_, usize>(0),
            )
            .map_err(Into::into)
    }

    /// Inserts `metadata` into the table.
    pub fn insert(&self, label: &str, offset: usize) -> EmbedBResult<()> {
        self.0.execute(
            "insert into metadata (label, offset) values (?1, ?2)
             on conflict(label) do update set offset = excluded.offset, is_deleted = 0",
            params![label, offset],
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
