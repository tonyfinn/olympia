use crate::gameboy::{
    cpu::{InterruptState, PowerSavingMode},
    GameBoy, StepResult,
};
use crate::instructions::{Carry, RotateDirection};
use crate::instructionsn::{ExecutableInstruction, RuntimeOpcode};
use crate::registers;

use alloc::boxed::Box;
use alloc::vec::Vec;
use olympia_derive::OlympiaInstruction;

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum SetZeroMode {
    Test,
    Clear,
}

pub(crate) fn exec_rotate(
    gb: &mut GameBoy,
    dir: RotateDirection,
    carry: Carry,
    reg: registers::ByteRegisterTarget,
    set_zero: SetZeroMode,
) -> StepResult<()> {
    let current_value = gb.exec_read_register_target(reg)?;
    let high_byte = if carry != Carry::Carry {
        if gb.read_flag(registers::Flag::Carry) {
            0x81
        } else {
            0x00
        }
    } else {
        current_value
    };
    let value_to_rotate = u16::from_le_bytes([current_value, high_byte]);
    let (rotated, carry) = if dir == RotateDirection::Left {
        let rotated = value_to_rotate.rotate_left(1);
        (rotated.to_le_bytes()[0], (rotated & 0x0100) != 0)
    } else {
        let rotated = value_to_rotate.rotate_right(1);
        (rotated.to_le_bytes()[0], (rotated & 0x8000) != 0)
    };
    gb.exec_write_register_target(reg, rotated)?;
    gb.set_flag_to(registers::Flag::Carry, carry);
    let zero_flag = match set_zero {
        SetZeroMode::Test => rotated == 0,
        SetZeroMode::Clear => false,
    };
    gb.set_flag_to(registers::Flag::Zero, zero_flag);
    gb.reset_flag(registers::Flag::HalfCarry);
    gb.reset_flag(registers::Flag::AddSubtract);
    Ok(())
}

macro_rules! rotate_instruction {
    ($name:ident, $label:literal, $opcode:literal, $dir:path, $carry:path, $target:path) => {
        #[derive(Debug, OlympiaInstruction)]
        #[olympia(opcode = $opcode, label = $label)]
        struct $name {}
        impl ExecutableInstruction for $name {
            fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
                exec_rotate(gb, $dir, $carry, $target, SetZeroMode::Clear)?;
                Ok(())
            }
        }
    };
}

rotate_instruction!(
    RotateLeftCarry,
    "RLCA",
    0x0000_0111,
    RotateDirection::Left,
    Carry::Carry,
    registers::ByteRegisterTarget::A
);

rotate_instruction!(
    RotateLeft,
    "RLA",
    0x0001_0111,
    RotateDirection::Left,
    Carry::NoCarry,
    registers::ByteRegisterTarget::A
);

rotate_instruction!(
    RotateRightCarry,
    "RRCA",
    0x0000_1111,
    RotateDirection::Right,
    Carry::Carry,
    registers::ByteRegisterTarget::A
);

rotate_instruction!(
    RotateRight,
    "RRA",
    0x0001_1111,
    RotateDirection::Right,
    Carry::NoCarry,
    registers::ByteRegisterTarget::A
);

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x1111_1011, label = "EI")]
struct EnableInterrupts {}

impl ExecutableInstruction for EnableInterrupts {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_interrupt_state(InterruptState::Pending);
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x1111_0011, label = "DI")]
struct DisableInterrupts {}

impl ExecutableInstruction for DisableInterrupts {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_interrupt_state(InterruptState::Disabled);
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0000_0000, label = "NOP")]
struct NoOp {}

impl ExecutableInstruction for NoOp {
    fn execute(&self, _gb: &mut GameBoy) -> StepResult<()> {
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0011_1111, label = "CCF")]
struct InvertCarry {}

impl ExecutableInstruction for InvertCarry {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let inverted = !gb.read_flag(registers::Flag::Carry);
        gb.set_flag_to(registers::Flag::Carry, inverted);
        gb.reset_flag(registers::Flag::HalfCarry);
        gb.reset_flag(registers::Flag::AddSubtract);
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0011_0111, label = "SCF")]
struct SetCarry {}

impl ExecutableInstruction for SetCarry {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_flag(registers::Flag::Carry);
        gb.reset_flag(registers::Flag::HalfCarry);
        gb.reset_flag(registers::Flag::AddSubtract);
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0010_1111, label = "CPL")]
struct InvertA {}

