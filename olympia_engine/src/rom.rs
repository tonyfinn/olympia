//! ROM and Cartridge handling code

use crate::gameboy::memory;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::ops::Range;
use derive_more::Display;
use enum_dispatch::enum_dispatch;

const TARGET_CONSOLE_LOCATION: usize = 0x143;
const CARTRIDGE_TYPE_LOCATION: usize = 0x147;
const RAM_SIZE_LOCATION: usize = 0x149;

#[derive(PartialEq, Eq, Debug, Display)]
/// Error turning ROMs into cartridges
pub enum CartridgeLoadError {
    /// The ROM's cartridge type (at 0x147) is not known or supported
    #[display(fmt = "Unsupported cartridge type: 0x{:X}", "_0")]
    UnsupportedCartridgeType(u8),
    /// The ROM's RAM size (at 0x149) is not known or supported
    #[display(fmt = "Unsupported cartridge RAM size: 0x{:X}", "_0")]
    UnsupportedRamSize(u8),
    #[display(
        fmt = "Data for cartridge too small. Was 0x{:X} bytes, must be at least 0x200",
        "_0"
    )]
    /// The ROM data is smaller than the cartridge header, and likely corrupt
    CartridgeTooSmall(usize),
}

#[cfg(feature = "std")]
impl std::error::Error for CartridgeLoadError {}

#[derive(PartialEq, Eq, Debug, Display)]
/// Cartridge Read/Write errors
pub enum CartridgeIOError {
    /// Attempted IO to an address in cart RAM address space not filled on this cart
    #[display(fmt = "Address 0x{:X} outside of available cart ram", "_0")]
    ExceedsCartridgeRam(u16),
    /// Attempted IO to an address in cart ROM address space not filled on this cart
    #[display(fmt = "Address 0x{:X} exceeds ROM", "_0")]
    NoDataInRom(u16),
    /// Attempted IO to an address not in cart address space
    #[display(fmt = "Cannot read non-cart address 0x{:X} from cartridge", "_0")]
    NonCartAddress(u16),
    /// Attempted IO to cart RAM address space when this cart has no RAM
    #[display(fmt = "RAM not supported by current cartridge")]
    NoCartridgeRam,
    /// Attempted IO to cart RAM address space when cart RAM is disabled at runtime
    #[display(fmt = "RAM disabled on current cartridge")]
    CartridgeRamDisabled,
}

#[cfg(feature = "std")]
impl std::error::Error for CartridgeIOError {}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
/// Indicates if a ROM uses GameBoy Color features
pub enum TargetConsole {
    /// The ROM does not use Color features
    GameBoyOnly,
    /// The ROM uses Color features where supported
    ColorEnhanced,
    /// The ROM requires Color features
    ColorOnly,
}

/// Result of cartridge load operations
pub type CartridgeLoadResult<T> = Result<T, CartridgeLoadError>;
/// Result of cartridge read/write operations
pub type CartridgeIOResult<T> = Result<T, CartridgeIOError>;

/// A gameboy cartridge, including ROM data and memory controller
pub struct Cartridge {
    pub data: Vec<u8>,
    pub controller: ControllerEnum,
    pub target: TargetConsole,
}

impl Cartridge {
    /// Read a byte from address space controlled by the cart
    pub fn read(&self, loc: u16) -> CartridgeIOResult<u8> {
        if memory::STATIC_ROM.contains(loc) {
            self.controller.read_static_rom(loc, &self.data)
        } else if memory::SWITCHABLE_ROM.contains(loc) {
            self.controller.read_switchable_rom(loc, &self.data)
        } else if memory::CARTRIDGE_RAM.contains(loc) {
            self.controller.read_switchable_ram(loc)
        } else {
            Err(CartridgeIOError::NonCartAddress(loc))
        }
    }

    /// Write a byte to address space controlled by the cart
    pub fn write(&mut self, loc: u16, value: u8) -> CartridgeIOResult<()> {
        self.controller.write(loc, value)
    }

