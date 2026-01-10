//! Checksum implementation for savegame validation
//!
//! This module provides a checksum type based on the DJB2 hash algorithm. The checksum
//! uses only 31 bits, with the most significant bit reserved as a validity marker.
//! This allows quick detection of uninitialized or invalid slots by checking if the
//! first byte has the high bit set.

/// A 31-bit checksum with validity marker
///
/// The checksum is computed using the DJB2 hash algorithm and masked to 31 bits.
/// The most significant bit (bit 31) must be zero for a valid checksum, allowing
/// quick detection of erased/uninitialized flash memory (which reads as 0xFF).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chksum(u32);

/// Mask to keep only the lower 31 bits for the checksum value
pub const CHKSUM_MASK: u32 = u32::MAX >> 1;

/// Mask for the high bit of the first byte (0x80)
///
/// Used to quickly detect invalid/erased slots where the first byte is 0xFF
pub const BYTE_MASK: u8 = !(u8::MAX >> 1); // 0x80

impl Chksum {
    /// Size of the checksum in bytes (4 bytes for u32)
    pub const SIZE: usize = u32::BITS as usize / 8;

    /// Create a zero checksum
    ///
    /// This is used as the initial previous checksum for the first savegame.
    pub const fn zero() -> Self {
        Self(0)
    }

    /// Compute a checksum for the given data, chained with a previous checksum
    ///
    /// Uses DJB2 hash algorithm. The previous checksum is included in the hash
    /// to create a chain of checksums linking savegames together.
    ///
    /// # Arguments
    ///
    /// * `prev` - The checksum of the previous savegame
    /// * `data` - The data to hash
    pub const fn hash(prev: Chksum, data: &[u8]) -> Self {
        let hash = djb2::hash(&prev.to_bytes());
        let hash = djb2::hash_with_initial(hash, data);
        Self(hash & CHKSUM_MASK)
    }

    /// Check if this checksum has a valid format
    ///
    /// A valid checksum has its most significant bit set to zero. This allows
    /// quick detection of uninitialized flash (0xFF) or corrupted data.
    pub const fn is_valid(&self) -> bool {
        let value = self.0 & !CHKSUM_MASK;
        value == 0
    }

    /// Convert the checksum to big-endian bytes
    pub const fn to_bytes(&self) -> [u8; Self::SIZE] {
        self.0.to_be_bytes()
    }

    /// Parse a checksum from big-endian bytes
    pub const fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        Self(u32::from_be_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chksum() {
        let data = b"hello world";
        let chksum = Chksum::hash(Chksum::zero(), data);
        assert_eq!(chksum, Chksum(646036933));
        assert!(chksum.is_valid());
    }

    #[test]
    fn test_header_mask() {
        let chksum = Chksum(0xFFFFFFFF);
        assert!(!chksum.is_valid());

        let chksum = Chksum(0x7FFFFFFF);
        assert!(chksum.is_valid());
    }
}
