use crate::decoder::{
    DecodeError, DecodeResult, OneByteDecoder, ThreeByteAddressDecoder, ThreeByteDataDecoder,
    TwoByteAddressDecoder, TwoByteDataDecoder, TwoByteOffsetDecoder,
};
use crate::instructions::Instruction;
use crate::{instructions, registers};

use olympia_core::address;

enum ByteRegisterLookupResult {
    Register(registers::ByteRegister),
    Memory,
}

fn byte_register_lookup(register_index: u8) -> Option<ByteRegisterLookupResult> {
    match register_index {
        0b000 => Some(ByteRegisterLookupResult::Register(
            registers::ByteRegister::B,
        )),
        0b001 => Some(ByteRegisterLookupResult::Register(
            registers::ByteRegister::C,
        )),
        0b010 => Some(ByteRegisterLookupResult::Register(
            registers::ByteRegister::D,
        )),
        0b011 => Some(ByteRegisterLookupResult::Register(
            registers::ByteRegister::E,
        )),
        0b100 => Some(ByteRegisterLookupResult::Register(
            registers::ByteRegister::H,
        )),
        0b101 => Some(ByteRegisterLookupResult::Register(
            registers::ByteRegister::L,
        )),
        0b110 => Some(ByteRegisterLookupResult::Memory),
        0b111 => Some(ByteRegisterLookupResult::Register(
            registers::ByteRegister::A,
        )),
        _ => None,
    }
}

fn stack_register_lookup(register_index: u8) -> Option<registers::StackRegister> {
    match register_index {
        0b00 => Some(registers::StackRegister::BC),
        0b01 => Some(registers::StackRegister::DE),
        0b10 => Some(registers::StackRegister::HL),
        0b11 => Some(registers::StackRegister::SP),
        _ => None,
    }
}

fn al_op_lookup(op: u8) -> Option<instructions::ALOp> {
    match op {
        0b000 => Some(instructions::ALOp::Add),
        0b001 => Some(instructions::ALOp::AddCarry),
        0b010 => Some(instructions::ALOp::Sub),
        0b011 => Some(instructions::ALOp::SubCarry),
        0b100 => Some(instructions::ALOp::And),
        0b101 => Some(instructions::ALOp::Xor),
        0b110 => Some(instructions::ALOp::Or),
        0b111 => Some(instructions::ALOp::Compare),
        _ => None,
    }
}

fn condition_lookup(condition_code: u8) -> Option<instructions::Condition> {
    match condition_code {
        0b00 => Some(instructions::Condition::NonZero),
        0b01 => Some(instructions::Condition::Zero),
        0b10 => Some(instructions::Condition::NoCarry),
        0b11 => Some(instructions::Condition::Carry),
        _ => None,
    }
}

pub(super) struct Literal;
impl OneByteDecoder for Literal {
    fn decode(&self, opcode: u8) -> DecodeResult<Instruction> {
        Ok(Instruction::Literal(opcode))
    }
}

pub(super) struct Stop;
impl TwoByteDataDecoder for Stop {
    fn decode(&self, _opcode: u8, _data: u8) -> DecodeResult<Instruction> {
        Ok(Instruction::Stop)
    }
}

pub(super) struct RelativeJump;
impl TwoByteOffsetDecoder for RelativeJump {
    fn decode(&self, _opcode: u8, offset: address::AddressOffset) -> DecodeResult<Instruction> {
        Ok(instructions::Jump::RelativeJump(offset).into())
    }
}

pub(super) struct StoreSP;
impl ThreeByteAddressDecoder for StoreSP {
    fn decode(&self, _opcode: u8, addr: address::LiteralAddress) -> DecodeResult<Instruction> {
        Ok(instructions::Stack::StoreStackPointerMemory(addr).into())
    }
}