    /// Build a cartridge from ROM data
    pub fn from_data(data: Vec<u8>) -> CartridgeLoadResult<Cartridge> {
        if data.len() < 0x200 {
            return Err(CartridgeLoadError::CartridgeTooSmall(data.len()));
        }
        let cartridge_type_id = data[CARTRIDGE_TYPE_LOCATION];
        let ram_size = lookup_ram_size(data[RAM_SIZE_LOCATION])?;
        let target = lookup_target(data[TARGET_CONSOLE_LOCATION]);
        let controller = match cartridge_type_id {
            0 => StaticRom.into(),
            1..=3 => MBC1::new(ram_size, cartridge_type_id).into(),
            5 | 6 => MBC2::new(cartridge_type_id).into(),
            0x10..=0x13 => MBC3::new(ram_size, cartridge_type_id).into(),
            _ => {
                return Err(CartridgeLoadError::UnsupportedCartridgeType(
                    cartridge_type_id,
                ))
            }
        };
        Ok(Cartridge {
            controller,
            data,
            target,
        })
    }
}

/// Type of cartidge controller
#[enum_dispatch]
pub enum ControllerEnum {
    /// No controller, just static data
    StaticRom,
    /// Uses the MBC 1 controller chip
    Type1(MBC1),
    /// Uses the MBC 2 controller chip
    Type2(MBC2),
    /// Uses the MBC 3 controller chip
    Type3(MBC3),
}

/// Represents a cartridge controller
#[enum_dispatch(ControllerEnum)]
pub trait CartridgeController {
    /// Read a value from the controller's static ROM
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8>;
    /// Read a value from the controller's switchable ROM banks
    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8>;
    /// Read a value from the controller's switchable RAM banks
    fn read_switchable_ram(&self, loc: u16) -> CartridgeIOResult<u8>;
    /// Write a value to the controller's memory space
    fn write(&mut self, loc: u16, value: u8) -> CartridgeIOResult<()>;
    /// Indicates if a controller contains onboard RAM
    fn has_ram(&self) -> bool {
        false
    }
    /// Indicates if a controller contains battery backed RAM
    fn has_battery(&self) -> bool {
        false
    }
    /// Indicates if a controller contains battery backed timer
    fn has_timer(&self) -> bool {
        false
    }
    /// Indicates the size of onboard RAM, or 0 if absent
    fn ram_size(&self) -> usize;
}

/// A cartridge that contains only a static ROM w/o controller
pub struct StaticRom;

impl CartridgeController for StaticRom {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8> {
        rom.get(usize::from(loc))
            .copied()
            .ok_or(CartridgeIOError::NoDataInRom(loc))
    }

    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8> {
        self.read_static_rom(loc, rom)
    }

    fn read_switchable_ram(&self, _loc: u16) -> CartridgeIOResult<u8> {
        Err(CartridgeIOError::NoCartridgeRam)
    }

    fn write(&mut self, _loc: u16, _value: u8) -> CartridgeIOResult<()> {
        Ok(())
    }

    fn ram_size(&self) -> usize {
        0
    }
}

#[derive(PartialEq, Eq, Debug)]
enum MBC1PageMode {
    LargeRom,
    LargeRam,
}

/// MBC1 cartridge controller
pub struct MBC1 {
    page_mode: MBC1PageMode,
    selected_rom: u8,
    selected_high: u8,
    ram_enabled: bool,
    has_ram: bool,
    has_battery: bool,
    ram: Vec<u8>,
}

impl MBC1 {
    pub fn new(ram_size: usize, cartridge_type_id: u8) -> MBC1 {
        let has_ram = (cartridge_type_id & 0b10) != 0;
        let has_battery = (cartridge_type_id & 0b11) == 0b11;
        let ram = if has_ram {
            vec![0x00; ram_size]
        } else {
            Vec::new()
        };
        MBC1 {
            page_mode: MBC1PageMode::LargeRom,
            selected_rom: 1,
            selected_high: 0,
            ram_enabled: false,
            has_ram,
            has_battery,
            ram,
        }
    }

