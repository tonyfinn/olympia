use crate::registers;
use crate::rom;

// Re-export long name, but use short name internally
pub use crate::registers::{ByteRegister, WordRegister};
use crate::registers::{ByteRegister as br, WordRegister as wr};

pub(crate) enum InterruptState {
    Pending,
    Enabled,
    Disabled,
}

pub enum Interrupt {
    VBlank,
    LCDStatus,
    Timer,
    Serial,
    Input,
}

impl Interrupt {
    pub(crate) fn test(ie: u8, iflag: u8) -> Option<Interrupt> {
        let pending_interrupts = ie & iflag;
        if pending_interrupts & 1 != 0 {
            Some(Interrupt::VBlank)
        } else if pending_interrupts & 2 != 0 {
            Some(Interrupt::LCDStatus)
        } else if pending_interrupts & 4 != 0 {
            Some(Interrupt::Timer)
        } else if pending_interrupts & 8 != 0 {
            Some(Interrupt::Serial)
        } else if pending_interrupts & 16 != 0 {
            Some(Interrupt::Input)
        } else {
            None
        }
    }

    pub(crate) fn handler_address(&self) -> crate::address::LiteralAddress {
        match self {
            Interrupt::VBlank => 0x40,
            Interrupt::LCDStatus => 0x48,
            Interrupt::Timer => 0x50,
            Interrupt::Serial => 0x58,
            Interrupt::Input => 0x60,
        }
        .into()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Registers {
    af: u16,
    bc: u16,
    de: u16,
    hl: u16,
    sp: u16,
    pc: u16,
}

impl Registers {
    fn read_u8(&self, reg: registers::ByteRegister) -> u8 {
        match reg {
            br::A => self.af.to_le_bytes()[1],
            br::F => self.af.to_le_bytes()[0],
            br::B => self.bc.to_le_bytes()[1],
            br::C => self.bc.to_le_bytes()[0],
            br::D => self.de.to_le_bytes()[1],
            br::E => self.de.to_le_bytes()[0],
            br::H => self.hl.to_le_bytes()[1],
            br::L => self.hl.to_le_bytes()[0],
        }
    }

    fn read_u16(&self, reg: registers::WordRegister) -> u16 {
        match reg {
            wr::AF => self.af.to_le(),
            wr::BC => self.bc.to_le(),
            wr::DE => self.de.to_le(),
            wr::HL => self.hl.to_le(),
            wr::SP => self.sp.to_le(),
            wr::PC => self.pc.to_le(),
        }
    }

    fn write_u8(&mut self, reg: registers::ByteRegister, value: u8) {
        let full_register = reg.lookup_word_register();
        let byte = reg.lookup_byte();

        match byte {
            registers::WordByte::High => self.write_high_byte(full_register, value),
            registers::WordByte::Low => self.write_low_byte(full_register, value),
        }
    }

    fn write_high_byte(&mut self, full_register: registers::WordRegister, value_to_write: u8) {
        let register_value = self.read_u16(full_register);
        let u16_value = u16::from(value_to_write);
        let masked_register_value = register_value & 0x00ff;
        let shifted_value = u16_value << 8;
        self.write_raw(full_register, masked_register_value + shifted_value);
    }

    fn write_low_byte(&mut self, full_register: registers::WordRegister, value_to_write: u8) {
        let register_value = self.read_u16(full_register);
        let u16_value = u16::from(value_to_write);
        let masked_register_value = register_value & 0xff00;
        self.write_raw(full_register, masked_register_value + u16_value);
    }

    fn write_raw(&mut self, reg: registers::WordRegister, value: u16) {
        match reg {
            registers::WordRegister::AF => self.af = value & 0xfff0,
            registers::WordRegister::BC => self.bc = value,
            registers::WordRegister::DE => self.de = value,
            registers::WordRegister::HL => self.hl = value,
            registers::WordRegister::SP => self.sp = value,
            registers::WordRegister::PC => self.pc = value,
        }
    }

    fn write_u16(&mut self, reg: registers::WordRegister, value: u16) {
        self.write_raw(reg, value.to_le());
    }

    fn default_for_model(model: super::GameBoyModel, target: rom::TargetConsole) -> Registers {
        Registers {
            af: model.default_af(),
            bc: model.default_bc(),
            de: model.default_de(target),
            hl: model.default_hl(target),
            sp: 0xfffe,
            pc: 0x100,
        }
    }

    fn read_flag(&self, flag: registers::Flag) -> bool {
        self.af & (1u16 << flag.bit()) != 0
    }

    fn set_flag(&mut self, flag: registers::Flag) {
        self.af |= 1 << flag.bit();
    }

    fn reset_flag(&mut self, flag: registers::Flag) {
        self.af &= !(1u16 << flag.bit());
    }
}

/*#[derive(PartialEq, Eq, Debug)]
enum IODirection {
    In,
    Out
}

impl Default for IODirection {
    fn default() -> IODirection {
        IODirection::In
    }
}

#[derive(PartialEq, Eq, Debug, Default)]
struct AddressBus {
    address: u16,
    data: u8,
    direction: IODirection
}*/

pub(crate) struct Cpu {
    registers: Registers,
    pub(crate) interrupts_enabled: InterruptState,
    // address_bus: AddressBus
}

impl Cpu {
    pub(crate) fn new(model: super::GameBoyModel, target: rom::TargetConsole) -> Cpu {
        Cpu {
            registers: Registers::default_for_model(model, target),
            interrupts_enabled: InterruptState::Disabled,
            // address_bus: AddressBus::default()
        }
    }

    pub(crate) fn read_register_u16(&self, reg: registers::WordRegister) -> u16 {
        self.registers.read_u16(reg)
    }

    pub(crate) fn write_register_u16(&mut self, reg: registers::WordRegister, val: u16) {
        self.registers.write_u16(reg, val)
    }

    pub(crate) fn read_register_u8(&self, reg: registers::ByteRegister) -> u8 {
        self.registers.read_u8(reg)
    }

    pub(crate) fn write_register_u8(&mut self, reg: registers::ByteRegister, val: u8) {
        self.registers.write_u8(reg, val)
    }

    pub(crate) fn read_flag(&self, flag: registers::Flag) -> bool {
        self.registers.read_flag(flag)
    }

    pub(crate) fn set_flag_to(&mut self, flag: registers::Flag, value: bool) {
        if value {
            self.set_flag(flag)
        } else {
            self.reset_flag(flag)
        }
    }

    pub(crate) fn set_flag(&mut self, flag: registers::Flag) {
        self.registers.set_flag(flag)
    }

    pub(crate) fn reset_flag(&mut self, flag: registers::Flag) {
        self.registers.reset_flag(flag)
    }
}

#[cfg(test)]
mod alu_tests;

#[cfg(test)]
mod extended_opcode_tests;

#[cfg(test)]
mod jump_tests;

#[cfg(test)]
mod interrupt_tests;

#[cfg(test)]
mod load_tests;

#[cfg(test)]
mod stack_tests;

#[cfg(test)]
mod misc_tests;

#[cfg(test)]
pub(crate) mod testutils {
    use super::*;
    use crate::address::LiteralAddress;
    use crate::gameboy;

    pub const PROGRAM_START: u16 = 0x200;
    pub const PROG_MEMORY_OFFSET: LiteralAddress = LiteralAddress(0x200);

    type ProgramSegment<'a> = (LiteralAddress, &'a [u8]);

    pub fn make_cartridge_with(segments: &[ProgramSegment]) -> rom::Cartridge {
        let mut data = vec![0u8; 0x8000];
        for segment in segments {
            let (start, segment_data) = segment;
            let LiteralAddress(raw_addr) = start;
            let offset = *raw_addr as usize;
            data[offset..offset + segment_data.len()].clone_from_slice(segment_data);
        }
        rom::Cartridge::from_data(data).unwrap()
    }

    pub fn run_program_with(
        steps: u64,
        segments: &[ProgramSegment],
    ) -> gameboy::StepResult<gameboy::GameBoy> {
        let cartridge = make_cartridge_with(segments);
        let mut gb = gameboy::GameBoy::new(cartridge, gameboy::GameBoyModel::GameBoy);
        gb.write_register_u16(registers::WordRegister::PC, PROGRAM_START);
        for _ in 0..steps {
            gb.step()?
        }
        Ok(gb)
    }

    pub fn run_program(steps: u64, program: &[u8]) -> gameboy::StepResult<gameboy::GameBoy> {
        run_program_with(steps, &[(PROG_MEMORY_OFFSET, program)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameboy::GameBoyModel;

    #[test]
    fn test_reg_write_u8_read_u8() {
        let mut cpu = Cpu::new(GameBoyModel::GameBoy, rom::TargetConsole::GameBoyOnly);

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
        let mut cpu = Cpu::new(GameBoyModel::GameBoy, rom::TargetConsole::GameBoyOnly);

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
        let mut cpu = Cpu::new(GameBoyModel::GameBoy, rom::TargetConsole::GameBoyOnly);

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
        let mut cpu = Cpu::new(GameBoyModel::GameBoy, rom::TargetConsole::GameBoyOnly);

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
}
