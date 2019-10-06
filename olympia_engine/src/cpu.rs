use crate::registers;

pub struct GameBoyCPU {
    af_register: u16,
    bc_register: u16,
    de_register: u16,
    hl_register: u16,
    sp_register: u16,
    pc_register: u16,
    memory: [u8; 0xffff],
}

impl GameBoyCPU {
    pub fn new() -> GameBoyCPU {
        GameBoyCPU {
            af_register: 0,
            bc_register: 0,
            de_register: 0,
            hl_register: 0,
            sp_register: 0,
            pc_register: 0,
            memory: [0u8; 0xffff],
        }
    }

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

    fn write_high_register_byte(&mut self, full_register: registers::WordRegister, value_to_write: u8) {
        let register_value = self.read_register_u16(full_register);
        let u16_value = u16::from(value_to_write);
        let masked_register_value = register_value & 0x00ff;
        let shifted_value = u16_value << 8;
        self.write_register_raw(full_register, masked_register_value + shifted_value);
    }

    fn write_low_register_byte(&mut self, full_register: registers::WordRegister, value_to_write: u8) {
        let register_value = self.read_register_u16(full_register);
        let u16_value = u16::from(value_to_write);
        let masked_register_value = register_value & 0xff00;
        self.write_register_raw(full_register, masked_register_value + u16_value);
    }

    fn write_register_raw(&mut self, reg: registers::WordRegister, value: u16) {
        match reg {
            registers::WordRegister::AF => self.af_register = value,
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

    pub fn read_memory_u8(&self, addr: u16) -> u8 {
        self.memory[addr as usize]
    }

    pub fn read_memory_i8(&self, addr: u16) -> i8 {
        i8::from_le_bytes([self.memory[addr as usize]])
    }

    pub fn read_memory_u16(&self, addr: u16) -> u16 {
        u16::from_le_bytes([
            self.memory[addr as usize],
            self.memory[addr.wrapping_add(1) as usize],
        ])
    }

    pub fn write_memory_u8(&mut self, addr: u16, value: u8) {
        self.memory[addr as usize] = value;
    }

    pub fn write_memory_u16(&mut self, addr: u16, value: u16) {
        let bytes = value.to_le_bytes();

        self.memory[addr as usize] = bytes[0];
        self.memory[addr.wrapping_add(1) as usize] = bytes[1];
    }
}

impl Default for GameBoyCPU {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reg_write_u8_read_u8() {
        let mut cpu = GameBoyCPU::new();

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

        cpu.write_register_u8(registers::ByteRegister::F, 0x06);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::F), 0x06);

        cpu.write_register_u8(registers::ByteRegister::H, 0x07);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::H), 0x07);

        cpu.write_register_u8(registers::ByteRegister::L, 0x08);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::L), 0x08);
    }

    #[test]
    fn test_reg_write_u16_read_u16() {
        let mut cpu = GameBoyCPU::new();

        cpu.write_register_u16(registers::WordRegister::AF, 0x1234);
        assert_eq!(cpu.read_register_u16(registers::WordRegister::AF), 0x1234);

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
        let mut cpu = GameBoyCPU::new();

        cpu.write_register_u8(registers::ByteRegister::A, 0x15);
        cpu.write_register_u8(registers::ByteRegister::F, 0x12);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::AF).to_be_bytes(),
            [0x15, 0x12]
        );

        cpu.write_register_u8(registers::ByteRegister::B, 0x25);
        cpu.write_register_u8(registers::ByteRegister::C, 0x22);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::BC).to_be_bytes(),
            [0x25, 0x22]
        );

        cpu.write_register_u8(registers::ByteRegister::D, 0x35);
        cpu.write_register_u8(registers::ByteRegister::E, 0x32);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::DE).to_be_bytes(),
            [0x35, 0x32]
        );

        cpu.write_register_u8(registers::ByteRegister::H, 0x45);
        cpu.write_register_u8(registers::ByteRegister::L, 0x42);
        assert_eq!(
            cpu.read_register_u16(registers::WordRegister::HL).to_be_bytes(),
            [0x45, 0x42]
        );
    }

    #[test]
    fn test_reg_write_u16_read_u8() {
        let mut cpu = GameBoyCPU::new();

        cpu.write_register_u16(registers::WordRegister::AF, 0x9876);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::A), 0x98);
        assert_eq!(cpu.read_register_u8(registers::ByteRegister::F), 0x76);

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
    fn test_mem_write_u8_read_u8() {
        let mut cpu = GameBoyCPU::new();

        cpu.write_memory_u8(0x100, 0x32);
        assert_eq!(cpu.read_memory_u8(0x100), 0x32);
    }

    #[test]
    fn test_mem_write_u16_read_u16() {
        let mut cpu = GameBoyCPU::new();

        cpu.write_memory_u16(0x100, 0x1032);
        assert_eq!(cpu.read_memory_u16(0x100), 0x1032);
    }

    #[test]
    fn test_mem_write_u8_read_u16() {
        let mut cpu = GameBoyCPU::new();

        cpu.write_memory_u8(0x100, 0x48);
        cpu.write_memory_u8(0x101, 0x94);

        assert_eq!(cpu.read_memory_u16(0x100), 0x9448);
    }

    #[test]
    fn test_mem_write_u16_read_u8() {
        let mut cpu = GameBoyCPU::new();

        cpu.write_memory_u16(0x200, 0x1345);

        assert_eq!(cpu.read_memory_u8(0x200), 0x45);
        assert_eq!(cpu.read_memory_u8(0x201), 0x13);
    }

    #[test]
    fn test_write_u8_read_i8() {
        let mut cpu = GameBoyCPU::new();
        let signed_value = i8::from_le_bytes([0xa2]);

        cpu.write_memory_u8(0x200, 0xa2);

        assert_eq!(cpu.read_memory_i8(0x200), signed_value);
    }
}
