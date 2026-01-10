use crate::{
    Slot,
    chksum::{self, Chksum},
};
use core::fmt;

pub trait Flash {
    type Error: fmt::Debug;

    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error>;

    // XXX: The w25q crate requires data to be mutable for write operations
    fn write(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Self::Error>;

    fn erase(&mut self, addr: u32) -> Result<(), Self::Error>;

    /// Some flash chips have a better way to do bulk erase
    /// Default implementation erases all sectors one by one
    fn erase_all(&mut self, count: usize) -> Result<(), Self::Error> {
        for idx in 0..count {
            self.erase(idx as u32)?;
        }
        Ok(())
    }
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

    fn scan_slot(&mut self, idx: usize) -> Result<Option<Slot>, F::Error> {
        let mut buf = [0u8; Slot::HEADER_SIZE];
        let (head, tail) = arrayref::mut_array_refs![&mut buf, 1, Slot::HEADER_SIZE - 1];

        // Read first byte for sanity check to allow early skip
        let addr = self.addr(idx);
        self.flash.read(addr, head)?;

        if head[0] & chksum::BYTE_MASK != 0 {
            return Ok(None);
        }

        // Read the rest of the header
        let addr = addr.saturating_add(1);
        self.flash.read(addr, tail)?;

        // Parse and validate slot
        let slot = Slot::from_bytes(idx, buf);
        let slot = slot.is_valid().then_some(slot);
        Ok(slot)
    }

    pub fn scan(&mut self) -> Result<Option<Slot>, F::Error> {
        let mut current: Option<Slot> = None;

        for idx in 0..SLOT_COUNT {
            let Some(slot) = self.scan_slot(idx)? else {
                continue;
            };

            if let Some(existing) = &current {
                if slot.is_update_to(existing) {
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
        self.idx = 0;
        self.prev = Chksum::zero();
        self.flash.erase_all(SLOT_COUNT)
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

        Ok(Some(data))
    }

    /// Read a static-sized savegame directly from a single slot
    ///
    /// This is useful when you need a more lightweight read operation
    pub fn read_static<'a, const SIZE: usize>(
        &mut self,
        idx: usize,
        buf: &'a mut [u8; SIZE],
    ) -> Result<(), F::Error> {
        // Sanity check
        const {
            let space_available = SLOT_SIZE
                .checked_sub(Slot::HEADER_SIZE)
                .expect("Invalid SLOT_SIZE, Slot::HEADER_SIZE doesn't fit");
            assert!(SIZE <= space_available);
        }

        // Calculate address behind slot header
        let addr = self.addr(idx).saturating_add(Slot::HEADER_SIZE as u32);
        // Read data directly into the buffer in one go
        self.flash.read(addr, buf)?;

        Ok(())
    }

    pub fn write(
        &mut self,
        mut idx: usize,
        prev: Chksum,
        mut data: &mut [u8],
    ) -> Result<(usize, Chksum), F::Error> {
        let slot = Slot::create(idx, prev, data);
        let slot_addr = self.addr(idx);
        self.flash.erase(slot_addr)?;

        let mut addr = slot_addr.saturating_add(Slot::HEADER_SIZE as u32);
        let mut remaining_space = SLOT_SIZE - Slot::HEADER_SIZE;

        loop {
            let write_size = remaining_space.min(data.len());
            let (to_write, remaining) = data.split_at_mut(write_size);
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

        // Write header last, to finalize the slot
        // The last field is `prev`, marking the previous slot as outdated
        let mut bytes = slot.to_bytes();
        self.flash.write(slot_addr, &mut bytes)?;

        Ok((idx, slot.chksum))
    }

    pub fn append(&mut self, data: &mut [u8]) -> Result<(), F::Error> {
        let (idx, chksum) = self.write(self.idx, self.prev, data)?;
        self.idx = idx;
        self.prev = chksum;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MeasuredMockFlash, MeasuredStats, MockFlash, SectorMockFlash};
    use core::convert::Infallible;

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

    fn mock_measured_storage() -> Storage<MeasuredMockFlash<SIZE>, SLOT_SIZE, SLOT_COUNT> {
        let flash = MeasuredMockFlash::<SIZE>::new();
        Storage::<_, SLOT_SIZE, SLOT_COUNT>::new(flash)
    }

    fn test_storage_empty_scan<F: Flash<Error = Infallible>>(
        storage: &mut Storage<F, SLOT_SIZE, SLOT_COUNT>,
    ) {
        let Ok(slot) = storage.scan();
        assert_eq!(slot, None);
    }

    #[test]
    fn test_at24cxx_storage_empty_scan() {
        let mut storage = mock_storage();
        test_storage_empty_scan(&mut storage);
    }

    #[test]
    fn test_w25qxx_storage_empty_scan() {
        let mut storage = mock_sector_storage();
        test_storage_empty_scan(&mut storage);
    }

    #[test]
    fn test_measured_storage_empty_scan() {
        let mut storage = mock_measured_storage();
        test_storage_empty_scan(&mut storage);
        assert_eq!(
            storage.flash.stats,
            MeasuredStats {
                read: 8,
                write: 0,
                erase: 0,
            }
        );
    }

    #[test]
    fn test_storage_write() {
        let mut storage = mock_storage();

        let mut data = *b"hello world";
        storage.append(&mut data);

        let mut buf = [0u8; Slot::HEADER_SIZE];
        storage.flash.read(0, &mut buf);
        let slot = Slot::from_bytes(0, buf);
        assert_eq!(
            slot,
            Slot {
                idx: 0,
                chksum: Chksum::hash(Chksum::zero(), &data),
                len: data.len() as u32,
                prev: Chksum::zero(),
            }
        );
    }

    fn test_storage_write_scan<F: Flash<Error = Infallible>>(
        storage: &mut Storage<F, SLOT_SIZE, SLOT_COUNT>,
    ) {
        let mut data = *b"hello world";
        storage.append(&mut data);

        let Ok(scan) = storage.scan();
        assert_eq!(
            scan,
            Some(Slot {
                idx: 0,
                chksum: Chksum::hash(Chksum::zero(), &data),
                len: data.len() as u32,
                prev: Chksum::zero(),
            })
        );
    }

    #[test]
    fn test_at24cxx_storage_write_scan() {
        let mut storage = mock_storage();
        test_storage_write_scan(&mut storage);
    }

    #[test]
    fn test_w25qxx_storage_write_scan() {
        let mut storage = mock_sector_storage();
        test_storage_write_scan(&mut storage);
    }

    #[test]
    fn test_measured_storage_write_scan() {
        let mut storage = mock_measured_storage();
        test_storage_write_scan(&mut storage);
        assert_eq!(
            storage.flash.stats,
            MeasuredStats {
                read: 19,
                write: 23,
                erase: 1,
            }
        );
    }

    fn test_storage_write_read<F: Flash<Error = Infallible>>(
        storage: &mut Storage<F, SLOT_SIZE, SLOT_COUNT>,
    ) {
        let mut data = *b"hello world";
        storage.append(&mut data);

        let mut buf = [0u8; 1024];
        let Ok(slice) = storage.read(0, &mut buf);

        assert_eq!(slice.map(|s| &*s), Some("hello world".as_bytes()));
    }

    #[test]
    fn test_at24cxx_storage_write_read() {
        let mut storage = mock_storage();
        test_storage_write_read(&mut storage);
    }

    #[test]
    fn test_w25qxx_storage_write_read() {
        let mut storage = mock_sector_storage();
        test_storage_write_read(&mut storage);
    }

    #[test]
    fn test_measured_storage_write_read() {
        let mut storage = mock_measured_storage();
        test_storage_write_read(&mut storage);
        assert_eq!(
            storage.flash.stats,
            MeasuredStats {
                read: 23,
                write: 23,
                erase: 1,
            }
        );
    }

    fn test_storage_write_wrap_around<F: Flash<Error = Infallible>>(
        storage: &mut Storage<F, SLOT_SIZE, SLOT_COUNT>,
    ) {
        for num in 0..(SLOT_COUNT as u32 * 3 + 2) {
            let mut buf = [0u8; 6];
            num.to_be_bytes().iter().enumerate().for_each(|(i, b)| {
                buf[i] = *b;
            });
            storage.append(&mut buf);
        }

        let slot = storage.scan().unwrap().unwrap();
        assert_eq!(slot.idx, 1);
        assert_eq!(storage.idx, 2);

        let mut buf = [0u8; 32];
        let Ok(slice) = storage.read(slot.idx, &mut buf);
        assert_eq!(slice, Some(&mut [0, 0, 0, 25, 0, 0][..]));
    }

    #[test]
    fn test_at24cxx_storage_write_wrap_around() {
        let mut storage = mock_storage();
        test_storage_write_wrap_around(&mut storage);
    }

    #[test]
    fn test_w25qxx_storage_write_wrap_around() {
        let mut storage = mock_sector_storage();
        test_storage_write_wrap_around(&mut storage);
    }

    #[test]
    fn test_measured_storage_write_wrap_around() {
        let mut storage = mock_measured_storage();
        test_storage_write_wrap_around(&mut storage);
        assert_eq!(
            storage.flash.stats,
            MeasuredStats {
                read: 114,
                write: 468,
                erase: 26,
            }
        );
    }

    fn test_storage_big_write<F: Flash<Error = Infallible>>(
        storage: &mut Storage<F, SLOT_SIZE, SLOT_COUNT>,
    ) {
        let mut buf = [b'A'; SLOT_SIZE * 5];
        storage.append(&mut buf);
        let slot = storage.scan().unwrap().unwrap();
        assert_eq!(
            slot,
            Slot {
                idx: 0,
                chksum: Chksum::hash(Chksum::zero(), &buf),
                len: buf.len() as u32,
                prev: Chksum::zero(),
            }
        );

        let mut buf2 = [0u8; 512];
        let Ok(slice) = storage.read(slot.idx, &mut buf2);
        assert_eq!(slice.map(|s| &*s), Some(&buf[..]));

        let mut buf = [b'B'; SLOT_SIZE * 5];
        storage.append(&mut buf);
        let new_slot = storage.scan().unwrap().unwrap();
        assert_eq!(
            new_slot,
            Slot {
                idx: 6,
                chksum: Chksum::hash(slot.chksum, &buf),
                len: buf.len() as u32,
                prev: slot.chksum,
            }
        );
    }

    #[test]
    fn test_at24cxx_storage_big_write() {
        let mut storage = mock_storage();
        test_storage_big_write(&mut storage);
    }

    #[test]
    fn test_w25qxx_storage_big_write() {
        let mut storage = mock_sector_storage();
        test_storage_big_write(&mut storage);
    }

    #[test]
    fn test_measured_storage_big_write() {
        let mut storage = mock_measured_storage();
        test_storage_big_write(&mut storage);
        assert_eq!(
            storage.flash.stats,
            MeasuredStats {
                read: 370,
                write: 664,
                erase: 12,
            }
        );
    }

    fn test_append_after_scan<F: Flash<Error = Infallible>>(
        storage: &mut Storage<F, SLOT_SIZE, SLOT_COUNT>,
    ) {
        let mut big = [b'A'; SLOT_SIZE * 2];
        storage.append(&mut big);
        assert_eq!(storage.idx, 3);
        storage.idx = 0;

        storage.scan().unwrap();
        assert_eq!(storage.idx, 3);
        assert_eq!(storage.prev, Chksum::hash(Chksum::zero(), &big));
    }

    #[test]
    fn test_at24cxx_append_after_scan() {
        let mut storage = mock_storage();
        test_append_after_scan(&mut storage);
    }

    #[test]
    fn test_w25qxx_append_after_scan() {
        let mut storage = mock_sector_storage();
        test_append_after_scan(&mut storage);
    }

    #[test]
    fn test_measured_append_after_scan() {
        let mut storage = mock_measured_storage();
        test_append_after_scan(&mut storage);
        assert_eq!(
            storage.flash.stats,
            MeasuredStats {
                read: 19,
                write: 140,
                erase: 3,
            }
        );
    }
}
