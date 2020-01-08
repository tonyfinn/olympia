use crate::gameboy::{GameBoy, StepResult};
use crate::{address, registers};

use super::{ExecutableInstruction, ExecutableOpcode};

use olympia_core::registers::{AccRegister, WordRegister};
use olympia_derive::OlympiaInstruction;

use alloc::boxed::Box;
use alloc::vec::Vec;

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x11AA_0101, label = "PUSH")]
struct Push {
    #[olympia(single, mask = 0xA)]
    reg: AccRegister,
}

impl ExecutableInstruction for Push {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.read_register_u16(self.reg.into());
        gb.exec_push(value)?;
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x11AA_0001, label = "POP")]
struct Pop {
    #[olympia(single, mask = 0xA)]
    reg: AccRegister,
}

impl ExecutableInstruction for Pop {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.exec_pop()?;
        gb.write_register_u16(self.reg.into(), value);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1110_1000, label = "ADD")]
struct AddStackPointer {
    #[olympia(dest, constant(WordRegister::SP))]
    dest: WordRegister,
    #[olympia(src)]
    value: address::AddressOffset,
}

impl ExecutableInstruction for AddStackPointer {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let sp = gb.read_register_u16(self.dest);
        let address::OffsetResolveResult {
            addr,
            half_carry,
            carry,
        } = self.value.resolve(sp.into());
        gb.write_register_u16(self.dest, addr.into());
        gb.set_flag_to(registers::Flag::HalfCarry, half_carry);
        gb.set_flag_to(registers::Flag::Carry, carry);
        gb.reset_flag(registers::Flag::Zero);
        gb.reset_flag(registers::Flag::AddSubtract);
        gb.cycle();
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1111_1000, label = "LD")]
struct LoadStackOffset {
    #[olympia(dest, constant(WordRegister::HL))]
    dest: WordRegister,
    #[olympia(src, constant(WordRegister::SP))]
    base: WordRegister,
    #[olympia(addsrc)]
    value: address::AddressOffset,
}

impl ExecutableInstruction for LoadStackOffset {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let sp = gb.read_register_u16(self.base);
        let address::OffsetResolveResult {
            addr,
            half_carry,
            carry,
        } = self.value.resolve(sp.into());
        gb.write_register_u16(self.dest, addr.into());
        gb.set_flag_to(registers::Flag::HalfCarry, half_carry);
        gb.set_flag_to(registers::Flag::Carry, carry);
        gb.reset_flag(registers::Flag::Zero);
        gb.reset_flag(registers::Flag::AddSubtract);
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1111_1001, label = "LD")]
struct SetStackPointer {
    #[olympia(dest, constant(WordRegister::SP))]
    dest: WordRegister,
    #[olympia(src, constant(WordRegister::HL))]
    src: WordRegister,
}

impl ExecutableInstruction for SetStackPointer {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let val = gb.read_register_u16(self.src);
        gb.write_register_u16(self.dest, val);
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x0000_1000, label = "LD")]
struct StoreStackPointerMemory {
    #[olympia(dest)]
    dest: address::LiteralAddress,
    #[olympia(src, constant(WordRegister::SP))]
    src: WordRegister,
}

impl ExecutableInstruction for StoreStackPointerMemory {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let sp_val = gb.read_register_u16(self.src);
        gb.exec_write_memory_u16(self.dest, sp_val)?;
        Ok(())
    }
}

pub(crate) fn opcodes() -> Vec<(u8, Box<dyn ExecutableOpcode>)> {
    vec![
        PushOpcode::all(),
        PopOpcode::all(),
        AddStackPointerOpcode::all(),
        LoadStackOffsetOpcode::all(),
        SetStackPointerOpcode::all(),
        StoreStackPointerMemoryOpcode::all(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
