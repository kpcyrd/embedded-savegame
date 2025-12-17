use crate::storage::Flash;

pub struct MockFlash<const SIZE: usize> {
    data: [u8; SIZE],
}

impl<const SIZE: usize> MockFlash<SIZE> {
    pub const fn new() -> Self {
        Self { data: [0xFF; SIZE] }
    }
}

impl<const SIZE: usize> Flash for MockFlash<SIZE> {
    fn read(&self, addr: u32, buf: &mut [u8]) {
        let addr = addr as usize;
        let len = buf.len();
        buf.copy_from_slice(&self.data[addr..addr + len]);
    }

    fn write(&mut self, addr: u32, data: &[u8]) {
        let addr = addr as usize;
        let len = data.len();
        self.data[addr..addr + len].copy_from_slice(data);
    }

    fn erase(&mut self, addr: u32) {
        self.data[addr as usize] = 0xFF;
    }
}

pub struct SectorMockFlash<const SECTOR_SIZE: usize, const SECTOR_COUNT: usize> {
    data: [[u8; SECTOR_SIZE]; SECTOR_COUNT],
}

impl<const SECTOR_SIZE: usize, const SECTOR_COUNT: usize>
    SectorMockFlash<SECTOR_SIZE, SECTOR_COUNT>
{
    pub const fn new() -> Self {
        Self {
            data: [[0xFF; SECTOR_SIZE]; SECTOR_COUNT],
        }
    }

    fn div_rem(addr: u32) -> (usize, usize) {
        let addr = addr as usize;
        let sector = addr / SECTOR_SIZE;
        let offset = addr % SECTOR_SIZE;
        (sector, offset)
    }
}

impl<const SECTOR_SIZE: usize, const SECTOR_COUNT: usize> Flash
    for SectorMockFlash<SECTOR_SIZE, SECTOR_COUNT>
{
    fn read(&self, addr: u32, buf: &mut [u8]) {
        let (sector, offset) = Self::div_rem(addr);
        buf.copy_from_slice(&self.data[sector][offset..offset + buf.len()]);
    }

    fn write(&mut self, addr: u32, buf: &[u8]) {
        let (sector, offset) = Self::div_rem(addr);

        let mut flash = self.data[sector][offset..offset + buf.len()].iter_mut();
        for byte in buf {
            let flash_byte = flash.next().unwrap();
            *flash_byte &= *byte;
        }
    }

    fn erase(&mut self, addr: u32) {
        let (sector, _offset) = Self::div_rem(addr);
        self.data[sector] = [0xFF; SECTOR_SIZE];
    }
}
