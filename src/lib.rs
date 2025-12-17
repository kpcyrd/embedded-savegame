#![no_std]

pub mod chksum;
#[cfg(test)]
pub mod mock;
pub mod storage;

use crate::chksum::Chksum;

#[derive(Debug, PartialEq)]
pub struct Slot {
    pub idx: usize,
    pub prev: Chksum,
    pub chksum: Chksum,
    pub len: u32,
}

impl Slot {
    /// Two checksums and one length field.
    /// The first byte of the checksum is also used to tell if the slot is in use.
    pub const HEADER_SIZE: usize = Chksum::SIZE * 2 + 4;

    pub fn create(idx: usize, prev: Chksum, data: &[u8]) -> Self {
        let chksum = Chksum::hash(data);
        let len = data.len() as u32;
        Self {
            idx,
            prev,
            chksum,
            len,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.prev.is_valid() && self.chksum.is_valid()
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
        let slice = &mut buf[..];

        let (dest, slice) = slice.split_at_mut(Chksum::SIZE);
        dest.copy_from_slice(&self.prev.to_bytes());

        let (dest, slice) = slice.split_at_mut(Chksum::SIZE);
        dest.copy_from_slice(&self.chksum.to_bytes());

        let (dest, _slice) = slice.split_at_mut(4);
        dest.copy_from_slice(&self.len.to_be_bytes());

        buf
    }

    pub fn from_bytes(idx: usize, bytes: [u8; Self::HEADER_SIZE]) -> Self {
        // TODO: try to get rid of the unwraps
        let slice = &bytes[..];

        let (prev_bytes, slice) = slice.split_at(Chksum::SIZE);
        let prev = Chksum::from_bytes(prev_bytes.try_into().unwrap());

        let (chksum_bytes, slice) = slice.split_at(Chksum::SIZE);
        let chksum = Chksum::from_bytes(chksum_bytes.try_into().unwrap());

        let (len_bytes, _slice) = slice.split_at(4);
        let len = u32::from_be_bytes(len_bytes.try_into().unwrap());

        Self {
            idx,
            prev,
            chksum,
            len,
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
        assert_eq!(slot.to_bytes(), [0, 0, 0, 0, 54, 16, 166, 134, 0, 0, 0, 5,]);

        let append = Slot::create(1, slot.chksum, b"world");
        assert_eq!(
            append.to_bytes(),
            [54, 16, 166, 134, 58, 119, 17, 67, 0, 0, 0, 5]
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
