//! Mock flash implementations for testing
//!
//! This module provides mock flash devices that can be used for testing without
//! real hardware. Available with the `mock` feature.
//!
//! - [`MockFlash`]: Simple byte-addressable mock flash (like EEPROM)
//! - [`SectorMockFlash`]: Sector-based mock flash (like NOR flash)
//! - [`MeasuredMockFlash`]: Mock flash that tracks operation statistics

use crate::storage::Flash;
use core::convert::Infallible;

/// Simple mock flash device with byte-level operations
///
/// Simulates EEPROM-like flash where individual bytes can be written.
/// Initialized with all bytes set to 0xFF (erased state).
#[derive(Debug, PartialEq)]
pub struct MockFlash<const SIZE: usize> {
    data: [u8; SIZE],
}

impl<const SIZE: usize> MockFlash<SIZE> {
    pub const fn new() -> Self {
        Self { data: [0xFF; SIZE] }
    }
}

impl<const SIZE: usize> Default for MockFlash<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize> Flash for MockFlash<SIZE> {
    type Error = Infallible;

    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        let addr = addr as usize;
        let len = buf.len();
        buf.copy_from_slice(&self.data[addr..addr + len]);
        Ok(())
    }

    fn write(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Self::Error> {
        let addr = addr as usize;
        let len = data.len();
        self.data[addr..addr + len].copy_from_slice(data);
        Ok(())
    }

    fn erase(&mut self, addr: u32) -> Result<(), Self::Error> {
        self.data[addr as usize] = 0xFF;
        Ok(())
    }
}

/// Sector-based mock flash device
///
/// Simulates NOR flash where writes can only set bits from 1 to 0, and entire
/// sectors must be erased to set bits back to 1. This more accurately models
/// real NOR flash behavior.
#[derive(Debug, PartialEq)]
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

impl<const SECTOR_SIZE: usize, const SECTOR_COUNT: usize> Default
    for SectorMockFlash<SECTOR_SIZE, SECTOR_COUNT>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const SECTOR_SIZE: usize, const SECTOR_COUNT: usize> Flash
    for SectorMockFlash<SECTOR_SIZE, SECTOR_COUNT>
{
    type Error = Infallible;

    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        let (sector, offset) = Self::div_rem(addr);
        buf.copy_from_slice(&self.data[sector][offset..offset + buf.len()]);
        Ok(())
    }

    fn write(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        let (sector, offset) = Self::div_rem(addr);

        let mut flash = self.data[sector][offset..offset + buf.len()].iter_mut();
        for byte in buf {
            let flash_byte = flash.next().unwrap();
            *flash_byte &= *byte;
        }

        Ok(())
    }

    fn erase(&mut self, addr: u32) -> Result<(), Self::Error> {
        let (sector, _offset) = Self::div_rem(addr);
        self.data[sector] = [0xFF; SECTOR_SIZE];
        Ok(())
    }
}

/// Mock flash device that tracks operation statistics
///
/// Wraps [`MockFlash`] and counts the number of bytes read/written and erase operations.
/// Useful for analyzing storage efficiency and optimization.
#[derive(Debug, Default)]
pub struct MeasuredMockFlash<const SIZE: usize> {
    flash: MockFlash<SIZE>,
    /// Statistics for all flash operations performed
    pub stats: MeasuredStats,
}

/// Statistics for flash operations
///
/// Tracks the total number of bytes read/written and erase operations.
#[derive(Debug, Default, PartialEq)]
pub struct MeasuredStats {
    /// Total number of bytes read
    pub read: usize,
    /// Total number of bytes written
    pub write: usize,
    /// Total number of erase operations
    pub erase: usize,
}

impl<const SIZE: usize> MeasuredMockFlash<SIZE> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<const SIZE: usize> Flash for MeasuredMockFlash<SIZE> {
    type Error = Infallible;

    fn read(&mut self, addr: u32, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.stats.read = self.stats.read.saturating_add(buf.len());
        self.flash.read(addr, buf)
    }

    fn write(&mut self, addr: u32, data: &mut [u8]) -> Result<(), Self::Error> {
        self.stats.write = self.stats.write.saturating_add(data.len());
        self.flash.write(addr, data)
    }

    fn erase(&mut self, addr: u32) -> Result<(), Self::Error> {
        self.stats.erase = self.stats.erase.saturating_add(1);
        self.flash.erase(addr)
    }
}
