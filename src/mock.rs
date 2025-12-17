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
