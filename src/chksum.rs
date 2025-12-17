#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chksum(u32);

pub const MASK: u32 = u32::MAX >> 1;

impl Chksum {
    pub const SIZE: usize = u32::BITS as usize / 8;

    pub const fn zero() -> Self {
        Self(0)
    }

    pub fn hash(data: &[u8]) -> Self {
        let mut hasher = crc32fast::Hasher::new();
        hasher.update(data);
        Self(hasher.finalize() & MASK)
    }

    pub fn is_valid(&self) -> bool {
        let value = self.0 & !MASK;
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
        let chksum = Chksum::hash(data);
        assert_eq!(chksum, Chksum(222957957));
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
