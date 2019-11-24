use crate::registers;
use crate::rom;

pub use crate::registers::{ByteRegister, WordRegister};

pub struct GameBoyCPU {
    af_register: u16,
    bc_register: u16,
    de_register: u16,
    hl_register: u16,
    sp_register: u16,
    pc_register: u16,
    sysram: [u8; 0x2000],
    vram: [u8; 0x2000],
    cpuram: [u8; 0x200],
    cartridge: rom::Cartridge,
}

#[derive(PartialEq, Eq, Debug)]
pub enum MemoryError {
    InvalidRomAddress(u16),
    InvalidRamAddress(u16),
}

pub type MemoryResult<T> = Result<T, MemoryError>;

impl GameBoyCPU {
    pub fn new(cartridge: rom::Cartridge) -> GameBoyCPU {
        GameBoyCPU {
            af_register: 0,
            bc_register: 0,
            de_register: 0,
            hl_register: 0,
            sp_register: 0,
            pc_register: 0x100,
            sysram: [0u8; 0x2000],
            vram: [0u8; 0x2000],
            cpuram: [0u8; 0x200],
            cartridge,
        }
    }

    pub fn step(&mut self) {}

    pub fn read_register_u8(&self, reg: registers::ByteRegister) -> u8 {
        match reg {
            registers::ByteRegister::A => self.af_register.to_le_bytes()[1],
            registers::ByteRegister::F => self.af_register.to_le_bytes()[0],
            registers::ByteRegister::B => self.bc_register.to_le_bytes()[1],
            registers::ByteRegister::C => self.bc_register.to_le_bytes()[0],
            registers::ByteRegister::D => self.de_register.to_le_bytes()[1],
            registers::ByteRegister::E => self.de_register.to_le_bytes()[0],
            registers::ByteRegister::H => self.hl_register.to_le_bytes()[1],
            registers::ByteRegister::L => self.hl_register.to_le_bytes()[0],
        }
    }

    pub fn read_register_u16(&self, reg: registers::WordRegister) -> u16 {
        match reg {
            registers::WordRegister::AF => self.af_register.to_le(),
            registers::WordRegister::BC => self.bc_register.to_le(),
            registers::WordRegister::DE => self.de_register.to_le(),
            registers::WordRegister::HL => self.hl_register.to_le(),
            registers::WordRegister::SP => self.sp_register.to_le(),
            registers::WordRegister::PC => self.pc_register.to_le(),
        }
    }

    pub fn write_register_u8(&mut self, reg: registers::ByteRegister, value: u8) {
        let full_register = reg.lookup_word_register();
        let byte = reg.lookup_byte();

        match byte {
            registers::WordByte::High => self.write_high_register_byte(full_register, value),
            registers::WordByte::Low => self.write_low_register_byte(full_register, value),
        }
    }

    fn write_high_register_byte(
        &mut self,
        full_register: registers::WordRegister,
        value_to_write: u8,
    ) {
        let register_value = self.read_register_u16(full_register);
        let u16_value = u16::from(value_to_write);
        let masked_register_value = register_value & 0x00ff;
        let shifted_value = u16_value << 8;
        self.write_register_raw(full_register, masked_register_value + shifted_value);
    }

    fn write_low_register_byte(
        &mut self,
        full_register: registers::WordRegister,
        value_to_write: u8,
    ) {
        let register_value = self.read_register_u16(full_register);
        let u16_value = u16::from(value_to_write);
        let masked_register_value = register_value & 0xff00;
        self.write_register_raw(full_register, masked_register_value + u16_value);
    }

    fn write_register_raw(&mut self, reg: registers::WordRegister, value: u16) {
        match reg {
            registers::WordRegister::AF => self.af_register = value & 0xfff0,
            registers::WordRegister::BC => self.bc_register = value,
            registers::WordRegister::DE => self.de_register = value,
            registers::WordRegister::HL => self.hl_register = value,
            registers::WordRegister::SP => self.sp_register = value,
            registers::WordRegister::PC => self.pc_register = value,
        }
    }

