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

    pub fn is_update_to(&self, other: &Self) -> bool {
        self.prev == other.chksum
    }

    pub fn to_bytes(&self) -> [u8; Self::HEADER_SIZE] {
        let mut buf = [0u8; Self::HEADER_SIZE];
        let slice = &mut buf[..];

        let (dest, slice) = slice.split_at_mut(Chksum::SIZE);
        dest.copy_from_slice(&self.prev.to_bytes());

        let (dest, slice) = slice.split_at_mut(Chksum::SIZE);
        dest.copy_from_slice(&self.chksum.to_bytes());

        let (dest, _slice) = slice.split_at_mut(4);
        dest.copy_from_slice(&self.len.to_le_bytes());

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
        let len = u32::from_le_bytes(len_bytes.try_into().unwrap());

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

    #[test]
    fn test_slot_to_bytes() {
        let slot = Slot::create(0, Chksum::zero(), b"hello");
        assert_eq!(slot.to_bytes(), [0, 0, 0, 0, 134, 166, 16, 54, 5, 0, 0, 0]);

        let append = Slot::create(1, slot.chksum, b"world");
        assert_eq!(
            append.to_bytes(),
            [134, 166, 16, 54, 67, 17, 119, 58, 5, 0, 0, 0]
        );
    }
}
