use bytemuck::{Pod, Zeroable, try_from_bytes};

use crate::error::{EmbedBResult, EmbedbError};

const MAGIC: [u8; 8] = *b"embedb\0\0";
const VERSION: u16 = 1;

/// The header format for EmbedB stores.
///
/// [`EmbedBHeader`] is guaranteed to be a multiple of 4 bytes so
/// that subsequent reads from the mmap'ed file can be safely
/// interpreted as a contiguous range of [`f32`]s.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct EmbedBHeader {
    magic: [u8; 8],
    version: u16,
    padding: [u8; 2],
    pub dimensionality: u32,
}

impl EmbedBHeader {
    /// Creates a new [`EmbedBHeader`] with the given `dimensionality`
    /// and the current version.
    pub fn new(dimensionality: u32) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            padding: [0; 2],
            dimensionality,
        }
    }

    /// Parses an [`EmbedBHeader`] from a byte slice, returning an error if
    /// the slice is the wrong size, misaligned, has an invalid magic value,
    /// or has non-zero padding bytes.
    pub fn parse(buffer: &[u8]) -> EmbedBResult<Self> {
        let &header = try_from_bytes::<Self>(buffer).map_err(|_| EmbedbError::InvalidHeader)?;
        if header.magic != MAGIC || header.padding != [0; 2] {
            return Err(EmbedbError::InvalidHeader);
        }

        Ok(header)
    }

    /// Returns the header as a byte slice suitable for writing to the store file.
    pub fn to_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}
