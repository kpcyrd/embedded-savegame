use crate::{Slot, chksum::Chksum};

pub trait Flash {
    fn read(&self, addr: u32, buf: &mut [u8]);

    fn write(&mut self, addr: u32, data: &[u8]);

    fn erase(&mut self, addr: u32);
}

pub struct Storage<F: Flash, const SLOT_SIZE: usize, const SLOT_COUNT: usize> {
    flash: F,
    prev: Chksum,
    idx: usize,
}

impl<F: Flash, const SLOT_SIZE: usize, const SLOT_COUNT: usize> Storage<F, SLOT_SIZE, SLOT_COUNT> {
    pub const SPACE: u32 = SLOT_SIZE as u32 * SLOT_COUNT as u32;

    pub const fn new(flash: F) -> Self {
        Self {
            flash,
            prev: Chksum::zero(),
            idx: 0,
        }
    }

    const fn addr(&self, idx: usize) -> u32 {
        ((idx % SLOT_COUNT) * SLOT_SIZE) as u32
    }

    pub fn scan(&self) -> Option<Slot> {
        // TODO: implement the actual scan logic
        let mut buf = [0u8; Slot::HEADER_SIZE];
        for idx in 0..SLOT_COUNT {
            self.flash.read(self.addr(idx), &mut buf);
            let slot = Slot::from_bytes(idx, buf);
            if slot.chksum.is_valid() {
                // TODO: set .idx properly (according to slot.len)
                return Some(slot);
            }
        }
        None
    }

    pub fn erase(&mut self, idx: usize) {
        self.flash.erase(self.addr(idx));
    }

    pub fn erase_all(&mut self) {
        // TODO: some flash chips have a better way to do bulk erase
        for idx in 0..SLOT_COUNT {
            self.erase(idx);
        }
    }

    pub fn read<'a>(&self, mut idx: usize, buf: &'a mut [u8]) -> Option<&'a mut [u8]> {
        let mut addr = self.addr(idx);
        let mut slot = [0u8; Slot::HEADER_SIZE];
        self.flash.read(addr, &mut slot);
        addr = addr.saturating_add(Slot::HEADER_SIZE as u32);
        let slot = Slot::from_bytes(idx, slot);

        let data = buf.get_mut(..slot.len as usize)?;
        let mut buf = &mut *data;
        let mut remaining_space = SLOT_SIZE - Slot::HEADER_SIZE;
        while !buf.is_empty() {
            let read_size = remaining_space.min(buf.len());
            let (to_read, remaining) = buf.split_at_mut(read_size);
            self.flash.read(addr, to_read);
            buf = remaining;

            idx = idx.saturating_add(1) % SLOT_COUNT;
            addr = self.addr(idx).saturating_add(1);
            remaining_space = SLOT_SIZE - 1;
        }

        // TODO: validate checksum

        Some(data)
    }

    pub fn write(&mut self, mut idx: usize, prev: Option<Chksum>, mut data: &[u8]) -> usize {
        let prev = prev.unwrap_or(Chksum::zero());
        let slot = Slot::create(idx, prev, data);
        let addr = self.addr(idx);
        let bytes = slot.to_bytes();
        self.flash.erase(addr);
        self.flash.write(addr, &bytes);

        let mut addr = addr.saturating_add(Slot::HEADER_SIZE as u32);
        let mut remaining_space = SLOT_SIZE - Slot::HEADER_SIZE;

        while !data.is_empty() {
            let write_size = remaining_space.min(data.len());
            let (to_write, remaining) = data.split_at(write_size);
            self.flash.write(addr, to_write);
            data = remaining;

            idx = idx.saturating_add(1) % SLOT_COUNT;
            // TODO: erase first byte of next slot, but only if more data remains
            /*
            addr = self.addr(idx);
            self.flash.erase(addr);
            */
            addr = self.addr(idx).saturating_add(1);
            remaining_space = SLOT_SIZE - 1;
        }

        idx
    }

    pub fn append(&mut self, data: &[u8]) {
        self.idx = self.write(self.idx, Some(self.prev), data);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockFlash;

    const SLOT_SIZE: usize = 64;
    const SLOT_COUNT: usize = 8;
    const SIZE: usize = SLOT_SIZE * SLOT_COUNT;

    const fn mock_storage() -> Storage<MockFlash<SIZE>, SLOT_SIZE, SLOT_COUNT> {
        let flash = MockFlash::<SIZE>::new();
        Storage::<_, SLOT_SIZE, SLOT_COUNT>::new(flash)
    }

    #[test]
    fn test_storage_empty_scan() {
        let flash = mock_storage();
        let slot = flash.scan();
        assert_eq!(slot, None);
    }

    #[test]
    fn test_storage_write() {
        let mut storage = mock_storage();

        let data = b"hello world";
        storage.append(data);

        let mut buf = [0u8; Slot::HEADER_SIZE];
        storage.flash.read(0, &mut buf);
        let slot = Slot::from_bytes(0, buf);
        assert_eq!(
            slot,
            Slot {
                idx: 0,
                prev: Chksum::zero(),
                chksum: Chksum::hash(data),
                len: data.len() as u32,
            }
        );
    }

    #[test]
    fn test_storage_write_scan() {
        let mut storage = mock_storage();

        let data = b"hello world";
        storage.append(data);

        let scan = storage.scan();
        assert_eq!(
            scan,
            Some(Slot {
                idx: 0,
                prev: Chksum::zero(),
                chksum: Chksum::hash(data),
                len: data.len() as u32,
            })
        );
    }

    #[test]
    fn test_storage_write_read() {
        let mut storage = mock_storage();

        let data = b"hello world";
        storage.append(data);

        let mut buf = [0u8; 1024];
        let slice = storage.read(0, &mut buf);

        assert_eq!(slice.map(|s| &*s), Some("hello world".as_bytes()));
    }
}
