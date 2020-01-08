use crate::address;
use crate::gameboy::{GameBoy, StepResult};
use crate::instructions::{ByteRegisterOffset, Increment};
use crate::instructionsn::{ExecutableInstruction, ExecutableOpcode};
use crate::registers::{ByteRegister, ByteRegisterTarget, StackRegister, WordRegister};

use alloc::boxed::Box;
use alloc::vec::Vec;

use olympia_derive::OlympiaInstruction;

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x01AA_ABBB, label = "LD", excluded(0x76))]
pub(crate) struct TargetTarget {
    #[olympia(dest, mask = 0xA)]
    dest: ByteRegisterTarget,
    #[olympia(src, mask = 0xB)]
    src: ByteRegisterTarget,
}

impl ExecutableInstruction for TargetTarget {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let val = gb.exec_read_register_target(self.src)?;
        gb.exec_write_register_target(self.dest, val)?;
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_A110, label = "LD")]
pub(crate) struct TargetConstant {
    #[olympia(dest, mask = 0xA)]
    dest: ByteRegisterTarget,
    #[olympia(src)]
    val: u8,
}

impl ExecutableInstruction for TargetConstant {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.exec_write_register_target(self.dest, self.val)?;
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_0001, label = "LD")]
pub(crate) struct Constant16 {
    #[olympia(dest, mask = 0xA)]
    dest: StackRegister,
    #[olympia(src)]
    val: u16,
}

impl ExecutableInstruction for Constant16 {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        gb.write_register_u16(self.dest.into(), self.val);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1110_1010, label = "LD")]
pub(crate) struct IndirectA {
    #[olympia(dest)]
    dest: address::LiteralAddress,
    #[olympia(src, constant(ByteRegister::A))]
    src: ByteRegister,
}

impl ExecutableInstruction for IndirectA {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.read_register_u8(self.src);
        gb.write_memory_u8(self.dest, value)?;
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1111_1010, label = "LD")]
pub(crate) struct AIndirect {
    #[olympia(src)]
    src: address::LiteralAddress,
    #[olympia(dest, constant(ByteRegister::A))]
    dest: ByteRegister,
}

impl ExecutableInstruction for AIndirect {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.read_memory_u8(self.src)?;
        gb.cycle();
        gb.write_register_u8(self.dest, value);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1110_0000, label = "LD")]
pub(crate) struct HighOffsetA {
    #[olympia(dest)]
    dest: address::HighAddress,
    #[olympia(src, constant(ByteRegister::A))]
    src: ByteRegister,
}

impl ExecutableInstruction for HighOffsetA {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.read_register_u8(self.src);
        gb.write_memory_u8(self.dest, value)?;
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1111_0000, label = "LD")]
pub(crate) struct AHighOffset {
    #[olympia(src)]
    src: address::HighAddress,
    #[olympia(dest, constant(ByteRegister::A))]
    dest: ByteRegister,
}

impl ExecutableInstruction for AHighOffset {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.read_memory_u8(self.src)?;
        gb.cycle();
        gb.write_register_u8(self.dest, value);
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1110_0010, label = "LD")]
pub(crate) struct RegisterOffsetA {
    #[olympia(dest, constant(ByteRegister::C))]
    dest: ByteRegisterOffset,
    #[olympia(src, constant(ByteRegister::A))]
    src: ByteRegister,
}

impl ExecutableInstruction for RegisterOffsetA {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let addr = address::HighAddress(gb.read_register_u8(self.dest.into()));
        let value = gb.read_register_u8(self.src);
        gb.write_memory_u8(addr, value)?;
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x1111_0010, label = "LD")]
pub(crate) struct ARegisterOffset {
    #[olympia(src, constant(ByteRegister::C))]
    src: ByteRegisterOffset,
    #[olympia(dest, constant(ByteRegister::A))]
    dest: ByteRegister,
}

impl ExecutableInstruction for ARegisterOffset {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let addr = address::HighAddress(gb.read_register_u8(self.src.into()));
        let value = gb.read_memory_u8(addr)?;
        gb.cycle();
        gb.write_register_u8(self.dest, value);
        Ok(())
    }
}