    fn selected_rom_bank(&self) -> u8 {
        let mut bank_id = self.selected_rom & 0x1F;
        if self.page_mode == MBC1PageMode::LargeRom {
            bank_id |= self.selected_high << 5;
        }
        bank_id
    }

    fn selected_static_rom_bank(&self) -> u8 {
        if self.page_mode == MBC1PageMode::LargeRam {
            0
        } else {
            self.selected_high << 5
        }
    }

    fn selected_ram_bank(&self) -> u8 {
        if self.page_mode == MBC1PageMode::LargeRom {
            0
        } else {
            self.selected_high
        }
    }

    const fn ram_enable_area() -> Range<u16> {
        0x0000..0x2000
    }

    const fn rom_select_area() -> Range<u16> {
        0x2000..0x4000
    }

    const fn high_select_area() -> Range<u16> {
        0x4000..0x6000
    }

    const fn mode_select_area() -> Range<u16> {
        0x6000..0x8000
    }
}

impl CartridgeController for MBC1 {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8> {
        let bank = u32::from(self.selected_static_rom_bank());
        let rom_addr = (bank * u32::from(memory::STATIC_ROM.len)) + u32::from(loc);
        rom.get(usize::try_from(rom_addr).expect("ROM too large for host platform"))
            .copied()
            .ok_or(CartridgeIOError::NoDataInRom(loc))
    }

    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8> {
        let bank_addr = loc - memory::SWITCHABLE_ROM.start;
        let bank = u32::from(self.selected_rom_bank());
        let rom_addr = (bank * u32::from(memory::SWITCHABLE_ROM.start)) + u32::from(bank_addr);
        rom.get(usize::try_from(rom_addr).expect("ROM too large for host platform"))
            .copied()
            .ok_or(CartridgeIOError::NoDataInRom(loc))
    }

    fn read_switchable_ram(&self, loc: u16) -> CartridgeIOResult<u8> {
        if self.has_ram && self.ram_enabled {
            let bank = u16::from(self.selected_ram_bank());
            let ram_addr = (bank * memory::CARTRIDGE_RAM.len) + (loc - memory::CARTRIDGE_RAM.start);
            Ok(self.ram[usize::from(ram_addr)])
        } else if self.has_ram {
            Err(CartridgeIOError::CartridgeRamDisabled)
        } else {
            Err(CartridgeIOError::NoCartridgeRam)
        }
    }

    fn write(&mut self, loc: u16, value: u8) -> CartridgeIOResult<()> {
        if MBC1::ram_enable_area().contains(&loc) {
            self.ram_enabled = value == 0b1010;
            log::info!(target: "rom::mbc1", "Toggled RAM enabled: {}", self.ram_enabled);
            Ok(())
        } else if MBC1::rom_select_area().contains(&loc) {
            self.selected_rom = value & 0x1F;
            if self.selected_rom == 0 {
                self.selected_rom = 1
            }
            log::info!(target: "rom::mbc1", "Selected Cart ROM bank: {}", self.selected_rom_bank());
            Ok(())
        } else if MBC1::high_select_area().contains(&loc) {
            self.selected_high = value & 0x3;
            if self.page_mode == MBC1PageMode::LargeRom {
                log::info!(
                    target: "rom::mbc1",
                    "Selected Static ROM bank: {}, Cart ROM bank: {}",
                    self.selected_static_rom_bank(), self.selected_rom_bank()
                );
            } else {
                log::info!(
                    target: "rom::mbc1",
                    "Selected RAM page: {}, Cart ROM bank: {}",
                    self.selected_ram_bank(), self.selected_rom_bank()
                );
            }
            Ok(())
        } else if MBC1::mode_select_area().contains(&loc) {
            if value == 0x00 {
                self.page_mode = MBC1PageMode::LargeRom
            } else {
                self.page_mode = MBC1PageMode::LargeRam
            }
            log::info!(target: "rom::mbc1", "Toggled Page Mode: {:?}", self.page_mode);
            Ok(())
        } else if memory::CARTRIDGE_RAM.contains(loc) {
            if self.ram_enabled {
                let ram_addr = loc - memory::CARTRIDGE_RAM.start;
                self.ram[usize::from(ram_addr)] = value;
            }
            Ok(())
        } else {
            unreachable!()
        }
    }

