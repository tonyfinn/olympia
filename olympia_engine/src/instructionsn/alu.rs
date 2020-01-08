use crate::gameboy::{GameBoy, StepResult};
use crate::instructions::{ALOp, ByteRegisterTarget};
use crate::instructionsn::{ExecutableInstruction, ExecutableOpcode};
use crate::registers;

use alloc::boxed::Box;
use alloc::vec::Vec;

use olympia_derive::OlympiaInstruction;

fn is_add_half_carry(a: u8, b: u8) -> bool {
    0 != (((a & 0xF) + (b & 0xF)) & 0xF0)
}

fn is_sub_half_carry(a: u8, b: u8) -> bool {
    let sub = (a & 0x1F).wrapping_sub(b & 0x0F);
    (sub & 0x10) != (a & 0x10)
}

fn alu_op(gb: &mut GameBoy, op: ALOp, arg: u8) -> u8 {
    let current_value = gb.cpu.read_register_u8(registers::ByteRegister::A);
    match op {
        ALOp::Add => {
            let (new, overflow) = current_value.overflowing_add(arg);
            gb.set_flag_to(registers::Flag::Carry, overflow);
            gb.reset_flag(registers::Flag::AddSubtract);
            gb.set_flag_to(registers::Flag::Zero, new == 0);
            gb.set_flag_to(
                registers::Flag::HalfCarry,
                is_add_half_carry(current_value, arg),
            );
            new
        }
        ALOp::AddCarry => {
            let carry_bit = u8::from(gb.read_flag(registers::Flag::Carry));
            let (tmp, overflow) = current_value.overflowing_add(arg);
            let (new, overflow_carry) = tmp.overflowing_add(carry_bit);
            gb.set_flag_to(registers::Flag::Carry, overflow | overflow_carry);
            gb.reset_flag(registers::Flag::AddSubtract);
            gb.set_flag_to(registers::Flag::Zero, new == 0);
            gb.set_flag_to(
                registers::Flag::HalfCarry,
                is_add_half_carry(current_value, arg + carry_bit),
            );
            new
        }
        ALOp::Sub => {
            let (new, overflow) = current_value.overflowing_sub(arg);
            gb.set_flag_to(registers::Flag::Carry, overflow);
            gb.set_flag(registers::Flag::AddSubtract);
            gb.set_flag_to(registers::Flag::Zero, new == 0);
            gb.set_flag_to(
                registers::Flag::HalfCarry,
                is_sub_half_carry(current_value, arg),
            );
            new
        }
        ALOp::SubCarry => {
            let carry_bit = u8::from(gb.read_flag(registers::Flag::Carry));
            let (tmp, overflow) = current_value.overflowing_sub(arg);
            let (new, overflow_carry) = tmp.overflowing_sub(carry_bit);
            gb.set_flag_to(registers::Flag::Carry, overflow | overflow_carry);
            gb.set_flag(registers::Flag::AddSubtract);
            gb.set_flag_to(registers::Flag::Zero, new == 0);
            gb.set_flag_to(
                registers::Flag::HalfCarry,
                is_sub_half_carry(current_value, arg + carry_bit),
            );
            new
        }
        ALOp::Compare => {
            let (new, overflow) = current_value.overflowing_sub(arg);
            gb.set_flag_to(registers::Flag::Carry, overflow);
            gb.set_flag(registers::Flag::AddSubtract);
            gb.set_flag_to(registers::Flag::Zero, new == 0);
            gb.set_flag_to(
                registers::Flag::HalfCarry,
                is_sub_half_carry(current_value, arg),
            );
            current_value
        }
        ALOp::And => {
            let new = current_value & arg;
            gb.reset_flag(registers::Flag::Carry);
            gb.set_flag(registers::Flag::HalfCarry);
            gb.reset_flag(registers::Flag::AddSubtract);
            gb.set_flag_to(registers::Flag::Zero, new == 0);
            new
        }
        ALOp::Or => {
            let new = current_value | arg;
            gb.reset_flag(registers::Flag::Carry);
            gb.reset_flag(registers::Flag::HalfCarry);
            gb.reset_flag(registers::Flag::AddSubtract);
            gb.set_flag_to(registers::Flag::Zero, new == 0);
            new
        }
        ALOp::Xor => {
            let new = current_value ^ arg;
            gb.reset_flag(registers::Flag::Carry);
            gb.reset_flag(registers::Flag::HalfCarry);
            gb.reset_flag(registers::Flag::AddSubtract);
            gb.set_flag_to(registers::Flag::Zero, new == 0);
            new
        }
    }
}

