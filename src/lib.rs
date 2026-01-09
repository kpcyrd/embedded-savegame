#![no_std]

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

#[derive(Debug, PartialEq)]
pub struct Slot {
    pub idx: usize,
    pub chksum: Chksum,
    pub len: u32,
    pub prev: Chksum,
}

impl Slot {
    /// Two checksums and one length field.
    /// The first byte of the checksum is also used to tell if the slot is in use.
    pub const HEADER_SIZE: usize = Chksum::SIZE * 2 + LENGTH_SIZE;

    pub fn create(idx: usize, prev: Chksum, data: &[u8]) -> Self {
        let chksum = Chksum::hash(prev, data);
        let len = data.len() as u32;
        Self {
            idx,
            chksum,
            len,
            prev,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.chksum.is_valid() && self.prev.is_valid()
    }

    pub fn is_update_to(&self, other: &Self) -> bool {
        self.prev == other.chksum
    }

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

    pub fn next_slot<const SLOT_SIZE: usize, const SLOT_COUNT: usize>(&self) -> usize {
        let used_slots = self.used_bytes::<SLOT_SIZE>().div_ceil(SLOT_SIZE);
        self.idx.saturating_add(used_slots) % SLOT_COUNT
    }

    pub fn to_bytes(&self) -> [u8; Self::HEADER_SIZE] {
        let mut buf = [0u8; Self::HEADER_SIZE];

        let (chksum, len, prev) =
            arrayref::mut_array_refs![&mut buf, Chksum::SIZE, LENGTH_SIZE, Chksum::SIZE];

        chksum.copy_from_slice(&self.chksum.to_bytes());
        len.copy_from_slice(&self.len.to_be_bytes());
        prev.copy_from_slice(&self.prev.to_bytes());

        buf
    }

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
