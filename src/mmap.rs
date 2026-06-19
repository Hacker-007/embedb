use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    ops::Index,
    path::Path,
    slice::SliceIndex,
};

use memmap2::MmapMut;

use crate::error::{EmbedBResult, EmbedbError};

pub(crate) const INITIAL_SIZE: u64 = 1_024;
pub(crate) const SOFT_THRESHOLD: u64 = 1_073_741_824;
pub(crate) const CHUNK_SIZE: u64 = 1_048_576;

/// A mmap'ed file that automatically grows the
/// length of backing file.
///
/// Upon initialization, the file is allocated to 1 KB
/// which doubles until a soft threshold of 1 GB is reached.
/// Afterwards, all future allocations occur in chunks of
/// 32 MB.
///
/// NOTE:
/// This implementation does not currently provide support
/// for automatic trunction.
#[derive(Debug)]
pub struct GrowableMmap {
    /// The backing file of the mmap'ed region.
    file: File,
    /// A mutable mmap'ed view of the file.
    mmap: MmapMut,
    /// Byte offset of the next read or write.
    cursor: usize,
}

impl GrowableMmap {
    /// Opens an existing store file at `path` for reading and writing,
    /// failing if it doesn't exist. The cursor is positioned at the
    /// start of the file.
    pub fn open(path: impl AsRef<Path>) -> EmbedBResult<Self> {
        let path = path.as_ref();
        let file = OpenOptions::new().read(true).write(true).open(path)?;

        // SAFETY:
        // We own the file handle and guarantee that it is
        // not modified in or out-of-process.
        let mmap = unsafe { MmapMut::map_mut(&file).map_err(|_| EmbedbError::MmapFailed)? };
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
        // SAFETY:
        // We own the file handle and guarantee that it is
        // not modified in or out-of-process.
        let mmap = unsafe { MmapMut::map_mut(&file).map_err(|_| EmbedbError::MmapFailed)? };
        Ok(Self {
            file,
            mmap,
            cursor: 0,
        })
    }

    /// Advances the cursor by `n` bytes.
    #[inline(always)]
    pub fn advance(&mut self, n: usize) {
        self.cursor += n;
    }

    /// Reads bytes from the current cursor position into `buffer`.
    pub fn read(&mut self, buffer: &mut [u8]) -> EmbedBResult<usize> {
        let mut slice = &self.mmap[self.cursor..];
        let n = slice.read(buffer).expect("read is infallible");
        self.advance(n);
        Ok(n)
    }

    /// Reads bytes from the given range into `buffer`.
    pub fn read_range<R>(&self, range: R, buffer: &mut [u8]) -> EmbedBResult<usize>
    where
        R: SliceIndex<[u8], Output = [u8]>,
    {
        let mut slice = self.mmap.index(range);
        let n = slice.read(buffer).expect("read is infallible");
        Ok(n)
    }

    /// Writes `bytes` at the current cursor position, growing the
    /// backing file if necessary, then advances the cursor.
    pub fn write(&mut self, bytes: &[u8]) -> EmbedBResult<usize> {
        while self.cursor + bytes.len() > self.mmap.len() {
            self.resize()?;
        }

        let end = self.cursor + bytes.len();
        let mut slice = &mut self.mmap[self.cursor..end];
        let n = slice.write(bytes).expect("write is infallible");
        self.mmap
            .flush_range(self.cursor, n)
            .map_err(|_| EmbedbError::FlushFailed)?;

        self.advance(n);
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
        self.mmap = unsafe { MmapMut::map_mut(&self.file).map_err(|_| EmbedbError::MmapFailed)? };
        Ok(())
    }
}
