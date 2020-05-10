use crate::gameboy::memory;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::ops::Range;
use derive_more::Display;

const TARGET_CONSOLE_LOCATION: usize = 0x143;
const CARTRIDGE_TYPE_LOCATION: usize = 0x147;
const RAM_SIZE_LOCATION: usize = 0x149;

#[derive(PartialEq, Eq, Debug, Display)]
pub enum CartridgeError {
    #[display(fmt = "Address 0x{:X} exceeds ROM", "_0")]
    NoDataInRom(u16),
    #[display(fmt = "Cannot read non-cart address 0x{:X} from cartridge", "_0")]
    NonCartAddress(u16),
    #[display(fmt = "RAM not supported by current cartridge")]
    NoCartridgeRam,
    #[display(fmt = "RAM disabled on current cartridge")]
    CartridgeRamDisabled,
    #[display(fmt = "Address 0x{:X} outside of available cart ram", "_0")]
    ExceedsCartridgeRam(u16),
    #[display(fmt = "Unsupported cartridge type: 0x{:X}", "_0")]
    UnsupportedCartridgeType(u8),
    #[display(fmt = "Unsupported cartridge RAM size: 0x{:X}", "_0")]
    UnsupportedRamSize(u8),
    #[display(
        fmt = "Data for cartridge too small. Was 0x{:X} bytes, must be at least 0x200",
        "_0"
    )]
    CartridgeTooSmall(usize),
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum TargetConsole {
    GameBoyOnly,
    ColorEnhanced,
    ColorOnly,
}

#[cfg(feature = "std")]
impl std::error::Error for CartridgeError {}

pub type CartridgeResult<T> = Result<T, CartridgeError>;

pub struct Cartridge {
    pub data: Vec<u8>,
    pub controller: CartridgeEnum,
    pub target: TargetConsole,
}

impl Cartridge {
    pub fn read(&self, loc: u16) -> CartridgeResult<u8> {
        if memory::STATIC_ROM.contains(loc) {
            self.controller.read_static_rom(loc, &self.data)
        } else if memory::SWITCHABLE_ROM.contains(loc) {
            self.controller.read_switchable_rom(loc, &self.data)
        } else if memory::CARTRIDGE_RAM.contains(loc) {
            self.controller.read_switchable_ram(loc)
        } else {
            Err(CartridgeError::NonCartAddress(loc))
        }
    }

    pub fn write(&mut self, loc: u16, value: u8) -> CartridgeResult<()> {
        self.controller.write(loc, value)
    }

    pub fn from_data(data: Vec<u8>) -> CartridgeResult<Cartridge> {
        if data.len() < 0x200 {
            return Err(CartridgeError::CartridgeTooSmall(data.len()));
        }
        let cartridge_type_id = data[CARTRIDGE_TYPE_LOCATION];
        let ram_size = lookup_ram_size(data[RAM_SIZE_LOCATION])?;
        let target = lookup_target(data[TARGET_CONSOLE_LOCATION]);
        match cartridge_type_id {
            0 => Ok(Cartridge {
                controller: StaticRom.into(),
                data,
                target,
            }),
            1 => Ok(Cartridge {
                controller: MBC1::new(0, false).into(),
                data,
                target,
            }),
            2 | 3 => Ok(Cartridge {
                controller: MBC1::new(ram_size, true).into(),
                data,
                target,
            }),
            5 | 6 => Ok(Cartridge {
                controller: MBC2::default().into(),
                data,
                target,
            }),
            _ => Err(CartridgeError::UnsupportedCartridgeType(cartridge_type_id)),
        }
    }
}

pub enum CartridgeEnum {
    StaticRom(StaticRom),
    Type1(MBC1),
    Type2(MBC2),
    //Type3(MBC3)
}

pub trait CartridgeType {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8>;
    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8>;
    fn read_switchable_ram(&self, loc: u16) -> CartridgeResult<u8>;
    fn write(&mut self, loc: u16, value: u8) -> CartridgeResult<()>;
    fn has_ram(&self) -> bool;
    fn ram_size(&self) -> usize;
}

