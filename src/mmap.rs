use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    ops::Index,
    path::Path,
    slice::SliceIndex,
};

use file_guard::Lock;
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
    /// failed insert, the cursor is rolled back so the next insert
    /// overwrites the uncommitted bytes rather than leaving them orphaned.
    cursor: usize,
}

impl GrowableMmap {
    /// Opens an existing store file at `path` for reading and writing,
    /// failing if it doesn't exist. The cursor is positioned at the
    /// start of the file.
    pub fn open(path: impl AsRef<Path>) -> EmbedBResult<Self> {
        let path = path.as_ref();
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
        let path = path.as_ref();
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

    /// Reads bytes from the current cursor position into `buffer`.
    pub fn read(&mut self, buffer: &mut [u8]) -> EmbedBResult<usize> {
        let _guard = file_guard::lock(&self.file, Lock::Shared, 0, 1)?;
        let mut slice = &self.mmap[self.cursor..];
        let n = slice.read(buffer).expect("read is infallible");
        self.cursor += n;
        Ok(n)
    }

    /// Reads bytes from the given range into `buffer`.
    #[allow(unused)]
    pub fn read_range<R>(&self, range: R, buffer: &mut [u8]) -> EmbedBResult<usize>
    where
        R: SliceIndex<[u8], Output = [u8]>,
    {
        let _guard = file_guard::lock(&self.file, Lock::Shared, 0, 1)?;
        let mut slice = self.mmap.index(range);
        let n = slice.read(buffer).expect("read is infallible");
        Ok(n)
    }

    /// Writes `bytes` at the current cursor position, growing the
    /// backing file if necessary, then advances the cursor.
    pub fn write(&mut self, bytes: &[u8]) -> EmbedBResult<usize> {
        // Resize before acquiring the lock: resize requires &mut self,
        // which conflicts with the borrow held by the lock guard.
        while self.cursor + bytes.len() > self.mmap.len() {
            self.resize()?;
        }

        let _guard = file_guard::lock(&self.file, Lock::Exclusive, 0, 1)?;
        let end = self.cursor + bytes.len();
        let mut slice = &mut self.mmap[self.cursor..end];
        let n = slice.write(bytes).expect("write is infallible");
        self.mmap
            .flush_range(self.cursor, n)
            .map_err(|_| StorageError::FlushFailed)?;

        self.cursor += n;
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
        self.mmap = map_mut(&self.file)?;
        Ok(())
    }
}

/// # Safety
/// See [`GrowableMmap`] struct-level safety comment.
fn map_mut(file: &File) -> EmbedBResult<MmapMut> {
    unsafe { MmapMut::map_mut(file).map_err(|_| StorageError::MmapFailed.into()) }
}
