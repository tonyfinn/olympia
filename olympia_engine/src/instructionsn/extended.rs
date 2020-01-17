use super::misc::{exec_rotate, SetZeroMode};
use crate::gameboy::{GameBoy, StepResult};
use crate::instructions::{Carry, RotateDirection};
use crate::instructionsn::{ExecutableInstruction, RuntimeOpcode};
use crate::registers::{ByteRegisterTarget, Flag};

use alloc::boxed::Box;
use alloc::vec::Vec;

use olympia_derive::OlympiaInstruction;

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x01_AAA_BBB, label = "BIT", extended)]
struct TestBit {
    #[olympia(dest, mask = 0xA)]
    bit: u8,
    #[olympia(src, mask = 0xB)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for TestBit {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let val = gb.exec_read_register_target(self.target)?;
        let bit_test = val & (1 << self.bit);
        gb.set_flag_to(Flag::Zero, bit_test == 0);
        gb.set_flag(Flag::HalfCarry);
        gb.set_flag_to(Flag::AddSubtract, false);
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x10_AAA_BBB, label = "RES", extended)]
struct ResetBit {
    #[olympia(dest, mask = 0xA)]
    bit: u8,
    #[olympia(src, mask = 0xB)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for ResetBit {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let val = gb.exec_read_register_target(self.target)?;
        let new_val = val & !(1 << self.bit);
        gb.exec_write_register_target(self.target, new_val)?;
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x11_AAA_BBB, label = "SET", extended)]
struct SetBit {
    #[olympia(dest, mask = 0xA)]
    bit: u8,
    #[olympia(src, mask = 0xB)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for SetBit {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let val = gb.exec_read_register_target(self.target)?;
        let new_val = val | (1 << self.bit);
        gb.exec_write_register_target(self.target, new_val)?;
        Ok(())
    }
}

macro_rules! ext_rotate_instruction {
    ($name:ident, $label:literal, $opcode:literal, $dir:path, $carry:path) => {
        #[derive(Debug, OlympiaInstruction)]
        #[olympia(opcode = $opcode, label = $label, extended)]
        struct $name {
            #[olympia(dest, mask = 0xA)]
            target: ByteRegisterTarget,
        }
        impl ExecutableInstruction for $name {
            fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
                exec_rotate(gb, $dir, $carry, self.target, SetZeroMode::Test)?;
                Ok(())
            }
        }
    };
}

ext_rotate_instruction!(
    RotateLeftCarry,
    "RLC",
    0x0000_0AAA,
    RotateDirection::Left,
    Carry::Carry
);
ext_rotate_instruction!(
    RotateLeft,
    "RL",
    0x0001_0AAA,
    RotateDirection::Left,
    Carry::NoCarry
);
ext_rotate_instruction!(
    RotateRightCarry,
    "RRC",
    0x0000_1AAA,
    RotateDirection::Right,
    Carry::Carry
);
ext_rotate_instruction!(
    RotateRight,
    "RR",
    0x0001_1AAA,
    RotateDirection::Right,
    Carry::NoCarry
);

fn exec_shift_zero(
    gb: &mut GameBoy,
    dir: RotateDirection,
    target: ByteRegisterTarget,
) -> StepResult<()> {
    let value = gb.exec_read_register_target(target)?;
    let (shifted_value, carry) = match dir {
        RotateDirection::Left => (value << 1, value & 0x80 != 0),
        RotateDirection::Right => (value >> 1, value & 0x01 != 0),
    };
    gb.set_flag_to(Flag::Carry, carry);
    gb.exec_write_register_target(target, shifted_value)?;
    Ok(())
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0010_0AAA, label = "SLA", extended)]
struct ShiftLeftZero {
    #[olympia(single, mask = 0xA)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for ShiftLeftZero {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        exec_shift_zero(gb, RotateDirection::Left, self.target)
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0011_1AAA, label = "SRL", extended)]
struct ShiftRightZero {
    #[olympia(single, mask = 0xA)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for ShiftRightZero {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        exec_shift_zero(gb, RotateDirection::Right, self.target)
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0010_1AAA, label = "SRA", extended)]
struct ShiftRightExtend {
    #[olympia(single, mask = 0xA)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for ShiftRightExtend {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.exec_read_register_target(self.target)?;
        let value16 = u16::from(value);
        let extra_bit = (value16 << 1) & 0xff00;
        let shifted_value = (extra_bit + value16) >> 1;
        let actual_byte = shifted_value.to_le_bytes()[0];
        gb.exec_write_register_target(self.target, actual_byte)?;
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0011_0AAA, label = "SWAP", extended)]
struct Swap {
    #[olympia(dest, mask = 0xA)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for Swap {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.exec_read_register_target(self.target)?;
        let low_nibble = value & 0x0F;
        let high_nibble = value & 0xF0;
        let new_value = (low_nibble.rotate_left(4)) + (high_nibble.rotate_right(4));
        gb.exec_write_register_target(self.target, new_value)?;
        Ok(())
    }
}

pub(crate) fn opcodes() -> Vec<(u8, Box<dyn RuntimeOpcode>)> {
    vec![
        TestBitOpcode::all(),
        ResetBitOpcode::all(),
        SetBitOpcode::all(),
        RotateLeftCarryOpcode::all(),
        RotateLeftOpcode::all(),
        RotateRightCarryOpcode::all(),
        RotateRightOpcode::all(),
        ShiftLeftZeroOpcode::all(),
        ShiftRightZeroOpcode::all(),
        ShiftRightExtendOpcode::all(),
        SwapOpcode::all(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