impl CartridgeType for CartridgeEnum {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        match self {
            CartridgeEnum::StaticRom(srom) => srom.read_static_rom(loc, rom),
            CartridgeEnum::Type1(mbc1) => mbc1.read_static_rom(loc, rom),
            CartridgeEnum::Type2(mbc2) => mbc2.read_static_rom(loc, rom),
        }
    }

    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        match self {
            CartridgeEnum::StaticRom(srom) => srom.read_switchable_rom(loc, rom),
            CartridgeEnum::Type1(mbc1) => mbc1.read_switchable_rom(loc, rom),
            CartridgeEnum::Type2(mbc2) => mbc2.read_switchable_rom(loc, rom),
        }
    }

    fn read_switchable_ram(&self, loc: u16) -> CartridgeResult<u8> {
        match self {
            CartridgeEnum::StaticRom(srom) => srom.read_switchable_ram(loc),
            CartridgeEnum::Type1(mbc1) => mbc1.read_switchable_ram(loc),
            CartridgeEnum::Type2(mbc2) => mbc2.read_switchable_ram(loc),
        }
    }

    fn write(&mut self, loc: u16, value: u8) -> CartridgeResult<()> {
        match self {
            CartridgeEnum::StaticRom(srom) => srom.write(loc, value),
            CartridgeEnum::Type1(mbc1) => mbc1.write(loc, value),
            CartridgeEnum::Type2(mbc2) => mbc2.write(loc, value),
        }
    }

    fn has_ram(&self) -> bool {
        match self {
            CartridgeEnum::StaticRom(srom) => srom.has_ram(),
            CartridgeEnum::Type1(mbc1) => mbc1.has_ram(),
            CartridgeEnum::Type2(mbc2) => mbc2.has_ram(),
        }
    }

    fn ram_size(&self) -> usize {
        match self {
            CartridgeEnum::StaticRom(srom) => srom.ram_size(),
            CartridgeEnum::Type1(mbc1) => mbc1.ram_size(),
            CartridgeEnum::Type2(mbc2) => mbc2.ram_size(),
        }
    }
}

pub struct StaticRom;

impl CartridgeType for StaticRom {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        rom.get(usize::from(loc))
            .copied()
            .ok_or(CartridgeError::NoDataInRom(loc))
    }

    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        self.read_static_rom(loc, rom)
    }

    fn read_switchable_ram(&self, _loc: u16) -> CartridgeResult<u8> {
        Err(CartridgeError::NoCartridgeRam)
    }

    fn write(&mut self, _loc: u16, _value: u8) -> CartridgeResult<()> {
        Ok(())
    }

    fn has_ram(&self) -> bool {
        false
    }

    fn ram_size(&self) -> usize {
        0
    }
}

impl From<StaticRom> for CartridgeEnum {
    fn from(srom: StaticRom) -> Self {
        CartridgeEnum::StaticRom(srom)
    }
}

#[derive(PartialEq, Eq, Debug)]
enum MBC1PageMode {
    LargeRom,
    LargeRam,
}

pub struct MBC1 {
    page_mode: MBC1PageMode,
    selected_rom: u8,
    selected_high: u8,
    ram_enabled: bool,
    has_ram: bool,
    ram: Vec<u8>,
}

