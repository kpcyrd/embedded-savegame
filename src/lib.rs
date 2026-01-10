#![no_std]
//! # Overview
//!
//! This library provides a power-fail safe savegame system for embedded devices with wear leveling.
//! It manages data storage on flash memory (EEPROM or NOR flash) by distributing writes across
//! multiple slots to prevent wear-out of specific memory locations.
//!
//! # Flash Support
//!
//! - `eeprom24x` feature: Support for AT24Cxx EEPROM chips
//! - `w25q` feature: Support for W25Q NOR flash chips
//! - `mock` feature: Mock flash implementations for testing
//!
//! # Example
//!
#![cfg_attr(feature = "mock", doc = r#"```"#)]
#![cfg_attr(not(feature = "mock"), doc = r#"```rust,compile_fail"#)]
//! use embedded_savegame::storage::{Storage, Flash};
//!
//! // Configure storage with 64-byte slots across 8 total slots
//! const SLOT_SIZE: usize = 64;
//! const SLOT_COUNT: usize = 8;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # use embedded_savegame::mock::SectorMockFlash;
//! # let mut flash_device = SectorMockFlash::<SLOT_SIZE, SLOT_COUNT>::new();
//! let mut storage = Storage::<_, SLOT_SIZE, SLOT_COUNT>::new(flash_device);
//!
//! // Scan for existing savegame
//! if let Some(slot) = storage.scan()? {
//!     let mut buf = [0u8; 256];
//!     if let Some(data) = storage.read(slot.idx, &mut buf)? {
//!         // Process loaded savegame
//!     }
//! }
//!
//! // Write new savegame
//! let mut save_data = b"game state data".to_vec();
//! storage.append(&mut save_data)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Architecture
//!
//! Each slot contains a header with:
//! - Current savegame checksum
//! - Data length
//! - Previous savegame checksum (for chain verification)
//!
//! The scanner finds the most recent valid savegame by following the checksum chain.

pub mod chksum;
#[cfg(feature = "eeprom24x")]
pub mod eeprom24x;
#[cfg(any(test, feature = "mock"))]
pub mod mock;
pub mod storage;
#[cfg(feature = "w25q")]
pub mod w25q;

use crate::chksum::Chksum;

const LENGTH_SIZE: usize = 4;

/// A savegame slot containing metadata about stored data
///
/// Each slot represents a savegame header stored in flash memory. Slots form a chain
/// where each new savegame references the previous one via checksums, enabling the
/// scanner to find the most recent valid savegame even after power failures.
///
/// # Fields
///
/// - `idx`: The slot index in flash memory
/// - `chksum`: Checksum of the savegame data
/// - `len`: Length of the savegame data in bytes
/// - `prev`: Checksum of the previous savegame (for chain verification)
#[derive(Debug, PartialEq)]
pub struct Slot {
    pub idx: usize,
    pub chksum: Chksum,
    pub len: u32,
    pub prev: Chksum,
}

impl Slot {
    /// Size of the slot header in bytes: two checksums and one length field.
    /// The first byte of the checksum is also used to indicate if the slot is in use.
    pub const HEADER_SIZE: usize = Chksum::SIZE * 2 + LENGTH_SIZE;

    /// Create a new slot for the given data
    ///
    /// Calculates the checksum for the data and creates a slot that references
    /// the previous savegame's checksum.
    ///
    /// # Arguments
    ///
    /// * `idx` - The slot index where this will be stored
    /// * `prev` - The checksum of the previous savegame (or zero for first savegame)
    /// * `data` - The savegame data to store
    pub const fn create(idx: usize, prev: Chksum, data: &[u8]) -> Self {
        let chksum = Chksum::hash(prev, data);
        let len = data.len() as u32;
        Self {
            idx,
            chksum,
            len,
            prev,
        }
    }

    /// Check if this slot has valid checksums
    ///
    /// A slot is valid if both its checksum and previous checksum have the correct format
    /// (most significant bit is zero).
    pub const fn is_valid(&self) -> bool {
        self.chksum.is_valid() && self.prev.is_valid()
    }

    /// Check if this slot is an update to another slot
    ///
    /// Returns `true` if this slot's `prev` checksum matches the other slot's checksum,
    /// indicating this is a newer version of the savegame.
    pub fn is_update_to(&self, other: &Self) -> bool {
        self.prev == other.chksum
    }

    /// Calculate the total number of bytes used by this savegame
    ///
    /// Accounts for the header in the first slot and continuation bytes in
    /// subsequent slots if the savegame spans multiple slots.
    ///
    /// # Type Parameters
    ///
    /// * `SLOT_SIZE` - The size of each slot in bytes
    pub fn used_bytes<const SLOT_SIZE: usize>(&self) -> usize {
        let mut size = Self::HEADER_SIZE;
        let mut remaining_data = self.len as usize;
        let mut remaining_space = SLOT_SIZE - Self::HEADER_SIZE;

        loop {
            let this_round = remaining_space.min(remaining_data);
            size = size.saturating_add(this_round);
            remaining_data = remaining_data.saturating_sub(this_round);

            if remaining_data == 0 {
                break;
            }

            size = size.saturating_add(1); // for the next slot's header byte
            remaining_space = SLOT_SIZE - 1;
        }

        size
    }