fn targeted_alu(gb: &mut GameBoy, operation: ALOp, src: ByteRegisterTarget) -> StepResult<()> {
    let arg = gb.exec_read_register_target(src)?;
    let result = alu_op(gb, operation, arg);
    gb.write_register_u8(registers::ByteRegister::A, result);
    Ok(())
}

fn literal_alu(gb: &mut GameBoy, operation: ALOp, arg: u8) -> StepResult<()> {
    let result = alu_op(gb, operation, arg);
    gb.write_register_u8(registers::ByteRegister::A, result);
    Ok(())
}

macro_rules! alu_register_target {
    ($name:ident, $opcode:literal, $label:literal, $op:path) => {
        #[derive(OlympiaInstruction)]
        #[olympia(opcode = $opcode, label=$label)]
        struct $name {
            #[olympia(single, mask = 0xA)]
            src: ByteRegisterTarget,
        }

        impl ExecutableInstruction for $name {
            fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
                targeted_alu(gb, $op, self.src)
            }
        }
    };
}

alu_register_target!(AddRegisterTarget, 0x1000_0AAA, "ADD", ALOp::Add);
alu_register_target!(AddCarryRegisterTarget, 0x1000_1AAA, "ADC", ALOp::AddCarry);
alu_register_target!(SubRegisterTarget, 0x1001_0AAA, "SUB", ALOp::Sub);
alu_register_target!(SubCarryRegisterTarget, 0x1001_1AAA, "SBC", ALOp::SubCarry);
alu_register_target!(AndRegisterTarget, 0x1010_0AAA, "AND", ALOp::And);
alu_register_target!(XorRegisterTarget, 0x1010_1AAA, "XOR", ALOp::Xor);
alu_register_target!(OrRegisterTarget, 0x1011_0AAA, "OR", ALOp::Or);
alu_register_target!(CompareRegisterTarget, 0x1011_1AAA, "CP", ALOp::Compare);

macro_rules! alu_literal {
    ($name:ident, $opcode:literal, $label:literal, $op:path) => {
        #[derive(OlympiaInstruction)]
        #[olympia(opcode = $opcode, label=$label)]
        struct $name {
            #[olympia(single)]
            arg: u8,
        }

        impl ExecutableInstruction for $name {
            fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
                literal_alu(gb, $op, self.arg)
            }
        }
    };
}

alu_literal!(AddLiteral, 0x1100_0110, "ADD", ALOp::Add);
alu_literal!(AddCarryLiteral, 0x1100_1110, "ADC", ALOp::AddCarry);
alu_literal!(SubLiteral, 0x1101_0110, "SUB", ALOp::Sub);
alu_literal!(SubCarryLiteral, 0x1101_1110, "SBC", ALOp::SubCarry);
alu_literal!(AndLiteral, 0x1110_0110, "AND", ALOp::And);
alu_literal!(XorLiteral, 0x1110_1110, "XOR", ALOp::Xor);
alu_literal!(OrLiteral, 0x1111_0110, "OR", ALOp::Or);
alu_literal!(CompareLiteral, 0x1111_1110, "CP", ALOp::Compare);

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_A100, label = "INC")]
struct Increment {
    #[olympia(dest, mask = 0xA)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for Increment {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let reg_value = gb.exec_read_register_target(self.target)?;
        let (new, carry) = reg_value.overflowing_add(1);
        gb.set_flag_to(registers::Flag::Zero, new == 0);
        gb.set_flag_to(registers::Flag::Carry, carry);
        gb.reset_flag(registers::Flag::AddSubtract);
        gb.set_flag_to(registers::Flag::HalfCarry, is_add_half_carry(reg_value, 1));
        gb.exec_write_register_target(self.target, new)?;
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_A101, label = "DEC")]
struct Decrement {
    #[olympia(dest, mask = 0xA)]
    target: ByteRegisterTarget,
}