impl MBC1 {
    fn new(ram_size: usize, has_ram: bool) -> MBC1 {
        MBC1 {
            page_mode: MBC1PageMode::LargeRom,
            selected_rom: 1,
            selected_high: 0,
            ram_enabled: false,
            has_ram,
            ram: vec![0x00; ram_size],
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

impl CartridgeType for MBC1 {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        let bank = u32::from(self.selected_static_rom_bank());
        let rom_addr = (bank * u32::from(memory::STATIC_ROM.len)) + u32::from(loc);
        rom.get(usize::try_from(rom_addr).expect("ROM too large for host platform"))
            .copied()
            .ok_or(CartridgeError::NoDataInRom(loc))
    }

    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        let bank_addr = loc - memory::SWITCHABLE_ROM.start;
        let bank = u32::from(self.selected_rom_bank());
        let rom_addr = (bank * u32::from(memory::SWITCHABLE_ROM.start)) + u32::from(bank_addr);
        rom.get(usize::try_from(rom_addr).expect("ROM too large for host platform"))
            .copied()
            .ok_or(CartridgeError::NoDataInRom(loc))
    }

    fn read_switchable_ram(&self, loc: u16) -> CartridgeResult<u8> {
        if self.has_ram && self.ram_enabled {
            let bank = u16::from(self.selected_ram_bank());
            let ram_addr = (bank * memory::CARTRIDGE_RAM.len) + (loc - memory::CARTRIDGE_RAM.start);
            Ok(self.ram[usize::from(ram_addr)])
        } else if self.has_ram {
            Err(CartridgeError::CartridgeRamDisabled)
        } else {
            Err(CartridgeError::NoCartridgeRam)
        }
    }

    fn write(&mut self, loc: u16, value: u8) -> CartridgeResult<()> {
        if MBC1::ram_enable_area().contains(&loc) {
            self.ram_enabled = value == 0b1010;
            Ok(())
        } else if MBC1::rom_select_area().contains(&loc) {
            self.selected_rom = value & 0x1F;
            if self.selected_rom == 0 {
                self.selected_rom = 1
            }
            Ok(())
        } else if MBC1::high_select_area().contains(&loc) {
            self.selected_high = value & 0x3;
            Ok(())
        } else if MBC1::mode_select_area().contains(&loc) {
            if value == 0x00 {
                self.page_mode = MBC1PageMode::LargeRom
            } else {
                self.page_mode = MBC1PageMode::LargeRam
            }
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

    fn ram_size(&self) -> usize {
        self.ram.len()
    }
}

impl From<MBC1> for CartridgeEnum {
    fn from(mbc: MBC1) -> Self {
        CartridgeEnum::Type1(mbc)
    }
}

pub struct MBC2 {
    selected_rom: u8,
    ram_enabled: bool,
    ram: Vec<u8>,
}

impl Default for MBC2 {
    fn default() -> Self {
        MBC2 {
            selected_rom: 1,
            ram_enabled: false,
            ram: vec![0x00; 512],
        }
    }
}

impl MBC2 {
    fn selected_rom_bank(&self) -> u8 {
        let bank_id = self.selected_rom & 0xF;
        if bank_id == 0 {
            1
        } else {
            bank_id
        }
    }
}

impl CartridgeType for MBC2 {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        Ok(rom[usize::from(loc)])
    }

    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        let bank_addr = loc - memory::SWITCHABLE_ROM.start;
        let bank = u16::from(self.selected_rom_bank());
        let rom_addr = (bank * memory::SWITCHABLE_ROM.len) + bank_addr;
        rom.get(usize::from(rom_addr))
            .copied()
            .ok_or(CartridgeError::NoDataInRom(loc))
    }

    fn read_switchable_ram(&self, loc: u16) -> CartridgeResult<u8> {
        if !self.ram_enabled {
            Err(CartridgeError::CartridgeRamDisabled)
        } else {
            let wrapped_ram_addr = (loc - memory::CARTRIDGE_RAM.start) % 0x200;
            Ok(self.ram[usize::from(wrapped_ram_addr)])
        }
    }

    fn write(&mut self, loc: u16, value: u8) -> CartridgeResult<()> {
        if memory::STATIC_ROM.contains(loc) {
            if loc & 0x100 == 0x100 {
                self.selected_rom = value & 0xF;
            } else {
                self.ram_enabled = value == 0b1010;
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

    fn ram_size(&self) -> usize {
        512
    }
}

impl From<MBC2> for CartridgeEnum {
    fn from(mbc: MBC2) -> Self {
        CartridgeEnum::Type2(mbc)
    }
}

fn lookup_ram_size(ram_size_id: u8) -> CartridgeResult<usize> {
    match ram_size_id {
        0 => Ok(0),
        1 => Ok(2 * 1024),
        2 => Ok(8 * 1024),
        3 => Ok(32 * 1024),
        4 => Ok(128 * 1024),
        5 => Ok(64 * 1024),
        _ => Err(CartridgeError::UnsupportedRamSize(ram_size_id)),
    }
}

fn lookup_target(target_id: u8) -> TargetConsole {
    match target_id {
        0xC0 => TargetConsole::ColorOnly,
        0x80 => TargetConsole::ColorEnhanced,
        _ => TargetConsole::GameBoyOnly,
    }
}

/*pub struct MBC3 {
    selected_rom_bank: u8
}*/

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
        assert_eq!(cartridge.read(0xA111), Err(CartridgeError::NoCartridgeRam));
        assert_eq!(
            cartridge.read(0x9222),
            Err(CartridgeError::NonCartAddress(0x9222))
        );
        assert_eq!(cartridge.write(0x1234, 0x22), Ok(()));
        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
        assert_eq!(cartridge.controller.has_ram(), false);
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
            Err(CartridgeError::NonCartAddress(0x9222))
        );
        assert_eq!(cartridge.read(0xA111), Err(CartridgeError::NoCartridgeRam));
        assert_eq!(cartridge.controller.has_ram(), false);
        assert_eq!(cartridge.controller.ram_size(), 0);
    }

    #[test]
    fn test_mbc1_large_rom_basic_ram() -> CartridgeResult<()> {
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
            Err(CartridgeError::CartridgeRamDisabled)
        );
        assert_eq!(cartridge.controller.has_ram(), true);
        assert_eq!(cartridge.controller.ram_size(), 8192);
        Ok(())
    }

    #[test]
    fn test_mbc1_largerom_rom_bank_switch() -> CartridgeResult<()> {
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
    fn test_mbc1_largeram_rom_bank_switch() -> CartridgeResult<()> {
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
        assert_eq!(cartridge.controller.has_ram(), true);
        assert_eq!(cartridge.controller.ram_size(), 32 * 1024);
        Ok(())
    }

    #[test]
    fn test_mbc1_largeram_ram_bank_switch() -> CartridgeResult<()> {
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
        assert_eq!(cartridge.controller.has_ram(), true);
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
            CartridgeError::UnsupportedRamSize(6)
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
            Err(CartridgeError::NonCartAddress(0x9222))
        );
        assert_eq!(
            cartridge.read(0xA111),
            Err(CartridgeError::CartridgeRamDisabled)
        );
        assert_eq!(cartridge.write(0x4001, 0x55), Ok(()));
        assert_eq!(cartridge.controller.has_ram(), true);
        assert_eq!(cartridge.controller.ram_size(), 512);
    }

    #[test]
    fn test_mbc2_basic_ram() -> CartridgeResult<()> {
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
            Err(CartridgeError::CartridgeRamDisabled)
        );
        assert_eq!(cartridge.controller.has_ram(), true);
        assert_eq!(cartridge.controller.ram_size(), 512);
        Ok(())
    }

    #[test]
    fn test_mbc2_rom_bank_switching() -> CartridgeResult<()> {
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
