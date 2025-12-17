use crate::storage::Flash;
use eeprom24x::Eeprom24xTrait;

impl<T: Eeprom24xTrait> Flash for T
where
    <T as Eeprom24xTrait>::Error: From<eeprom24x::Error<<T as Eeprom24xTrait>::Error>>,
{
    type Error = T::Error;

    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.read_data(addr, buf)?;
        Ok(())
    }

    fn write(&mut self, addr: u32, data: &[u8]) -> Result<(), Self::Error> {
        self.write_page(addr, data)?;
        Ok(())
    }

    fn erase(&mut self, addr: u32) -> Result<(), Self::Error> {
        self.write_byte(addr, 0xFF)?;
        Ok(())
    }
}