fn increment_16a(
    gb: &mut GameBoy,
    dest: WordRegister,
    src: ByteRegister,
    inc: Increment,
) -> StepResult<()> {
    let addr = gb.read_register_u16(dest);
    gb.write_memory_u8(addr, gb.read_register_u8(src))?;
    let new_addr = match inc {
        Increment::Increment => addr.wrapping_add(1),
        Increment::Decrement => addr.wrapping_sub(1),
    };
    gb.write_register_u16(dest, new_addr);
    gb.cycle();
    Ok(())
}

fn a_increment_16(
    gb: &mut GameBoy,
    dest: ByteRegister,
    src: WordRegister,
    inc: Increment,
) -> StepResult<()> {
    let addr = gb.read_register_u16(src);
    let value = gb.read_memory_u8(addr)?;
    let new_addr = match inc {
        Increment::Increment => addr.wrapping_add(1),
        Increment::Decrement => addr.wrapping_sub(1),
    };
    gb.write_register_u8(dest, value);
    gb.write_register_u16(src, new_addr);
    gb.cycle();
    Ok(())
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x0010_0010, label = "INC")]
pub(crate) struct Increment16A {
    #[olympia(src, constant(ByteRegister::A))]
    src: ByteRegister,
    #[olympia(dest, constant(WordRegister::HL))]
    dest: WordRegister,
}

impl ExecutableInstruction for Increment16A {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        increment_16a(gb, self.dest, self.src, Increment::Increment)
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x0011_0010, label = "DEC")]
pub(crate) struct Decrement16A {
    #[olympia(src, constant(ByteRegister::A))]
    src: ByteRegister,
    #[olympia(dest, constant(WordRegister::HL))]
    dest: WordRegister,
}

impl ExecutableInstruction for Decrement16A {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        increment_16a(gb, self.dest, self.src, Increment::Decrement)
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x0010_1010, label = "INC")]
pub(crate) struct AIncrement16 {
    #[olympia(dest, constant(ByteRegister::A))]
    dest: ByteRegister,
    #[olympia(src, constant(WordRegister::HL))]
    src: WordRegister,
}

impl ExecutableInstruction for AIncrement16 {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        a_increment_16(gb, self.dest, self.src, Increment::Increment)
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x0011_1010, label = "DEC")]
pub(crate) struct ADecrement16 {
    #[olympia(dest, constant(ByteRegister::A))]
    dest: ByteRegister,
    #[olympia(src, constant(WordRegister::HL))]
    src: WordRegister,
}

impl ExecutableInstruction for ADecrement16 {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        a_increment_16(gb, self.dest, self.src, Increment::Decrement)
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_1010, label = "LD", excluded(0x2A, 0x3A))]
pub(crate) struct AWordTarget {
    #[olympia(dest, constant(ByteRegister::A))]
    dest: ByteRegister,
    #[olympia(src, mask = 0xA)]
    src: StackRegister,
}

impl ExecutableInstruction for AWordTarget {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let addr = gb.read_register_u16(self.src.into());
        let value = gb.read_memory_u8(addr)?;
        gb.cpu.write_register_u8(self.dest, value);
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_0010, label = "LD", excluded(0x22, 0x32))]
pub(crate) struct WordTargetA {
    #[olympia(src, constant(ByteRegister::A))]
    src: ByteRegister,
    #[olympia(dest, mask = 0xA)]
    dest: StackRegister,
}

impl ExecutableInstruction for WordTargetA {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let value = gb.read_register_u8(self.src);
        let target_addr = gb.read_register_u16(self.dest.into());
        gb.write_memory_u8(target_addr, value)?;
        gb.cycle();
        Ok(())
    }
}

pub(crate) fn opcodes() -> Vec<(u8, Box<dyn ExecutableOpcode>)> {
    vec![
        TargetTargetOpcode::all(),
        TargetConstantOpcode::all(),
        Constant16Opcode::all(),
        AIndirectOpcode::all(),
        IndirectAOpcode::all(),
        AHighOffsetOpcode::all(),
        HighOffsetAOpcode::all(),
        ARegisterOffsetOpcode::all(),
        RegisterOffsetAOpcode::all(),
        Increment16AOpcode::all(),
        Decrement16AOpcode::all(),
        AIncrement16Opcode::all(),
        ADecrement16Opcode::all(),
        AWordTargetOpcode::all(),
        WordTargetAOpcode::all(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
