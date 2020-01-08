use crate::gameboy::{cpu::InterruptState, GameBoy, StepResult};
use crate::instructionsn::{ExecutableInstruction, ExecutableOpcode};
use crate::registers;

use alloc::boxed::Box;
use alloc::vec::Vec;
use olympia_derive::OlympiaInstruction;

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1111_1011, label = "EI")]
struct EnableInterrupts {}

impl ExecutableInstruction for EnableInterrupts {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_interrupt_state(InterruptState::Pending);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1111_0011, label = "DI")]
struct DisableInterrupts {}

impl ExecutableInstruction for DisableInterrupts {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.set_interrupt_state(InterruptState::Disabled);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x0000_0000, label = "NOP")]
struct NOP {}

impl ExecutableInstruction for NOP {
    fn execute(&self, _gb: &mut GameBoy) -> StepResult<()> {
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
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

#[derive(OlympiaInstruction)]
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

#[derive(OlympiaInstruction)]
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

#[derive(OlympiaInstruction)]
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
        } else {
            if top_nibble <= 9 && bottom_nibble <= 9 && !half_carry {
                (0, false)
            } else if top_nibble <= 8 && bottom_nibble >= 6 && half_carry {
                (0xFA, false)
            } else {
                (0, carry)
            }
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

pub(crate) fn opcodes() -> Vec<(u8, Box<dyn ExecutableOpcode>)> {
    vec![
        EnableInterruptsOpcode::all(),
        DisableInterruptsOpcode::all(),
        NOPOpcode::all(),
        InvertCarryOpcode::all(),
        SetCarryOpcode::all(),
        InvertAOpcode::all(),
        AToBCDOpcode::all(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
