use crate::storage::Flash;
use core::fmt;
use eh0::blocking::spi::Transfer;
use eh0::digital::v2::OutputPin;

impl<SPI: Transfer<u8>, CS: OutputPin> Flash for w25q::series25::Flash<SPI, CS>
where
    SPI::Error: fmt::Debug,
    CS::Error: fmt::Debug,
{
    type Error = w25q::Error<SPI, CS>;

    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        w25q::series25::Flash::read(self, addr, buf)?;
        Ok(())
    }

    fn write(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Self::Error> {
        self.write_bytes(addr, data)?;
        Ok(())
    }

    fn erase(&mut self, addr: u32) -> Result<(), Self::Error> {
        self.erase_sectors(addr, 1)?;
        Ok(())
    }

    fn erase_all(&mut self, _count: usize) -> Result<(), Self::Error> {
        w25q::series25::Flash::erase_all(self)
    }
}