    pub fn write_register_u16(&mut self, reg: registers::WordRegister, value: u16) {
        self.write_register_raw(reg, value.to_le());
    }

    pub fn read_memory_u8(&self, addr: u16) -> MemoryResult<u8> {
        if addr < 0x8000 {
            self.cartridge
                .read(addr)
                .map_err(|_| MemoryError::InvalidRomAddress(addr))
        } else if addr <= 0x9fff {
            Ok(self.vram[(addr - 0x8000) as usize])
        } else if addr <= 0xbfff {
            self.cartridge
                .read(addr)
                .map_err(|_| MemoryError::InvalidRamAddress(addr))
        } else if addr <= 0xdfff {
            Ok(self.sysram[(addr - 0xc000) as usize])
        } else if addr <= 0xfdff {
            Ok(self.sysram[(addr - 0xe000) as usize])
        } else {
            Ok(self.cpuram[(addr - 0xfe00) as usize])
        }
    }

    pub fn read_memory_i8(&self, addr: u16) -> MemoryResult<i8> {
        Ok(i8::from_le_bytes([self.read_memory_u8(addr)?]))
    }

    pub fn read_memory_u16(&self, addr: u16) -> MemoryResult<u16> {
        Ok(u16::from_le_bytes([
            self.read_memory_u8(addr)?,
            self.read_memory_u8(addr.wrapping_add(1))?,
        ]))
    }

    pub fn write_memory_u8(&mut self, addr: u16, value: u8) -> MemoryResult<()> {
        if addr < 0x8000 {
            self.cartridge
                .write(addr, value)
                .map_err(|_| MemoryError::InvalidRomAddress(addr))
        } else if addr <= 0x9fff {
            self.vram[(addr - 0x8000) as usize] = value;
            Ok(())
        } else if addr <= 0xbfff {
            self.cartridge
                .write(addr, value)
                .map_err(|_| MemoryError::InvalidRamAddress(addr))
        } else if addr <= 0xdfff {
            self.sysram[(addr - 0xc000) as usize] = value;
            Ok(())
        } else if addr <= 0xfdff {
            self.sysram[(addr - 0xe000) as usize] = value;
            Ok(())
        } else {
            self.cpuram[(addr - 0xfe00) as usize] = value;
            Ok(())
        }
    }

    pub fn write_memory_u16(&mut self, addr: u16, value: u16) -> MemoryResult<()> {
        let bytes = value.to_le_bytes();

        self.write_memory_u8(addr, bytes[0])?;
        self.write_memory_u8(addr.wrapping_add(1), bytes[1])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cartridge() -> rom::Cartridge {
        rom::Cartridge::from_data(vec![0u8; 0x8000]).unwrap()
    }

    #[test]
    fn test_reg_write_u8_read_u8() {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_register_u8(registers::ByteRegister::A, 0x01);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::A), 0x01);

