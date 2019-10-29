use alloc::vec::Vec;
use core::convert::TryFrom;
use core::ops::Range;
use enum_dispatch::enum_dispatch;

const STATIC_ROM: Range<u16> = 0x0000..0x4000;
const SWITCHABLE_ROM: Range<u16> = 0x4000..0x8000;
const SWITCHABLE_RAM: Range<u16> = 0xA000..0xBFFF;

const ROM_BANK_SIZE: u16 = 0x4000;
const RAM_BANK_SIZE: u16 = 0x2000;


#[derive(PartialEq, Eq, Debug)]
pub enum CartridgeError {
    NonRomAddress,
    InvalidBank,
    NoDataInRom,
    NoCartridgeRam,
    UnsupportedCartridgeType
}


pub type CartridgeResult<T> = Result<T, CartridgeError>;


pub struct Cartridge {
    data: Vec<u8>,
    controller: CartridgeEnum
}

impl Cartridge {
    pub fn read(&self, loc: u16) -> CartridgeResult<u8> {
        if STATIC_ROM.contains(&loc) {
            self.controller.read_static_rom(loc, &self.data)
        } else if SWITCHABLE_ROM.contains(&loc) {
            self.controller.read_switchable_rom(loc, &self.data)
        } else if SWITCHABLE_RAM.contains(&loc) {
            self.controller.read_switchable_ram(loc)
        } else {
            Err(CartridgeError::NonRomAddress)
        }
    }

    pub fn write(&mut self, loc: u16, value: u8) -> CartridgeResult<()> {
        self.controller.write(loc, value)
    }

    pub fn from_data(data: Vec<u8>) -> CartridgeResult<Cartridge> {
        let cartridge_type_id = data[0x147];
        let ram_size = data[0x149];
        match cartridge_type_id {
            0 => Ok(Cartridge {
                controller: StaticRom.into(),
                data
            }),
            1 => Ok(Cartridge {
                controller: MBC1::new(ram_size).into(),
                data
            }),
            2 | 3 => Ok(Cartridge {
                controller: MBC1::new(ram_size).into(),
                data
            }),
            5 | 6 => Ok(Cartridge {
                controller: MBC2::default().into(),
                data
            }),
            _ => Err(CartridgeError::UnsupportedCartridgeType)
        }
    }
}


#[enum_dispatch]
pub enum CartridgeEnum {
    StaticRom,
    Type1(MBC1),
    Type2(MBC2),
    //Type3(MBC3)
}


#[enum_dispatch(CartridgeEnum)]
trait CartridgeAccess {
    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8>;
    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8>;
    fn read_switchable_ram(&self, loc: u16) -> CartridgeResult<u8>;
    fn write(&mut self, loc: u16, value: u8) -> CartridgeResult<()>;
}

pub struct StaticRom;

impl CartridgeAccess for StaticRom {

    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        rom.get(usize::from(loc))
            .copied()
            .ok_or(CartridgeError::NonRomAddress)
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
}


#[derive(PartialEq, Eq, Debug)]
enum MBC1PageMode {
    LargeRom,
    LargeRam
}


pub struct MBC1 {
    page_mode: MBC1PageMode,
    selected_rom: u8,
    selected_high: u8,
    ram_enabled: bool,
    ram: Vec<u8>
}

impl MBC1 {
    fn new(ram_size_id: u8) -> MBC1 {
        let ram_size = (32 << ram_size_id) * 1024;
        MBC1 {
            page_mode: MBC1PageMode::LargeRom,
            selected_rom: 1,
            selected_high: 0,
            ram_enabled: false,
            ram: vec![0x00; ram_size]
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


impl CartridgeAccess for MBC1 {

    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        let bank = u32::from(self.selected_static_rom_bank());
        let rom_addr = (bank * u32::from(ROM_BANK_SIZE)) + u32::from(loc);
        rom.get(usize::try_from(rom_addr).expect("ROM too large for host platform"))
            .copied()
            .ok_or(CartridgeError::NoDataInRom)
    }    
    
    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        let bank_addr = loc - SWITCHABLE_ROM.start;
        let bank = u32::from(self.selected_rom_bank());
        let rom_addr = (bank * u32::from(ROM_BANK_SIZE)) + u32::from(bank_addr);
        rom.get(usize::try_from(rom_addr).expect("ROM too large for host platform"))
            .copied()
            .ok_or(CartridgeError::NoDataInRom)
    }

    fn read_switchable_ram(&self, loc: u16) -> CartridgeResult<u8> {
        if self.ram_enabled {
            let bank = u16::from(self.selected_ram_bank());
            let ram_addr = (bank * RAM_BANK_SIZE) + (loc - SWITCHABLE_RAM.start);
            Ok(self.ram[usize::from(ram_addr)])
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
        } else if SWITCHABLE_RAM.contains(&loc) {
            if self.ram_enabled {
                let ram_addr = loc - SWITCHABLE_RAM.start;
                self.ram[usize::from(ram_addr)] = value;
            }
            Ok(())
        } else {
            unreachable!()
        }
    }
}


pub struct MBC2 {
    selected_rom: u8,
    ram_enabled: bool,
    ram: Vec<u8>
}

impl Default for MBC2 {
    fn default() -> Self {
        MBC2 {
            selected_rom: 1,
            ram_enabled: false,
            ram: vec![0x00; 512]
        }
    }
}


impl MBC2 {
    fn selected_rom_bank(&self) -> u8 {
        let bank_id = self.selected_rom & 0xF;
        if bank_id == 0 { 1 } else { bank_id }
    }
}


impl CartridgeAccess for MBC2 {