    fn has_ram(&self) -> bool {
        self.has_ram
    }

    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn ram_size(&self) -> usize {
        self.ram.len()
    }
}

/// MBC2 cartridge controller
pub struct MBC2 {
    selected_rom: u8,
    ram_enabled: bool,
    ram: Vec<u8>,
    has_battery: bool,
}

impl MBC2 {
    pub fn new(cartridge_type_id: u8) -> MBC2 {
        MBC2 {
            selected_rom: 1,
            ram_enabled: false,
            ram: vec![0x00; 512],
            has_battery: cartridge_type_id == 6,
        }
    }

    fn selected_rom_bank(&self) -> u8 {
        let bank_id = self.selected_rom & 0xF;
        if bank_id == 0 {
            1
        } else {
            bank_id
        }
    }
}

impl CartridgeController for MBC2 {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8> {
        Ok(rom[usize::from(loc)])
    }

    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8> {
        let bank_addr = loc - memory::SWITCHABLE_ROM.start;
        let bank = u16::from(self.selected_rom_bank());
        let rom_addr = (bank * memory::SWITCHABLE_ROM.len) + bank_addr;
        rom.get(usize::from(rom_addr))
            .copied()
            .ok_or(CartridgeIOError::NoDataInRom(loc))
    }

    fn read_switchable_ram(&self, loc: u16) -> CartridgeIOResult<u8> {
        if !self.ram_enabled {
            Err(CartridgeIOError::CartridgeRamDisabled)
        } else {
            let wrapped_ram_addr = (loc - memory::CARTRIDGE_RAM.start) % 0x200;
            Ok(self.ram[usize::from(wrapped_ram_addr)])
        }
    }

    fn write(&mut self, loc: u16, value: u8) -> CartridgeIOResult<()> {
        if memory::STATIC_ROM.contains(loc) {
            if loc & 0x100 == 0x100 {
                self.selected_rom = value & 0xF;
                log::info!(target: "rom::mbc2", "Selected ROM bank {}", self.selected_rom_bank());
            } else {
                self.ram_enabled = (value & 0xF) == 0b1010;
                log::info!(target: "rom::mbc2", "Toggled RAM {}", self.ram_enabled);
            }
            Ok(())
        } else if memory::CARTRIDGE_RAM.contains(loc) && self.ram_enabled {
            let ram_addr = loc - memory::CARTRIDGE_RAM.start;
            let wrapped_ram_addr = ram_addr % 0x200;
            self.ram[usize::from(wrapped_ram_addr)] = value & 0xF;
            Ok(())
        } else {
            Ok(())
        }
    }

    fn has_ram(&self) -> bool {
        true
    }

    fn has_battery(&self) -> bool {
        self.has_battery
    }

    fn ram_size(&self) -> usize {
        512
    }
}

fn lookup_ram_size(ram_size_id: u8) -> CartridgeLoadResult<usize> {
    match ram_size_id {
        0 => Ok(0),
        1 => Ok(2 * 1024),
        2 => Ok(8 * 1024),
        3 => Ok(32 * 1024),
        4 => Ok(128 * 1024),
        5 => Ok(64 * 1024),
        _ => Err(CartridgeLoadError::UnsupportedRamSize(ram_size_id)),
    }
}

fn lookup_target(target_id: u8) -> TargetConsole {
    match target_id {
        0xC0 => TargetConsole::ColorOnly,
        0x80 => TargetConsole::ColorEnhanced,
        _ => TargetConsole::GameBoyOnly,
    }
}