pub(super) struct LoadConstant16;
impl ThreeByteDataDecoder for LoadConstant16 {
    fn decode(&self, opcode: u8, value: u16) -> DecodeResult<Instruction> {
        let register_bits = 0b0011_0000;
        let register_value = (opcode & register_bits) >> 4;
        let register = stack_register_lookup(register_value)
            .ok_or(DecodeError::UnknownWordRegister(register_value))?;
        Ok(instructions::Load::Constant16(register, value).into())
    }
}

pub(super) struct HighOffsetA;
impl TwoByteAddressDecoder for HighOffsetA {
    fn decode(&self, _opcode: u8, addr: address::HighAddress) -> DecodeResult<Instruction> {
        Ok(instructions::Load::HighOffsetA(addr).into())
    }
}

pub(super) struct AHighOffset;
impl TwoByteAddressDecoder for AHighOffset {
    fn decode(&self, _opcode: u8, addr: address::HighAddress) -> DecodeResult<Instruction> {
        Ok(instructions::Load::AHighOffset(addr).into())
    }
}

pub(super) struct ConditionalRelativeJump;
impl TwoByteOffsetDecoder for ConditionalRelativeJump {
    fn decode(&self, opcode: u8, offset: address::AddressOffset) -> DecodeResult<Instruction> {
        let condition_bits = 0b0001_1000;
        let condition_value = (opcode & condition_bits) >> 3;
        let condition = condition_lookup(condition_value)
            .ok_or(DecodeError::UnknownCondition(condition_value))?;
        Ok(instructions::Jump::RelativeJumpIf(condition, offset).into())
    }
}

pub(super) struct Jump;
impl ThreeByteAddressDecoder for Jump {
    fn decode(&self, _opcode: u8, addr: address::LiteralAddress) -> DecodeResult<Instruction> {
        Ok(instructions::Jump::Jump(addr).into())
    }
}

pub(super) struct Call;
impl ThreeByteAddressDecoder for Call {
    fn decode(&self, _opcode: u8, addr: address::LiteralAddress) -> DecodeResult<Instruction> {
        Ok(instructions::Jump::Call(addr).into())
    }
}

pub(super) struct ConditionalJump;
impl ThreeByteAddressDecoder for ConditionalJump {
    fn decode(&self, opcode: u8, addr: address::LiteralAddress) -> DecodeResult<Instruction> {
        let condition_bits = 0b0001_1000;
        let condition_value = (opcode & condition_bits) >> 3;
        let condition = condition_lookup(condition_value)
            .ok_or(DecodeError::UnknownCondition(condition_value))?;
        Ok(instructions::Jump::JumpIf(condition, addr).into())
    }
}

pub(super) struct ConditionalCall;
impl ThreeByteAddressDecoder for ConditionalCall {
    fn decode(&self, opcode: u8, addr: address::LiteralAddress) -> DecodeResult<Instruction> {
        let condition_bits = 0b0001_1000;
        let condition_value = (opcode & condition_bits) >> 3;
        let condition = condition_lookup(condition_value)
            .ok_or(DecodeError::UnknownCondition(condition_value))?;
        Ok(instructions::Jump::CallIf(condition, addr).into())
    }
}

pub(super) struct LoadConstant;
impl TwoByteDataDecoder for LoadConstant {
    fn decode(&self, opcode: u8, data: u8) -> DecodeResult<Instruction> {
        let register_bits = 0b0011_1000;
        let register_value = (opcode & register_bits) >> 3;
        let register = byte_register_lookup(register_value)
            .ok_or(DecodeError::UnknownByteRegister(register_value))?;
        match register {
            ByteRegisterLookupResult::Memory => Ok(instructions::Load::ConstantMemory(data).into()),
            ByteRegisterLookupResult::Register(reg) => {
                Ok(instructions::Load::Constant(reg, data).into())
            }
        }
    }
}