    fn read_static_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        Ok(rom[usize::from(loc)])
    }    
    
    fn read_switchable_rom(&self, loc: u16, rom: &[u8]) -> CartridgeResult<u8> {
        let bank_addr = loc - SWITCHABLE_ROM.start;
        let bank = u16::from(self.selected_rom_bank());
        let rom_addr = (bank * ROM_BANK_SIZE) + bank_addr;
        rom.get(usize::from(rom_addr))
            .copied()
            .ok_or(CartridgeError::NoDataInRom)
    }

    fn read_switchable_ram(&self, loc: u16) -> CartridgeResult<u8> {
        if !self.ram_enabled {
            Err(CartridgeError::NoCartridgeRam)
        } else {
            let wrapped_ram_addr = (loc - SWITCHABLE_RAM.start) % 0x200;
            Ok(self.ram[usize::from(wrapped_ram_addr)])
        }
    }

    fn write(&mut self, loc: u16, value: u8) -> CartridgeResult<()> {
        if STATIC_ROM.contains(&loc) {
            if loc & 0x100 == 0x100 {
                self.selected_rom = value & 0xF;
            } else {
                self.ram_enabled = value == 0b1010;
            }
            Ok(())
        } else if SWITCHABLE_RAM.contains(&loc) && self.ram_enabled {
            let ram_addr = loc - SWITCHABLE_RAM.start;
            let wrapped_ram_addr = ram_addr % 0x200;
            self.ram[usize::from(wrapped_ram_addr)] = value & 0xF;
            Ok(())
        } else {
            Ok(())
        }
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
        rom_data[0x5500] = 0x23;
        let mut cartridge = Cartridge {
            data: rom_data,
            controller: StaticRom.into()
        };

        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
        assert_eq!(cartridge.read(0x5500).unwrap(), 0x23);
        assert_eq!(cartridge.read(0xA111), Err(CartridgeError::NoCartridgeRam));
        assert_eq!(cartridge.read(0x9222), Err(CartridgeError::NonRomAddress));
        assert_eq!(cartridge.write(0x1234, 0x22), Ok(()));
        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
    }

    #[test]
    fn test_mbc1_large_rom_basic_rom() {
        let mut rom_data = vec![0x12; 96 * 1024];
        rom_data[0x5500] = 0x23;
        let cartridge = Cartridge {
            data: rom_data,
            controller: MBC1::new(0).into()
        };

        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
        assert_eq!(cartridge.read(0x5500).unwrap(), 0x23);
        assert_eq!(cartridge.read(0x9222), Err(CartridgeError::NonRomAddress));
        assert_eq!(cartridge.read(0xA111), Err(CartridgeError::NoCartridgeRam));
    }
    
    #[test]
    fn test_mbc1_large_rom_basic_ram() -> CartridgeResult<()> {
        let rom_data = vec![0x12; 96 * 1024];
        let mut cartridge = Cartridge {
            data: rom_data,
            controller: MBC1::new(0).into()
        };

        cartridge.write(0x00ff, 0b1010)?;
        cartridge.write(0xA111, 0x20)?;
        assert_eq!(cartridge.read(0xA111)?, 0x20);
        cartridge.write(0x00ff, 0b1000)?;
        cartridge.write(0xA111, 0x20)?;
        assert_eq!(cartridge.read(0xA111), Err(CartridgeError::NoCartridgeRam));
        Ok(())
    }
    
    #[test]
    fn test_mbc1_largerom_rom_bank_switch() -> CartridgeResult<()> {
        let mut rom_data = vec![0x12; 1024 * 1024];
        rom_data[0x4001] = 0x33;
        rom_data[0x8001] = 0x99;
        rom_data[0x80001] = 0x34;
        rom_data[0x88001] = 0x66;
        let mut cartridge = Cartridge {
            data: rom_data,
            controller: MBC1::new(0).into()
        };

        assert_eq!(cartridge.read(0x4001)?, 0x33, "Default to bank 1");
        cartridge.write(0x2001, 2)?;
        assert_eq!(cartridge.read(0x4001)?, 0x99, "Switch to bank 2");
        cartridge.write(0x2001, 0)?;
        assert_eq!(cartridge.read(0x4001)?, 0x33, "Bank 0 mapped to bank 1");
        cartridge.write(0x2001, 1)?;
        assert_eq!(cartridge.read(0x4001)?, 0x33, "Bank 1 mapped to bank 1");
        cartridge.write(0x2001, 0x82)?;
        assert_eq!(cartridge.read(0x4001)?, 0x99, "Only bottom 5 bits of ROM select used to select bank (2)");
        cartridge.write(0x4001, 0x1)?;
        assert_eq!(cartridge.read(0x4001)?, 0x66, "High select bits used to load ROM > 512 KiB (bank 18)");
        assert_eq!(cartridge.read(0x1)?, 0x34, "High select bits used to load static ROM (bank 17)");
        Ok(())
    }
    
