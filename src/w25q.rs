//! W25Q NOR flash support
//!
//! This module provides a [`Flash`](crate::storage::Flash) implementation for W25Q series NOR flash chips.
//! Available with the `w25q` feature.
//!
//! Supports flash chips from the W25Q series using the `w25q` crate's driver.

use crate::storage::Flash;
use core::fmt;
use eh0::blocking::spi::Transfer;
use eh0::digital::v2::OutputPin;

/// Flash trait implementation for W25Q series NOR flash chips
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
