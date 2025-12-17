use crate::{Slot, chksum::Chksum};

pub trait Flash {
    type Error;

    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error>;

    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Self::Error>;

    fn erase(&mut self, addr: u32) -> Result<(), Self::Error>;
}

#[derive(Debug)]
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

    pub fn scan(&mut self) -> Result<Option<Slot>, F::Error> {
        let mut current: Option<Slot> = None;
        let mut buf = [0u8; Slot::HEADER_SIZE];

        for idx in 0..SLOT_COUNT {
            self.flash.read(self.addr(idx), &mut buf)?;
            let slot = Slot::from_bytes(idx, buf);
            if !slot.is_valid() {
                continue;
            }

            if let Some(existing) = &current {
                if slot.is_update_to(&existing) {
                    current = Some(slot);
                }
            } else {
                current = Some(slot);
            }
        }

        if let Some(current) = &current {
            self.idx = current.next_slot::<SLOT_SIZE, SLOT_COUNT>();
            self.prev = current.chksum;
        }

        Ok(current)
    }

    pub fn erase(&mut self, idx: usize) -> Result<(), F::Error> {
        self.flash.erase(self.addr(idx))?;
        Ok(())
    }

    pub fn erase_all(&mut self) -> Result<(), F::Error> {
        // TODO: some flash chips have a better way to do bulk erase
        for idx in 0..SLOT_COUNT {
            self.erase(idx)?;
        }
        Ok(())
    }

    pub fn read<'a>(
        &mut self,
        mut idx: usize,
        buf: &'a mut [u8],
    ) -> Result<Option<&'a mut [u8]>, F::Error> {
        let mut addr = self.addr(idx);
        let mut slot = [0u8; Slot::HEADER_SIZE];
        self.flash.read(addr, &mut slot)?;
        addr = addr.saturating_add(Slot::HEADER_SIZE as u32);
        let slot = Slot::from_bytes(idx, slot);

        let Some(data) = buf.get_mut(..slot.len as usize) else {
            return Ok(None);
        };
        let mut buf = &mut *data;
        let mut remaining_space = SLOT_SIZE - Slot::HEADER_SIZE;
        while !buf.is_empty() {
            let read_size = remaining_space.min(buf.len());
            let (to_read, remaining) = buf.split_at_mut(read_size);
            self.flash.read(addr, to_read)?;
            buf = remaining;

            idx = idx.saturating_add(1) % SLOT_COUNT;
            addr = self.addr(idx).saturating_add(1);
            remaining_space = SLOT_SIZE - 1;
        }

        // TODO: validate checksum

        Ok(Some(data))
    }

    pub fn write(
        &mut self,
        mut idx: usize,
        prev: Option<Chksum>,
        mut data: &[u8],
    ) -> Result<(usize, Chksum), F::Error> {
        let prev = prev.unwrap_or(Chksum::zero());
        let slot = Slot::create(idx, prev, data);
        let chksum = slot.chksum;
        let addr = self.addr(idx);
        let bytes = slot.to_bytes();
        self.flash.erase(addr)?;
        self.flash.write(addr, &bytes)?;

        let mut addr = addr.saturating_add(Slot::HEADER_SIZE as u32);
        let mut remaining_space = SLOT_SIZE - Slot::HEADER_SIZE;

        loop {
            let write_size = remaining_space.min(data.len());
            let (to_write, remaining) = data.split_at(write_size);
            self.flash.write(addr, to_write)?;
            data = remaining;
            idx = idx.saturating_add(1) % SLOT_COUNT;

            // erase first byte of next slot, but only if more data remains
            if data.is_empty() {
                break;
            }

            addr = self.addr(idx);
            self.flash.erase(addr)?;

            addr = addr.saturating_add(1);
            remaining_space = SLOT_SIZE - 1;
        }

        Ok((idx, chksum))
    }

    pub fn append(&mut self, data: &[u8]) -> Result<(), F::Error> {
        let (idx, chksum) = self.write(self.idx, Some(self.prev), data)?;
        self.idx = idx;
        self.prev = chksum;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MockFlash, SectorMockFlash};

    const SLOT_SIZE: usize = 64;
    const SLOT_COUNT: usize = 8;
    const SIZE: usize = SLOT_SIZE * SLOT_COUNT;

    const fn mock_storage() -> Storage<MockFlash<SIZE>, SLOT_SIZE, SLOT_COUNT> {
        let flash = MockFlash::<SIZE>::new();
        Storage::<_, SLOT_SIZE, SLOT_COUNT>::new(flash)
    }

    const fn mock_sector_storage()
    -> Storage<SectorMockFlash<SLOT_SIZE, SLOT_COUNT>, SLOT_SIZE, SLOT_COUNT> {
        let flash = SectorMockFlash::<SLOT_SIZE, SLOT_COUNT>::new();
        Storage::<_, SLOT_SIZE, SLOT_COUNT>::new(flash)
    }

    fn test_storage_empty_scan<F: Flash>(mut storage: Storage<F, SLOT_SIZE, SLOT_COUNT>) {
        let slot = storage.scan();
        assert_eq!(slot, None);
    }

    #[test]
    fn test_at24cxx_storage_empty_scan() {
        let storage = mock_storage();
        test_storage_empty_scan(storage);
    }

    #[test]
    fn test_w25qxx_storage_empty_scan() {
        let storage = mock_sector_storage();
        test_storage_empty_scan(storage);
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

    fn test_storage_write_scan<F: Flash>(mut storage: Storage<F, SLOT_SIZE, SLOT_COUNT>) {
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
    fn test_at24cxx_storage_write_scan() {
        let storage = mock_storage();
        test_storage_write_scan(storage);
    }

    #[test]
    fn test_w25qxx_storage_write_scan() {
        let storage = mock_sector_storage();
        test_storage_write_scan(storage);
    }

    fn test_storage_write_read<F: Flash>(mut storage: Storage<F, SLOT_SIZE, SLOT_COUNT>) {
        let data = b"hello world";
        storage.append(data);

        let mut buf = [0u8; 1024];
        let slice = storage.read(0, &mut buf);

        assert_eq!(slice.map(|s| &*s), Some("hello world".as_bytes()));
    }

    #[test]
    fn test_at24cxx_storage_write_read() {
        let storage = mock_storage();
        test_storage_write_read(storage);
    }

    #[test]
    fn test_w25qxx_storage_write_read() {
        let storage = mock_sector_storage();
        test_storage_write_read(storage);
    }

    fn test_storage_write_wrap_around<F: Flash>(mut storage: Storage<F, SLOT_SIZE, SLOT_COUNT>) {
        for num in 0..(SLOT_COUNT as u32 * 3 + 2) {
            let mut buf = [0u8; 6];
            num.to_be_bytes().iter().enumerate().for_each(|(i, b)| {
                buf[i] = *b;
            });
            storage.append(&buf);
        }

        let slot = storage.scan().unwrap();
        assert_eq!(slot.idx, 1);
        assert_eq!(storage.idx, 2);

        let mut buf = [0u8; 32];
        let slice = storage.read(slot.idx, &mut buf);
        assert_eq!(slice, Some(&mut [0, 0, 0, 25, 0, 0][..]));
    }

    #[test]
    fn test_at24cxx_storage_write_wrap_around() {
        let storage = mock_storage();
        test_storage_write_wrap_around(storage);
    }

    #[test]
    fn test_w25qxx_storage_write_wrap_around() {
        let storage = mock_sector_storage();
        test_storage_write_wrap_around(storage);
    }

    fn test_storage_big_write<F: Flash>(mut storage: Storage<F, SLOT_SIZE, SLOT_COUNT>) {
        let buf = [b'A'; SLOT_SIZE * 5];
        storage.append(&buf);
        let slot = storage.scan().unwrap();
        assert_eq!(
            slot,
            Slot {
                idx: 0,
                prev: Chksum::zero(),
                chksum: Chksum::hash(&buf),
                len: buf.len() as u32,
            }
        );

        let mut buf2 = [0u8; 512];
        let slice = storage.read(slot.idx, &mut buf2);
        assert_eq!(slice.map(|s| &*s), Some(&buf[..]));

        let buf = [b'B'; SLOT_SIZE * 5];
        storage.append(&buf);
        let new_slot = storage.scan().unwrap();
        assert_eq!(
            new_slot,
            Slot {
                idx: 6,
                prev: slot.chksum,
                chksum: Chksum::hash(&buf),
                len: buf.len() as u32,
            }
        );
        // TODO: this test is also broken because it's parsing the content of a slot as header
    }

    #[test]
    fn test_at24cxx_storage_big_write() {
        let storage = mock_storage();
        test_storage_big_write(storage);
    }

    #[test]
    fn test_w25qxx_storage_big_write() {
        let storage = mock_sector_storage();
        test_storage_big_write(storage);
    }

    fn test_append_after_scan<F: Flash>(mut storage: Storage<F, SLOT_SIZE, SLOT_COUNT>) {
        let big = [b'A'; SLOT_SIZE * 2];
        storage.append(&big);
        assert_eq!(storage.idx, 3);
        storage.idx = 0;

        storage.scan();
        assert_eq!(storage.idx, 3);
        assert_eq!(storage.prev, Chksum::hash(&big));
    }

    #[test]
    fn test_at24cxx_append_after_scan() {
        let storage = mock_storage();
        test_append_after_scan(storage);
    }

    #[test]
    fn test_w25qxx_append_after_scan() {
        let storage = mock_sector_storage();
        test_append_after_scan(storage);
    }
}