    #[test]
    fn test_mbc1_largeram_rom_bank_switch() -> CartridgeResult<()> {
        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[0x4001] = 0x33;
        rom_data[0x8001] = 0x99;
        let mut cartridge = Cartridge {
            data: rom_data,
            controller: MBC1::new(3).into()
        };
        cartridge.write(0x6001, 1)?;

        assert_eq!(cartridge.read(0x4001)?, 0x33, "Default to bank 1");
        cartridge.write(0x2001, 2)?;
        assert_eq!(cartridge.read(0x4001)?, 0x99, "Switch to bank 2");
        cartridge.write(0x2001, 0)?;
        assert_eq!(cartridge.read(0x4001)?, 0x33, "Bank 0 mapped to bank 1");
        cartridge.write(0x2001, 1)?;
        assert_eq!(cartridge.read(0x4001)?, 0x33, "Bank 1 mapped to bank 1");
        cartridge.write(0x2001, 0x82)?;
        assert_eq!(cartridge.read(0x4001)?, 0x99, "Only bottom 5 bits of ROM select used to select bank (2)");
        cartridge.write(0x4001, 0x1)?;
        assert_eq!(cartridge.read(0x4001)?, 0x99, "High select bits not used to load ROM > 512 KiB (bank 18)");
        assert_eq!(cartridge.read(0x1)?, 0x12, "High select bits not used to load static ROM (bank 17)");
        Ok(())
    }
    
    #[test]
    fn test_mbc1_largeram_ram_bank_switch() -> CartridgeResult<()> {
        let rom_data = vec![0x12; 512 * 1024];
        let mut cartridge = Cartridge {
            data: rom_data,
            controller: MBC1::new(3).into()
        };
        cartridge.write(0x6001, 1)?;
        cartridge.write(0x00ff, 0b1010)?;

        cartridge.write(0xA111, 0x43)?;
        assert_eq!(cartridge.read(0xA111), Ok(0x43));
        cartridge.write(0x4001, 0x1)?;
        assert_ne!(cartridge.read(0xA111), Ok(0x43));
        cartridge.write(0x4001, 0x0)?;
        assert_eq!(cartridge.read(0xA111), Ok(0x43));
        Ok(())
    }

    #[test]
    fn test_mbc2_basic_rom() {
        let mut rom_data = vec![0x12; 96 * 1024];
        rom_data[0x5500] = 0x23;
        let mut cartridge = Cartridge {
            data: rom_data,
            controller: MBC2::default().into()
        };

        assert_eq!(cartridge.read(0x1234).unwrap(), 0x12);
        assert_eq!(cartridge.read(0x5500).unwrap(), 0x23);
        assert_eq!(cartridge.read(0x9222), Err(CartridgeError::NonRomAddress));
        assert_eq!(cartridge.read(0xA111), Err(CartridgeError::NoCartridgeRam));
        assert_eq!(cartridge.write(0x4001, 0x55), Ok(()));
    }

    #[test]
    fn test_mbc2_basic_ram() -> CartridgeResult<()> {
        let rom_data = vec![0x12; 96 * 1024];
        let mut cartridge = Cartridge {
            data: rom_data,
            controller: MBC2::default().into()
        };
        cartridge.write(0x00, 0b1010)?;

        cartridge.write(0xA123, 0xF1)?;
        assert_eq!(cartridge.read(0xA123), Ok(0x1), "Bottom nibble only stored");
        assert_eq!(cartridge.read(0xA323), Ok(0x1), "RAM repeats through address space");
        cartridge.write(0x00, 0b1000)?;
        assert_eq!(cartridge.read(0xA111), Err(CartridgeError::NoCartridgeRam));
        Ok(())
    }

    #[test]
    fn test_mbc2_rom_bank_switching() -> CartridgeResult<()> {
        let mut rom_data = vec![0x12; 512 * 1024];
        rom_data[0x4001] = 0x33;
        rom_data[0x8001] = 0x99;
        let mut cartridge = Cartridge {
            data: rom_data,
            controller: MBC2::default().into()
        };


        assert_eq!(cartridge.read(0x4001).unwrap(), 0x33);
        cartridge.write(0x100, 0b10)?;
        assert_eq!(cartridge.read(0x4001).unwrap(), 0x99);
        Ok(())
    }
}