pub struct MBC3 {
    selected_rom: u8,
    selected_ram: u8,
    ram_enabled: bool,
    has_ram: bool,
    has_timer: bool,
    has_battery: bool,
    ram: Vec<u8>,
}

impl MBC3 {
    pub fn new(ram_size: usize, cartridge_type_id: u8) -> MBC3 {
        let has_timer = (cartridge_type_id & 0b100) != 0;
        let has_ram = (cartridge_type_id & 0b10) != 0;
        let has_battery = (cartridge_type_id & 0b11) == 0b11;
        MBC3 {
            selected_rom: 1,
            selected_ram: 0,
            ram_enabled: false,
            has_ram,
            has_timer,
            has_battery,
            ram: vec![0x00; ram_size],
        }
    }

    fn selected_rom_bank(&self) -> u8 {
        self.selected_rom & 0x7F
    }

    fn selected_static_rom_bank(&self) -> u8 {
        0
    }

    fn selected_ram_bank(&self) -> u8 {
        self.selected_ram
    }

    const fn ram_enable_area() -> Range<u16> {
        0x0000..0x2000
    }

    const fn rom_select_area() -> Range<u16> {
        0x2000..0x4000
    }

    const fn ram_select_area() -> Range<u16> {
        0x4000..0x6000
    }

    const fn timer_latch_area() -> Range<u16> {
        0x6000..0x8000
    }
}

impl CartridgeController for MBC3 {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8> {
        let bank = u32::from(self.selected_static_rom_bank());
        let rom_addr = (bank * u32::from(memory::STATIC_ROM.len)) + u32::from(loc);
        rom.get(usize::try_from(rom_addr).expect("ROM too large for host platform"))
            .copied()
            .ok_or(CartridgeIOError::NoDataInRom(loc))
    }

    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeIOResult<u8> {
        let bank_addr = loc - memory::SWITCHABLE_ROM.start;
        let bank = u32::from(self.selected_rom_bank());
        let rom_addr = (bank * u32::from(memory::SWITCHABLE_ROM.start)) + u32::from(bank_addr);
        rom.get(usize::try_from(rom_addr).expect("ROM too large for host platform"))
            .copied()
            .ok_or(CartridgeIOError::NoDataInRom(loc))
    }

    fn read_switchable_ram(&self, loc: u16) -> CartridgeIOResult<u8> {
        if self.has_ram && self.ram_enabled {
            let bank = u16::from(self.selected_ram_bank());
            let ram_addr = (bank * memory::CARTRIDGE_RAM.len) + (loc - memory::CARTRIDGE_RAM.start);
            Ok(self.ram[usize::from(ram_addr)])
        } else if self.has_ram {
            Err(CartridgeIOError::CartridgeRamDisabled)
        } else {
            Err(CartridgeIOError::NoCartridgeRam)
        }
    }

    fn write(&mut self, loc: u16, value: u8) -> CartridgeIOResult<()> {
        if MBC3::ram_enable_area().contains(&loc) {
            self.ram_enabled = (value & 0xF) == 0b1010;
            log::info!(target: "rom::mbc3", "Toggled ram: {}", self.ram_enabled);
            Ok(())
        } else if MBC3::rom_select_area().contains(&loc) {
            self.selected_rom = value & 0x7F;
            if self.selected_rom == 0 {
                self.selected_rom = 1
            }
            log::info!(target: "rom::mbc3", "Selected ROM bank: {}", self.selected_rom);
            Ok(())
        } else if MBC3::ram_select_area().contains(&loc) {
            self.selected_ram = value & 0x3;
            log::info!(target: "rom::mbc3", "Selected RAM bank: {}", self.selected_ram);
            Ok(())
        } else if MBC3::timer_latch_area().contains(&loc) {
            Ok(())
        } else if memory::CARTRIDGE_RAM.contains(loc) {
            if self.ram_enabled {
                let ram_addr = loc - memory::CARTRIDGE_RAM.start;
                self.ram[usize::from(ram_addr)] = value;
            }
            Ok(())
        } else {
            unreachable!()
        }
    }