    /// Calculate the index of the next free slot after this savegame
    ///
    /// Takes into account how many slots this savegame occupies and wraps around
    /// using modulo arithmetic.
    ///
    /// # Type Parameters
    ///
    /// * `SLOT_SIZE` - The size of each slot in bytes
    /// * `SLOT_COUNT` - The total number of slots available
    pub fn next_slot<const SLOT_SIZE: usize, const SLOT_COUNT: usize>(&self) -> usize {
        let used_slots = self.used_bytes::<SLOT_SIZE>().div_ceil(SLOT_SIZE);
        self.idx.saturating_add(used_slots) % SLOT_COUNT
    }

    /// Serialize the slot header to bytes for writing to flash
    ///
    /// The format is: checksum (4 bytes) + length (4 bytes) + prev checksum (4 bytes)
    pub fn to_bytes(&self) -> [u8; Self::HEADER_SIZE] {
        let mut buf = [0u8; Self::HEADER_SIZE];

        let (chksum, len, prev) =
            arrayref::mut_array_refs![&mut buf, Chksum::SIZE, LENGTH_SIZE, Chksum::SIZE];

        chksum.copy_from_slice(&self.chksum.to_bytes());
        len.copy_from_slice(&self.len.to_be_bytes());
        prev.copy_from_slice(&self.prev.to_bytes());

        buf
    }

    /// Deserialize a slot header from bytes
    ///
    /// # Arguments
    ///
    /// * `idx` - The slot index where this header was read from
    /// * `bytes` - The header bytes in the format: checksum + length + prev checksum
    pub fn from_bytes(idx: usize, bytes: [u8; Self::HEADER_SIZE]) -> Self {
        let (chksum, len, prev) =
            arrayref::array_refs![&bytes, Chksum::SIZE, LENGTH_SIZE, Chksum::SIZE];

        Self {
            idx,
            chksum: Chksum::from_bytes(*chksum),
            len: u32::from_be_bytes(*len),
            prev: Chksum::from_bytes(*prev),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOT_SIZE: usize = 64;
    const SLOT_COUNT: usize = 8;

    #[test]
    fn test_slot_to_bytes() {
        let slot = Slot::create(0, Chksum::zero(), b"hello");
        assert_eq!(
            slot.to_bytes(),
            [116, 186, 120, 103, 0, 0, 0, 5, 0, 0, 0, 0]
        );

        let append = Slot::create(1, slot.chksum, b"world");
        assert_eq!(
            append.to_bytes(),
            [21, 165, 57, 22, 0, 0, 0, 5, 116, 186, 120, 103]
        );
    }

    #[test]
    fn test_slot_size_small() {
        let slot = Slot::create(0, Chksum::zero(), b"ohai!");
        assert_eq!(slot.used_bytes::<SLOT_SIZE>(), Slot::HEADER_SIZE + 5);
        assert_eq!(slot.next_slot::<SLOT_SIZE, SLOT_COUNT>(), 1);
    }

    #[test]
    fn test_slot_size_full() {
        let bytes = [b'B'; SLOT_SIZE - Slot::HEADER_SIZE];
        let slot = Slot::create(0, Chksum::zero(), &bytes);
        assert_eq!(slot.used_bytes::<SLOT_SIZE>(), SLOT_SIZE);
        assert_eq!(slot.next_slot::<SLOT_SIZE, SLOT_COUNT>(), 1);
    }

    #[test]
    fn test_slot_spill_over() {
        let bytes = [b'B'; SLOT_SIZE];
        let slot = Slot::create(0, Chksum::zero(), &bytes);
        assert_eq!(
            slot.used_bytes::<SLOT_SIZE>(),
            // One extra because the continue-header
            Slot::HEADER_SIZE + SLOT_SIZE + 1,
        );
        assert_eq!(slot.next_slot::<SLOT_SIZE, SLOT_COUNT>(), 2);
    }

    #[test]
    fn test_slot_spill_over_twice() {
        let bytes = [b'B'; SLOT_SIZE * 2];
        let slot = Slot::create(0, Chksum::zero(), &bytes);
        assert_eq!(
            slot.used_bytes::<SLOT_SIZE>(),
            // Two extra because the continue-header
            Slot::HEADER_SIZE + SLOT_SIZE * 2 + 2,
        );
        assert_eq!(slot.next_slot::<SLOT_SIZE, SLOT_COUNT>(), 3);
    }
}