pub(super) struct ConstantAL;
impl TwoByteDataDecoder for ConstantAL {
    fn decode(&self, opcode: u8, data: u8) -> DecodeResult<Instruction> {
        let operation_bits = 0b0011_1000;
        let operation_value = (opcode & operation_bits) >> 3;
        let operation = al_op_lookup(operation_value)
            .ok_or(DecodeError::UnknownALOperation(operation_value))?;
        Ok(Instruction::ConstantAL(operation, data))
    }
}

pub(super) struct StoreAddress;
impl ThreeByteAddressDecoder for StoreAddress {
    fn decode(&self, _opcode: u8, addr: address::LiteralAddress) -> DecodeResult<Instruction> {
        Ok(instructions::Load::IndirectA(addr).into())
    }
}

pub(super) struct LoadAddress;
impl ThreeByteAddressDecoder for LoadAddress {
    fn decode(&self, _opcode: u8, addr: address::LiteralAddress) -> DecodeResult<Instruction> {
        Ok(instructions::Load::AIndirect(addr).into())
    }
}

pub(super) struct AddToStack;
impl TwoByteOffsetDecoder for AddToStack {
    fn decode(&self, _opcode: u8, offset: address::AddressOffset) -> DecodeResult<Instruction> {
        Ok(instructions::Stack::AddStackPointer(offset).into())
    }
}

pub(super) struct CalcStackOffset;
impl TwoByteOffsetDecoder for CalcStackOffset {
    fn decode(&self, _opcode: u8, offset: address::AddressOffset) -> DecodeResult<Instruction> {
        Ok(instructions::Stack::LoadStackOffset(offset).into())
    }
}