        cpu.write_register_u8(registers::ByteRegister::B, 0x02);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::B), 0x02);

        cpu.write_register_u8(registers::ByteRegister::C, 0x03);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::C), 0x03);

        cpu.write_register_u8(registers::ByteRegister::D, 0x04);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::D), 0x04);

        cpu.write_register_u8(registers::ByteRegister::E, 0x05);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::E), 0x05);

        cpu.write_register_u8(registers::ByteRegister::F, 0x66);
        // F register lower 4 bytes are not writable
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::F), 0x60);

        cpu.write_register_u8(registers::ByteRegister::H, 0x07);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::H), 0x07);

        cpu.write_register_u8(registers::ByteRegister::L, 0x08);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::L), 0x08);
    }

    #[test]
    fn test_reg_write_u16_read_u16() {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_register_u16(registers::WordRegister::AF, 0x1234);
        // F register lower 4 bytes are not writable
        assert_eq!(cpu.read_register_u16(registers::WordRegister::AF), 0x1230);

        cpu.write_register_u16(registers::WordRegister::BC, 0x1235);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::BC), 0x1235);

        cpu.write_register_u16(registers::WordRegister::DE, 0x1236);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::DE), 0x1236);

        cpu.write_register_u16(registers::WordRegister::HL, 0x1237);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::HL), 0x1237);

        cpu.write_register_u16(registers::WordRegister::PC, 0x1238);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::PC), 0x1238);

        cpu.write_register_u16(registers::WordRegister::SP, 0x1239);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::SP), 0x1239);
    }

    #[test]
    fn test_reg_write_u8_read_u16() {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_register_u8(registers::ByteRegister::A, 0x15);
        cpu.write_register_u8(registers::ByteRegister::F, 0x12);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::AF)
                .to_be_bytes(),
            // F register lower 4 bytes are not writable
            [0x15, 0x10]
        );

        cpu.write_register_u8(registers::ByteRegister::B, 0x25);
        cpu.write_register_u8(registers::ByteRegister::C, 0x22);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::BC)
                .to_be_bytes(),
            [0x25, 0x22]
        );

        cpu.write_register_u8(registers::ByteRegister::D, 0x35);
        cpu.write_register_u8(registers::ByteRegister::E, 0x32);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::DE)
                .to_be_bytes(),
            [0x35, 0x32]
        );

        cpu.write_register_u8(registers::ByteRegister::H, 0x45);
        cpu.write_register_u8(registers::ByteRegister::L, 0x42);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::HL)
                .to_be_bytes(),
            [0x45, 0x42]
        );
    }

    #[test]
    fn test_reg_write_u16_read_u8() {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_register_u16(registers::WordRegister::AF, 0x9876);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::A), 0x98);
        // F register lower 4 bytes are not writable
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::F), 0x70);

        cpu.write_register_u16(registers::WordRegister::BC, 0x9775);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::B), 0x97);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::C), 0x75);

        cpu.write_register_u16(registers::WordRegister::DE, 0x9674);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::D), 0x96);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::E), 0x74);

        cpu.write_register_u16(registers::WordRegister::HL, 0x9573);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::H), 0x95);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::L), 0x73);
    }

    #[test]
    fn test_mem_write_u8_read_u8_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_memory_u8(0xc100, 0x32)?;
        assert_eq!(cpu.read_memory_u8(0xc100), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u16_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_memory_u16(0xc100, 0x1032)?;
        assert_eq!(cpu.read_memory_u16(0xc100), Ok(0x1032));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u16_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_memory_u8(0xc100, 0x48)?;
        cpu.write_memory_u8(0xc101, 0x94)?;

        assert_eq!(cpu.read_memory_u16(0xc100), Ok(0x9448));
        Ok(())
    }

    #[test]
    fn test_mem_write_u16_read_u8_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_memory_u16(0xc200, 0x1345)?;

        assert_eq!(cpu.read_memory_u8(0xc200), Ok(0x45));
        assert_eq!(cpu.read_memory_u8(0xc201), Ok(0x13));
        Ok(())
    }

    #[test]
    fn test_write_u8_read_i8_sysram() -> MemoryResult<()> {
        let mut cpu = GameBoyCPU::new(make_cartridge());
        let signed_value = i8::from_le_bytes([0xa2]);

        cpu.write_memory_u8(0xc200, 0xa2)?;

        assert_eq!(cpu.read_memory_i8(0xc200), Ok(signed_value));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u8_vram() -> MemoryResult<()> {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_memory_u8(0x8100, 0x32)?;
        assert_eq!(cpu.read_memory_u8(0x8100), Ok(0x32));
        Ok(())
    }

    #[test]
    fn test_mem_write_u8_read_u8_cpuram() -> MemoryResult<()> {
        let mut cpu = GameBoyCPU::new(make_cartridge());

        cpu.write_memory_u8(0xff80, 0x32)?;
        assert_eq!(cpu.read_memory_u8(0xff80), Ok(0x32));
        Ok(())
    }
}
