#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chksum(u32);

pub const CHKSUM_MASK: u32 = u32::MAX >> 1;
pub const BYTE_MASK: u8 = !(u8::MAX >> 1); // 0x80

impl Chksum {
    pub const SIZE: usize = u32::BITS as usize / 8;

    pub const fn zero() -> Self {
        Self(0)
    }

    pub fn hash(prev: Chksum, data: &[u8]) -> Self {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(&prev.to_bytes());
        hasher.update(data);
        Self(hasher.finalize() & CHKSUM_MASK)
    }

    pub fn is_valid(&self) -> bool {
        let value = self.0 & !CHKSUM_MASK;
        value == 0
    }

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        self.0.to_be_bytes()
    }

    pub fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
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
        assert_eq!(chksum, Chksum(824091534));
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