pub(crate) struct Extended;
impl TwoByteDataDecoder for Extended {
    fn decode(&self, _opcode: u8, data: u8) -> DecodeResult<Instruction> {
        use instructions::Extended;
        use ByteRegisterLookupResult::{Memory, Register};
        let high_bits = (data & 0b1100_0000) >> 6;
        let destination = data & 0b0000_0111;
        let reglookup = byte_register_lookup(destination)
            .ok_or(DecodeError::UnknownByteRegister(destination))?;
        if high_bits > 0 {
            let bit = (data & 0b0011_1000) >> 3;
            let extended = match (reglookup, high_bits) {
                (Register(reg), 0b01) => Extended::TestBit(bit, reg),
                (Memory, 0b01) => Extended::TestMemoryBit(bit),
                (Register(reg), 0b10) => Extended::ResetBit(bit, reg),
                (Memory, 0b10) => Extended::ResetMemoryBit(bit),
                (Register(reg), 0b11) => Extended::SetBit(bit, reg),
                (Memory, 0b11) => Extended::SetMemoryBit(bit),
                _ => panic!("Invalid bit pattern"),
            };

            Ok(extended.into())
        } else {
            use instructions::Carry::{Carry, NoCarry};
            use instructions::RotateDirection::{Left, Right};

            let op = (data & 0b0011_0000) >> 4;
            let direction = if (data & 0b0000_1000) == 0 {
                Left
            } else {
                Right
            };

            let extended = match (reglookup, op, direction) {
                (Memory, 0b00, dir) => Extended::RotateMemory(dir, Carry),
                (Memory, 0b01, dir) => Extended::RotateMemory(dir, NoCarry),
                (Memory, 0b10, Left) => Extended::ShiftMemoryZero(Left),
                (Memory, 0b10, Right) => Extended::ShiftMemoryRightExtend,
                (Memory, 0b11, Right) => Extended::ShiftMemoryZero(Right),
                (Memory, 0b11, Left) => Extended::SwapMemory,
                (Register(reg), 0b00, dir) => Extended::Rotate(dir, Carry, reg),
                (Register(reg), 0b01, dir) => Extended::Rotate(dir, NoCarry, reg),
                (Register(reg), 0b10, Left) => Extended::ShiftZero(Left, reg),
                (Register(reg), 0b10, Right) => Extended::ShiftRightExtend(reg),
                (Register(reg), 0b11, Right) => Extended::ShiftZero(Right, reg),
                (Register(reg), 0b11, Left) => Extended::Swap(reg),
                _ => panic!("Invalid bit pattern"),
            };

            Ok(extended.into())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_conditional_relative_jump() {
        use instructions::Condition as cond;
        use instructions::Jump::RelativeJumpIf;
        let idecoder = ConditionalRelativeJump {};

        let addr: address::AddressOffset = 0x12.into();

        assert_eq!(
            idecoder.decode(0x20, addr),
            Ok(RelativeJumpIf(cond::NonZero, addr).into())
        );
        assert_eq!(
            idecoder.decode(0x28, addr),
            Ok(RelativeJumpIf(cond::Zero, addr).into())
        );
        assert_eq!(
            idecoder.decode(0x30, addr),
            Ok(RelativeJumpIf(cond::NoCarry, addr).into())
        );
        assert_eq!(
            idecoder.decode(0x38, addr),
            Ok(RelativeJumpIf(cond::Carry, addr).into())
        );
    }

    #[test]
    fn test_load_constant_16() {
        use crate::registers::StackRegister as w;
        use instructions::Load;

        let idecoder = LoadConstant16 {};
        assert_eq!(
            idecoder.decode(0x01, 0x2310),
            Ok(Load::Constant16(w::BC, 0x2310).into())
        );
        assert_eq!(
            idecoder.decode(0x11, 0x2317),
            Ok(Load::Constant16(w::DE, 0x2317).into())
        );
        assert_eq!(
            idecoder.decode(0x21, 0x2350),
            Ok(Load::Constant16(w::HL, 0x2350).into())
        );
        assert_eq!(
            idecoder.decode(0x31, 0x2410),
            Ok(Load::Constant16(w::SP, 0x2410).into())
        );
    }

    #[test]
    fn test_indirect_a() {
        let idecoder = StoreAddress {};

        let addr: address::LiteralAddress = 0x1012.into();

        assert_eq!(
            idecoder.decode(0xEA, addr),
            Ok(instructions::Load::IndirectA(addr).into())
        );
    }

    #[test]
    fn test_a_indirect() {
        let idecoder = LoadAddress {};

        let addr: address::LiteralAddress = 0x1012.into();

        assert_eq!(
            idecoder.decode(0xFA, addr),
            Ok(instructions::Load::AIndirect(addr).into())
        );
    }

    #[test]
    fn test_jump() {
        let idecoder = Jump {};

        let addr: address::LiteralAddress = 0x1012.into();

        assert_eq!(
            idecoder.decode(0xC2, addr),
            Ok(instructions::Jump::Jump(addr).into())
        );
    }

    #[test]
    fn test_call() {
        let idecoder = Call {};

        let addr: address::LiteralAddress = 0x1012.into();

        assert_eq!(
            idecoder.decode(0xC2, addr),
            Ok(instructions::Jump::Call(addr).into())
        );
    }

    #[test]
    fn test_conditional_jump() {
        use instructions::Condition as cond;
        use instructions::Jump::JumpIf;
        let idecoder = ConditionalJump {};

        let addr: address::LiteralAddress = 0x1012.into();

        assert_eq!(
            idecoder.decode(0xC2, addr),
            Ok(JumpIf(cond::NonZero, addr).into())
        );
        assert_eq!(
            idecoder.decode(0xCA, addr),
            Ok(JumpIf(cond::Zero, addr).into())
        );
        assert_eq!(
            idecoder.decode(0xD2, addr),
            Ok(JumpIf(cond::NoCarry, addr).into())
        );
        assert_eq!(
            idecoder.decode(0xDA, addr),
            Ok(JumpIf(cond::Carry, addr).into())
        );
    }

    #[test]
    fn test_conditional_call() {
        use instructions::Condition as cond;
        use instructions::Jump::CallIf;
        let idecoder = ConditionalCall {};

        let addr: address::LiteralAddress = 0x1012.into();

        assert_eq!(
            idecoder.decode(0xC4, addr),
            Ok(CallIf(cond::NonZero, addr).into())
        );
        assert_eq!(
            idecoder.decode(0xCC, addr),
            Ok(CallIf(cond::Zero, addr).into())
        );
        assert_eq!(
            idecoder.decode(0xD4, addr),
            Ok(CallIf(cond::NoCarry, addr).into())
        );
        assert_eq!(
            idecoder.decode(0xDC, addr),
            Ok(CallIf(cond::Carry, addr).into())
        );
    }

    #[test]
    fn test_load_constant() {
        use crate::registers::ByteRegister as b;
        let idecoder = LoadConstant {};

        assert_eq!(
            idecoder.decode(0x06, 0x12),
            Ok(instructions::Load::Constant(b::B, 0x12).into())
        );
        assert_eq!(
            idecoder.decode(0x0E, 0x12),
            Ok(instructions::Load::Constant(b::C, 0x12).into())
        );
        assert_eq!(
            idecoder.decode(0x16, 0x12),
            Ok(instructions::Load::Constant(b::D, 0x12).into())
        );
        assert_eq!(
            idecoder.decode(0x1E, 0x12),
            Ok(instructions::Load::Constant(b::E, 0x12).into())
        );
        assert_eq!(
            idecoder.decode(0x26, 0x12),
            Ok(instructions::Load::Constant(b::H, 0x12).into())
        );
        assert_eq!(
            idecoder.decode(0x2E, 0x12),
            Ok(instructions::Load::Constant(b::L, 0x12).into())
        );
        assert_eq!(
            idecoder.decode(0x36, 0x24),
            Ok(instructions::Load::ConstantMemory(0x24).into())
        );
        assert_eq!(
            idecoder.decode(0x3E, 0x12),
            Ok(instructions::Load::Constant(b::A, 0x12).into())
        );
    }

    #[test]
    fn test_constant_al() {
        use crate::instructions::ALOp as op;
        let idecoder = ConstantAL {};

        assert_eq!(
            idecoder.decode(0xC6, 0x12),
            Ok(Instruction::ConstantAL(op::Add, 0x12))
        );
        assert_eq!(
            idecoder.decode(0xCE, 0x12),
            Ok(Instruction::ConstantAL(op::AddCarry, 0x12))
        );
        assert_eq!(
            idecoder.decode(0xD6, 0x12),
            Ok(Instruction::ConstantAL(op::Sub, 0x12))
        );
        assert_eq!(
            idecoder.decode(0xDE, 0x12),
            Ok(Instruction::ConstantAL(op::SubCarry, 0x12))
        );
        assert_eq!(
            idecoder.decode(0xE6, 0x12),
            Ok(Instruction::ConstantAL(op::And, 0x12))
        );
        assert_eq!(
            idecoder.decode(0xEE, 0x12),
            Ok(Instruction::ConstantAL(op::Xor, 0x12))
        );
        assert_eq!(
            idecoder.decode(0xF6, 0x12),
            Ok(Instruction::ConstantAL(op::Or, 0x12))
        );
        assert_eq!(
            idecoder.decode(0xFE, 0x12),
            Ok(Instruction::ConstantAL(op::Compare, 0x12))
        );
    }

    #[test]
    fn test_store_high_offset() {
        let idecoder = HighOffsetA {};

        assert_eq!(
            idecoder.decode(0xE0, 0x55.into()),
            Ok(instructions::Load::HighOffsetA(0x55.into()).into())
        );
    }

    #[test]
    fn test_load_high_offset() {
        let idecoder = AHighOffset {};

        assert_eq!(
            idecoder.decode(0xF0, 0x55.into()),
            Ok(instructions::Load::AHighOffset(0x55.into()).into())
        );
    }

    #[test]
    fn test_add_stack_pointer() {
        let idecoder = AddToStack {};

        assert_eq!(
            idecoder.decode(0xE8, 0x55.into()),
            Ok(instructions::Stack::AddStackPointer(0x55.into()).into())
        );
    }

    #[test]
    fn test_calc_stack_offset() {
        let idecoder = CalcStackOffset {};

        assert_eq!(
            idecoder.decode(0xF8, 0x55.into()),
            Ok(instructions::Stack::LoadStackOffset(0x55.into()).into())
        );
    }

    #[test]
    fn test_extended_rotate() {
        use crate::registers::ByteRegister as b;
        use instructions::Carry::{Carry, NoCarry};
        use instructions::Extended;
        use instructions::RotateDirection::{Left, Right};
        let idecoder = super::Extended {};

        assert_eq!(
            idecoder.decode(0xCB, 0x01),
            Ok(Extended::Rotate(Left, Carry, b::C).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x14),
            Ok(Extended::Rotate(Left, NoCarry, b::H).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x0D),
            Ok(Extended::Rotate(Right, Carry, b::L).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x1F),
            Ok(Extended::Rotate(Right, NoCarry, b::A).into())
        );

        assert_eq!(
            idecoder.decode(0xCB, 0x06),
            Ok(Extended::RotateMemory(Left, Carry).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x16),
            Ok(Extended::RotateMemory(Left, NoCarry).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x0E),
            Ok(Extended::RotateMemory(Right, Carry).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x1E),
            Ok(Extended::RotateMemory(Right, NoCarry).into())
        );
    }

    #[test]
    fn test_extended_shift() {
        use crate::registers::ByteRegister as b;
        use instructions::Extended;
        use instructions::RotateDirection::{Left, Right};
        let idecoder = super::Extended {};

        assert_eq!(
            idecoder.decode(0xCB, 0x21),
            Ok(Extended::ShiftZero(Left, b::C).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x2F),
            Ok(Extended::ShiftRightExtend(b::A).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x3B),
            Ok(Extended::ShiftZero(Right, b::E).into())
        );

        assert_eq!(
            idecoder.decode(0xCB, 0x26),
            Ok(Extended::ShiftMemoryZero(Left).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x2E),
            Ok(Extended::ShiftMemoryRightExtend.into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x3E),
            Ok(Extended::ShiftMemoryZero(Right).into())
        );
    }

    #[test]
    fn test_extended_swap() {
        use crate::registers::ByteRegister as b;
        use instructions::Extended;
        let idecoder = super::Extended {};

        assert_eq!(idecoder.decode(0xCB, 0x31), Ok(Extended::Swap(b::C).into()));
        assert_eq!(idecoder.decode(0xCB, 0x36), Ok(Extended::SwapMemory.into()));
    }

    #[test]
    fn test_extended_test_bit() {
        use crate::registers::ByteRegister as b;
        use instructions::Extended;
        let idecoder = super::Extended {};

        assert_eq!(
            idecoder.decode(0xCB, 0x74),
            Ok(Extended::TestBit(6, b::H).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x58),
            Ok(Extended::TestBit(3, b::B).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x6E),
            Ok(Extended::TestMemoryBit(5).into())
        );
    }

    #[test]
    fn test_extended_reset_bit() {
        use crate::registers::ByteRegister as b;
        use instructions::Extended;
        let idecoder = super::Extended {};

        assert_eq!(
            idecoder.decode(0xCB, 0x80),
            Ok(Extended::ResetBit(0, b::B).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x93),
            Ok(Extended::ResetBit(2, b::E).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0x8E),
            Ok(Extended::ResetMemoryBit(1).into())
        );
    }

    #[test]
    fn test_extended_set_bit() {
        use crate::registers::ByteRegister as b;
        use instructions::Extended;
        let idecoder = super::Extended {};

        assert_eq!(
            idecoder.decode(0xCB, 0xC4),
            Ok(Extended::SetBit(0, b::H).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0xD5),
            Ok(Extended::SetBit(2, b::L).into())
        );
        assert_eq!(
            idecoder.decode(0xCB, 0xDE),
            Ok(Extended::SetMemoryBit(3).into())
        );
    }
}
