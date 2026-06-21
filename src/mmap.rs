use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    ops::Index,
    path::Path,
    slice::SliceIndex,
};

use file_guard::{FileGuard, Lock};
use memmap2::MmapMut;

use crate::error::{EmbedBResult, StorageError};

pub(crate) const INITIAL_SIZE: u64 = 1_024;
pub(crate) const SOFT_THRESHOLD: u64 = 1_073_741_824;
pub(crate) const CHUNK_SIZE: u64 = 1_048_576;

/// A mmap'ed file that automatically grows the length of
/// its backing file.
///
/// Upon initialization, the file is allocated to 1 KB, which
/// doubles until a soft threshold of 1 GB is reached. Afterwards,
/// all future allocations occur in fixed chunks of 1 MB.
///
/// This implementation does not currently support automatic
/// truncation.
///
/// # Safety
///
/// Modifying the backing file of an active mmap outside of this
/// type is undefined behavior. Callers must ensure no external
/// process writes to the file while a [`GrowableMmap`] is live.
/// Within the EmbedB client, all accesses are coordinated through
/// `fcntl` (Unix) or `LockFileEx` (Windows) advisory locks.
#[derive(Debug)]
pub struct GrowableMmap {
    /// The backing file of the mmap'ed region.
    file: File,
    /// A mutable mmap'ed view of the file.
    mmap: MmapMut,
    /// Byte offset of the next read or write. Only advances after a
    /// complete, committed insert (mmap write + SQLite write). On a
    /// failed insert, the cursor is not advanced so the next insert
    /// overwrites the uncommitted bytes rather than leaving them orphaned.
    cursor: usize,
}

/// A temporary guard that holds a shared lock on the backing
/// file of a [`GrowableMmap`]. Read operations are gated through
/// this type.
pub struct ReadGuard<'a> {
    mmap: &'a MmapMut,
    cursor: &'a usize,
    _guard: FileGuard<&'a File>,
}

/// A temporary guard that holds an exclusive lock on the backing
/// file of a [`GrowableMmap`]. Write operations are gated through
/// this type.
pub struct WriteGuard<'a> {
    file: &'a File,
    mmap: &'a mut MmapMut,
    cursor: &'a mut usize,
    _guard: FileGuard<&'a File>,
}

impl GrowableMmap {
    /// Opens an existing store file at `path` for reading and writing,
    /// failing if it doesn't exist. The cursor is positioned at the
    /// start of the file.
    pub fn open(path: impl AsRef<Path>) -> EmbedBResult<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        let mmap = map_mut(&file)?;
        Ok(Self {
            file,
            mmap,
            cursor: 0,
        })
    }

    /// Creates a new store file at `path`, failing if it already exists.
    /// The file is pre-allocated to [`INITIAL_SIZE`] bytes before mapping.
    pub fn create(path: impl AsRef<Path>) -> EmbedBResult<Self> {
        let file = OpenOptions::new()
            .create_new(true)
            .read(true)
            .write(true)
            .open(path)?;

        file.set_len(INITIAL_SIZE)?;
        let mmap = map_mut(&file)?;
        Ok(Self {
            file,
            mmap,
            cursor: 0,
        })
    }

    /// Acquires a shared lock on the backing file and returns a
    /// [`ReadGuard`] through which read operations can be performed.
    pub fn acquire_read(&self) -> EmbedBResult<ReadGuard<'_>> {
        file_guard::lock(&self.file, Lock::Shared, 0, 1)
            .map(|guard| ReadGuard {
                mmap: &self.mmap,
                cursor: &self.cursor,
                _guard: guard,
            })
            .map_err(|_| StorageError::LockFailed.into())
    }

    /// Acquires an exclusive lock on the backing file and returns a
    /// [`WriteGuard`] through which write operations can be performed.
    pub fn acquire_write(&mut self) -> EmbedBResult<WriteGuard<'_>> {
        file_guard::lock(&self.file, Lock::Exclusive, 0, 1)
            .map(|guard| WriteGuard {
                file: &self.file,
                mmap: &mut self.mmap,
                cursor: &mut self.cursor,
                _guard: guard,
            })
            .map_err(|_| StorageError::LockFailed.into())
    }
}

impl ReadGuard<'_> {
    /// Reads bytes from the current cursor position into `buffer`.
    pub fn read(&mut self, buffer: &mut [u8]) -> EmbedBResult<usize> {
        let start = *self.cursor;
        let mut slice = &self.mmap[start..];
        let n = slice.read(buffer).expect("read is infallible");
        Ok(n)
    }

    /// Reads bytes from the given range into `buffer`.
    #[allow(unused)]
    pub fn read_range<R>(&self, range: R, buffer: &mut [u8]) -> EmbedBResult<usize>
    where
        R: SliceIndex<[u8], Output = [u8]>,
    {
        let mut slice = self.mmap.index(range);
        let n = slice.read(buffer).expect("read is infallible");
        Ok(n)
    }
}

impl WriteGuard<'_> {
    /// Advances the cursor by `n` bytes, committing the most recent write.
    #[allow(unused)]
    pub fn advance(&mut self, offset: usize) {
        *self.cursor += offset;
    }

    /// Writes `bytes` at the current cursor position, growing the
    /// backing file if necessary. Does not advance the cursor.
    pub fn write(&mut self, bytes: &[u8]) -> EmbedBResult<usize> {
        while *self.cursor + bytes.len() > self.mmap.len() {
            self.resize()?;
        }

        let start = *self.cursor;
        let end = start + bytes.len();
        let mut slice = &mut self.mmap[start..end];
        let n = slice.write(bytes).expect("write is infallible");
        self.mmap
            .flush_range(start, n)
            .map_err(|_| StorageError::FlushFailed)?;

        Ok(n)
    }

    /// Grows the backing file according to the allocation strategy: doubles
    /// the current size until [`SOFT_THRESHOLD`] is reached, then grows in
    /// fixed [`CHUNK_SIZE`] increments. Remaps after resizing.
    fn resize(&mut self) -> EmbedBResult<()> {
        let current_size = self.mmap.len() as u64;
        let updated_size = if current_size >= SOFT_THRESHOLD {
            current_size + CHUNK_SIZE
        } else {
            current_size * 2
        };

        self.file.set_len(updated_size)?;
        *self.mmap = map_mut(self.file)?;
        Ok(())
    }
}

/// # Safety
/// See [`GrowableMmap`]'s safety comment.
fn map_mut(file: &File) -> EmbedBResult<MmapMut> {
    unsafe { MmapMut::map_mut(file).map_err(|_| StorageError::MmapFailed.into()) }
}