    fn has_timer(&self) -> bool {
        self.has_timer
    }

    fn has_ram(&self) -> bool {
        self.has_ram
    }

    fn ram_size(&self) -> usize {
        self.ram.len()
    }

    fn has_battery(&self) -> bool {
        self.has_battery
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_rom() {
        let mut rom_data = vec![0x12; 32 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 0;
        rom_data[RAM_SIZE_LOCATION] = 0;
        rom_data[0x5500] = 0x23;
        let mut cartridge = Cartridge::from_data(rom_data).unwrap();

        assert_eq!(cartridge.target, TargetConsole::GameBoyOnly);
        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
        assert_eq!(cartridge.read(0x5500).unwrap(), 0x23);
        assert_eq!(
            cartridge.read(0xA111),
            Err(CartridgeIOError::NoCartridgeRam)
        );
        assert_eq!(
            cartridge.read(0x9222),
            Err(CartridgeIOError::NonCartAddress(0x9222))
        );
        assert_eq!(cartridge.write(0x1234, 0x22), Ok(()));
        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
        assert!(!cartridge.controller.has_ram());
        assert_eq!(cartridge.controller.ram_size(), 0);
    }

    #[test]
    fn test_mbc1_large_rom_basic_rom() {
        let mut rom_data = vec![0x12; 96 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 1;
        rom_data[RAM_SIZE_LOCATION] = 0;
        rom_data[0x5500] = 0x23;
        let cartridge = Cartridge::from_data(rom_data).unwrap();

        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
        assert_eq!(cartridge.read(0x5500).unwrap(), 0x23);
        assert_eq!(
            cartridge.read(0x9222),
            Err(CartridgeIOError::NonCartAddress(0x9222))
        );
        assert_eq!(
            cartridge.read(0xA111),
            Err(CartridgeIOError::NoCartridgeRam)
        );
        assert!(!cartridge.controller.has_ram());
        assert_eq!(cartridge.controller.ram_size(), 0);
    }

    #[test]
    fn test_mbc1_large_rom_basic_ram() -> CartridgeIOResult<()> {
        let mut rom_data = vec![0x12; 96 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 2;
        rom_data[RAM_SIZE_LOCATION] = 2;
        let mut cartridge = Cartridge::from_data(rom_data).unwrap();

        cartridge.write(0x00ff, 0b1010)?;
        cartridge.write(0xA111, 0x20)?;
        assert_eq!(cartridge.read(0xA111)?, 0x20);
        cartridge.write(0x00ff, 0b1000)?;
        cartridge.write(0xA111, 0x20)?;
        assert_eq!(
            cartridge.read(0xA111),
            Err(CartridgeIOError::CartridgeRamDisabled)
        );
        assert!(cartridge.controller.has_ram());
        assert_eq!(cartridge.controller.ram_size(), 8192);
        Ok(())
    }

    #[test]
    fn test_mbc1_largerom_rom_bank_switch() -> CartridgeIOResult<()> {
        let mut rom_data = vec![0x12; 1024 * 1024];
        rom_data[0x4001] = 0x33;
        rom_data[0x8001] = 0x99;
        rom_data[0x80001] = 0x34;
        rom_data[0x88001] = 0x66;
        rom_data[CARTRIDGE_TYPE_LOCATION] = 1;
        rom_data[RAM_SIZE_LOCATION] = 0;
        let mut cartridge = Cartridge::from_data(rom_data).unwrap();

        assert_eq!(cartridge.read(0x4001)?, 0x33, "Default to bank 1");
        cartridge.write(0x2001, 2)?;
        assert_eq!(cartridge.read(0x4001)?, 0x99, "Switch to bank 2");
        cartridge.write(0x2001, 0)?;
        assert_eq!(cartridge.read(0x4001)?, 0x33, "Bank 0 mapped to bank 1");
        cartridge.write(0x2001, 1)?;
        assert_eq!(cartridge.read(0x4001)?, 0x33, "Bank 1 mapped to bank 1");
        cartridge.write(0x2001, 0x82)?;
        assert_eq!(
            cartridge.read(0x4001)?,
            0x99,
            "Only bottom 5 bits of ROM select used to select bank (2)"
        );
        cartridge.write(0x4001, 0x1)?;
        assert_eq!(
            cartridge.read(0x4001)?,
            0x66,
            "High select bits used to load ROM > 512 KiB (bank 18)"
        );
        assert_eq!(
            cartridge.read(0x1)?,
            0x34,
            "High select bits used to load static ROM (bank 17)"
        );
        Ok(())
    }

    #[test]
    fn test_mbc1_largeram_rom_bank_switch() -> CartridgeIOResult<()> {
        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[0x4001] = 0x33;
        rom_data[0x8001] = 0x99;
        rom_data[CARTRIDGE_TYPE_LOCATION] = 2;
        rom_data[RAM_SIZE_LOCATION] = 3;
        let mut cartridge = Cartridge::from_data(rom_data).unwrap();
        cartridge.write(0x6001, 1)?;

        assert_eq!(cartridge.read(0x4001)?, 0x33, "Default to bank 1");
        cartridge.write(0x2001, 2)?;
        assert_eq!(cartridge.read(0x4001)?, 0x99, "Switch to bank 2");
        cartridge.write(0x2001, 0)?;
        assert_eq!(cartridge.read(0x4001)?, 0x33, "Bank 0 mapped to bank 1");
        cartridge.write(0x2001, 1)?;
        assert_eq!(cartridge.read(0x4001)?, 0x33, "Bank 1 mapped to bank 1");
        cartridge.write(0x2001, 0x82)?;
        assert_eq!(
            cartridge.read(0x4001)?,
            0x99,
            "Only bottom 5 bits of ROM select used to select bank (2)"
        );
        cartridge.write(0x4001, 0x1)?;
        assert_eq!(
            cartridge.read(0x4001)?,
            0x99,
            "High select bits not used to load ROM > 512 KiB (bank 18)"
        );
        assert_eq!(
            cartridge.read(0x1)?,
            0x12,
            "High select bits not used to load static ROM (bank 17)"
        );
        assert!(cartridge.controller.has_ram());
        assert_eq!(cartridge.controller.ram_size(), 32 * 1024);
        Ok(())
    }

    #[test]
    fn test_mbc1_largeram_ram_bank_switch() -> CartridgeIOResult<()> {
        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 3;
        rom_data[RAM_SIZE_LOCATION] = 3;
        let mut cartridge = Cartridge::from_data(rom_data).unwrap();
        cartridge.write(0x6001, 1)?;
        cartridge.write(0x00ff, 0b1010)?;

        cartridge.write(0xA111, 0x43)?;
        assert_eq!(cartridge.read(0xA111), Ok(0x43));
        cartridge.write(0x4001, 0x1)?;
        assert_ne!(cartridge.read(0xA111), Ok(0x43));
        cartridge.write(0x4001, 0x0)?;
        assert_eq!(cartridge.read(0xA111), Ok(0x43));
        assert!(cartridge.controller.has_ram());
        assert_eq!(cartridge.controller.ram_size(), 32 * 1024);
        Ok(())
    }

    #[test]
    fn test_mbc1_ram_sizes() {
        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 3;
        rom_data[RAM_SIZE_LOCATION] = 1;
        let cartridge = Cartridge::from_data(rom_data).unwrap();

        assert_eq!(cartridge.controller.ram_size(), 2 * 1024);

        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 3;
        rom_data[RAM_SIZE_LOCATION] = 4;
        let cartridge = Cartridge::from_data(rom_data).unwrap();

        assert_eq!(cartridge.controller.ram_size(), 128 * 1024);

        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 3;
        rom_data[RAM_SIZE_LOCATION] = 5;
        let cartridge = Cartridge::from_data(rom_data).unwrap();

        assert_eq!(cartridge.controller.ram_size(), 64 * 1024);

        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 3;
        rom_data[RAM_SIZE_LOCATION] = 6;
        let cartridge_result = Cartridge::from_data(rom_data);

        assert_eq!(
            cartridge_result.err().unwrap(),
            CartridgeLoadError::UnsupportedRamSize(6)
        );
    }

    #[test]
    fn test_mbc2_basic_rom() {
        let mut rom_data = vec![0x12; 96 * 1024];
        rom_data[0x5500] = 0x23;
        rom_data[CARTRIDGE_TYPE_LOCATION] = 5;
        rom_data[RAM_SIZE_LOCATION] = 0;
        let mut cartridge = Cartridge::from_data(rom_data).unwrap();

        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
        assert_eq!(cartridge.read(0x5500).unwrap(), 0x23);
        assert_eq!(
            cartridge.read(0x9222),
            Err(CartridgeIOError::NonCartAddress(0x9222))
        );
        assert_eq!(
            cartridge.read(0xA111),
            Err(CartridgeIOError::CartridgeRamDisabled)
        );
        assert_eq!(cartridge.write(0x4001, 0x55), Ok(()));
        assert!(cartridge.controller.has_ram());
        assert_eq!(cartridge.controller.ram_size(), 512);
    }

    #[test]
    fn test_mbc2_basic_ram() -> CartridgeIOResult<()> {
        let mut rom_data = vec![0x12; 96 * 1024];
        rom_data[CARTRIDGE_TYPE_LOCATION] = 6;
        rom_data[RAM_SIZE_LOCATION] = 0;
        let mut cartridge = Cartridge::from_data(rom_data).unwrap();
        cartridge.write(0x00, 0b1010)?;

        cartridge.write(0xA123, 0xF1)?;
        assert_eq!(cartridge.read(0xA123), Ok(0x1), "Bottom nibble only stored");
        assert_eq!(
            cartridge.read(0xA323),
            Ok(0x1),
            "RAM repeats through address space"
        );
        cartridge.write(0x00, 0b1000)?;
        assert_eq!(
            cartridge.read(0xA111),
            Err(CartridgeIOError::CartridgeRamDisabled)
        );
        assert!(cartridge.controller.has_ram());
        assert_eq!(cartridge.controller.ram_size(), 512);
        Ok(())
    }

    #[test]
    fn test_mbc2_rom_bank_switching() -> CartridgeIOResult<()> {
        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[0x4001] = 0x33;
        rom_data[0x8001] = 0x99;
        rom_data[CARTRIDGE_TYPE_LOCATION] = 6;
        rom_data[RAM_SIZE_LOCATION] = 0;
        let mut cartridge = Cartridge::from_data(rom_data).unwrap();

        assert_eq!(cartridge.read(0x4001).unwrap(), 0x33);
        cartridge.write(0x100, 0b10)?;
        assert_eq!(cartridge.read(0x4001).unwrap(), 0x99);
        Ok(())
    }

    #[test]
    fn test_target_detection() {
        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[RAM_SIZE_LOCATION] = 0;
        rom_data[CARTRIDGE_TYPE_LOCATION] = 6;
        let cartridge = Cartridge::from_data(rom_data.clone()).unwrap();

        assert_eq!(cartridge.target, TargetConsole::GameBoyOnly);

        rom_data[TARGET_CONSOLE_LOCATION] = 0xC0;
        let cartridge = Cartridge::from_data(rom_data.clone()).unwrap();

        assert_eq!(cartridge.target, TargetConsole::ColorOnly);

        rom_data[TARGET_CONSOLE_LOCATION] = 0x80;
        let cartridge = Cartridge::from_data(rom_data.clone()).unwrap();

        assert_eq!(cartridge.target, TargetConsole::ColorEnhanced);
    }
}
