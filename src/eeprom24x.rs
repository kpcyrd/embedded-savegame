use crate::storage::Flash;
use core::fmt;
use eeprom24x::Eeprom24xTrait;

impl<T: Eeprom24xTrait> Flash for T
where
    T::Error: fmt::Debug,
{
    type Error = eeprom24x::Error<T::Error>;

    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.read_data(addr, buf)?;
        Ok(())
    }

    fn write(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Self::Error> {
        self.write_page(addr, data)?;
        while self.read_current_address().is_err() {}
        Ok(())
    }

    fn erase(&mut self, addr: u32) -> Result<(), Self::Error> {
        if self.read_byte(addr)? != 0xFF {
            self.write_byte(addr, 0xFF)?;
            while self.read_current_address().is_err() {}
        }
        Ok(())
    }
}