impl ExecutableInstruction for Decrement {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let reg_value = gb.exec_read_register_target(self.target)?;
        let (new, carry) = reg_value.overflowing_sub(1);
        gb.set_flag_to(registers::Flag::Zero, new == 0);
        gb.set_flag_to(registers::Flag::Carry, carry);
        gb.set_flag(registers::Flag::AddSubtract);
        gb.set_flag_to(registers::Flag::HalfCarry, is_sub_half_carry(reg_value, 1));
        gb.exec_write_register_target(self.target, new)?;
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_0011, label = "INC")]
struct Increment16 {
    #[olympia(dest, mask = 0xA)]
    target: registers::StackRegister,
}

impl ExecutableInstruction for Increment16 {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let reg_value = gb.read_register_u16(self.target.into());
        let (new, _carry) = reg_value.overflowing_add(1);
        gb.write_register_u16(self.target.into(), new);
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_1011, label = "INC")]
struct Decrement16 {
    #[olympia(dest, mask = 0xA)]
    target: registers::StackRegister,
}

impl ExecutableInstruction for Decrement16 {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let reg_value = gb.read_register_u16(self.target.into());
        let (new, _carry) = reg_value.overflowing_sub(1);
        gb.write_register_u16(self.target.into(), new);
        gb.cycle();
        Ok(())
    }
}

#[derive(OlympiaInstruction)]
#[olympia(opcode = 0x00AA_1001, label = "INC")]
struct Add16 {
    #[olympia(dest, constant(registers::WordRegister::HL))]
    dest: registers::WordRegister,
    #[olympia(src, mask = 0xA)]
    src: registers::StackRegister,
}

impl ExecutableInstruction for Add16 {
    fn execute(&self, gb: &mut GameBoy) -> StepResult<()> {
        let current_value = gb.read_register_u16(self.dest);
        let value_to_add = gb.read_register_u16(self.src.into());
        let (new, carry) = current_value.overflowing_add(value_to_add);
        gb.set_flag_to(registers::Flag::Zero, new == 0);
        gb.set_flag_to(registers::Flag::Carry, carry);
        gb.reset_flag(registers::Flag::AddSubtract);
        let has_half_carry = (((current_value & 0x0FFF) + (value_to_add & 0x0FFF)) & 0xF000) != 0;
        gb.set_flag_to(registers::Flag::HalfCarry, has_half_carry);
        gb.write_register_u16(self.dest, new);
        gb.cycle();
        Ok(())
    }
}

pub(crate) fn all_alu_opcodes() -> Vec<(u8, Box<dyn ExecutableOpcode>)> {
    vec![
        AddRegisterTargetOpcode::all(),
        AddCarryRegisterTargetOpcode::all(),
        SubRegisterTargetOpcode::all(),
        SubCarryRegisterTargetOpcode::all(),
        AndRegisterTargetOpcode::all(),
        XorRegisterTargetOpcode::all(),
        OrRegisterTargetOpcode::all(),
        CompareRegisterTargetOpcode::all(),
        AddLiteralOpcode::all(),
        AddCarryLiteralOpcode::all(),
        SubLiteralOpcode::all(),
        SubCarryLiteralOpcode::all(),
        AndLiteralOpcode::all(),
        XorLiteralOpcode::all(),
        OrLiteralOpcode::all(),
        CompareLiteralOpcode::all(),
        IncrementOpcode::all(),
        DecrementOpcode::all(),
        Increment16Opcode::all(),
        Decrement16Opcode::all(),
        Add16Opcode::all(),
    ]
    .into_iter()
    .flatten()
    .collect()
}