impl ExecutableInstruction for InvertA {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_flag(registers::Flag::AddSubtract);
        gb.set_flag(registers::Flag::HalfCarry);
        let val = gb.read_register_u8(registers::ByteRegister::A);
        gb.write_register_u8(registers::ByteRegister::A, !val);
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0010_0111, label = "DAA")]
struct AToBCD {}

impl AToBCD {
    #[allow(clippy::collapsible_if)]
    fn after_add(carry: bool, half_carry: bool, top_nibble: u8, bottom_nibble: u8) -> (u8, bool) {
        match (carry, half_carry) {
            (false, false) => {
                if bottom_nibble <= 9 {
                    if top_nibble <= 9 {
                        (0, false)
                    } else {
                        (0x60, true)
                    }
                } else if top_nibble <= 8 {
                    (6, false)
                } else {
                    (0x66, true)
                }
            }
            (false, true) => {
                if bottom_nibble <= 0x3 {
                    if top_nibble <= 0x9 {
                        (0x6, false)
                    } else {
                        (0x66, true)
                    }
                } else {
                    (0, carry)
                }
            }
            (true, false) => {
                if top_nibble <= 2 {
                    if bottom_nibble <= 9 {
                        (0x60, true)
                    } else {
                        (0x66, true)
                    }
                } else {
                    (0, carry)
                }
            }
            (true, true) => {
                if top_nibble <= 3 && bottom_nibble <= 3 {
                    (0x66, true)
                } else {
                    (0, carry)
                }
            }
        }
    }

    #[allow(clippy::collapsible_if)]
    fn after_sub(carry: bool, half_carry: bool, top_nibble: u8, bottom_nibble: u8) -> (u8, bool) {
        if carry {
            if top_nibble >= 7 && bottom_nibble <= 9 && !half_carry {
                (0xA0, true)
            } else if top_nibble >= 6 && bottom_nibble >= 6 && half_carry {
                (0x9A, true)
            } else {
                (0, carry)
            }
        } else if top_nibble <= 9 && bottom_nibble <= 9 && !half_carry {
            (0, false)
        } else if top_nibble <= 8 && bottom_nibble >= 6 && half_carry {
            (0xFA, false)
        } else {
            (0, carry)
        }
    }
}

impl ExecutableInstruction for AToBCD {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let carry = gb.read_flag(registers::Flag::Carry);
        let half_carry = gb.read_flag(registers::Flag::HalfCarry);
        let add_subtract = gb.read_flag(registers::Flag::AddSubtract);
        let val = gb.read_register_u8(registers::ByteRegister::A);
        let top_nibble = (val & 0xF0) >> 4;
        let bottom_nibble = val & 0x0F;
        let (add, carry) = if add_subtract {
            AToBCD::after_sub(carry, half_carry, top_nibble, bottom_nibble)
        } else {
            AToBCD::after_add(carry, half_carry, top_nibble, bottom_nibble)
        };
        let result = val.wrapping_add(add);
        gb.write_register_u8(registers::ByteRegister::A, result);
        gb.set_flag_to(registers::Flag::Carry, carry);
        gb.set_flag_to(registers::Flag::Zero, result == 0);
        gb.reset_flag(registers::Flag::HalfCarry);
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0111_0110, label = "HALT")]
struct Halt {}

impl ExecutableInstruction for Halt {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        // TODO: Require an interrupt flag to be set
        gb.set_power_saving_mode(PowerSavingMode::Halt);
        Ok(())
    }
}

#[derive(Debug, OlympiaInstruction)]
#[olympia(opcode = 0x0001_0000, label = "STOP")]
struct Stop {}

impl ExecutableInstruction for Stop {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_power_saving_mode(PowerSavingMode::Stop);
        Ok(())
    }
}

pub(crate) fn opcodes() -> Vec<(u8, Box<dyn RuntimeOpcode>)> {
    vec![
        EnableInterruptsOpcode::all(),
        DisableInterruptsOpcode::all(),
        NoOpOpcode::all(),
        InvertCarryOpcode::all(),
        SetCarryOpcode::all(),
        InvertAOpcode::all(),
        AToBCDOpcode::all(),
        RotateRightCarryOpcode::all(),
        RotateRightOpcode::all(),
        RotateLeftCarryOpcode::all(),
        RotateLeftOpcode::all(),
        HaltOpcode::all(),
        StopOpcode::all(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
