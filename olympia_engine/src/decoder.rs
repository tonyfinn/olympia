//! Contains decoding logic for gameboy instructions

pub(crate) mod idecoders;

use alloc::boxed::Box;
#[cfg(feature = "disassembler")]
use alloc::string::String;
use alloc::vec::Vec;

use crate::{instructions, instructions::Instruction, registers};

use olympia_core::address;

#[derive(PartialEq, Eq, Debug)]
/// Represents an error decoding a given instruction
///
/// Apart from IncompleteInstruction, the remainder of these errors usually
/// indicate a problem in the emulator frontend, passing invalid data for decoding,
/// and should not occur normally when executing a given ROM.
pub enum DecodeError {
    /// The instruction requires additional bytes that were not present.assert_eq!
    ///
    /// This will usually occur for a truncated ROM file
    IncompleteInstruction,
    UnknownOpcode(u8),
    UnknownCondition(u8),
    UnknownByteRegister(u8),
    UnknownWordRegister(u8),
    UnknownALOperation(u8),
    UnknownExtendedInstruction(u8),
}

pub type DecodeResult<T> = Result<T, DecodeError>;

#[derive(Default)]
/// Struct that can be used to decode or disassemble instructions in a incremental fashion.
///
/// See also [`decode`](fn.decode.html) and [`disassemble`](fn.disassemble.html)
/// for helper methods to operate on an entire ROM.
pub struct Decoder {
    instruction_decoders: Vec<InstructionDecoder>,
}

enum InstructionDecoder {
    Basic(Instruction),
    OneByte(Box<dyn OneByteDecoder>),
    TwoByteData(Box<dyn TwoByteDataDecoder>),
    TwoByteOffset(Box<dyn TwoByteOffsetDecoder>),
    TwoByteAddress(Box<dyn TwoByteAddressDecoder>),
    ThreeByteData(Box<dyn ThreeByteDataDecoder>),
    ThreeByteAddress(Box<dyn ThreeByteAddressDecoder>),
}

#[cfg(feature = "disassembler")]
#[derive(PartialEq, Eq, Debug)]
/// Represents an instruction that has been disassembled.
///
/// The first field indicates the numeric value of the instruction,
/// while the second is a textual label corresponding to the instruction.
pub enum DisassembledInstruction {
    OneByte(u8, String),
    TwoByte(u16, String),
    ThreeByte(u32, String),
}

impl DisassembledInstruction {
    /// Returns the text of a given instruction
    pub fn text(&self) -> &str {
        match self {
            DisassembledInstruction::OneByte(_, text) => &text,
            DisassembledInstruction::TwoByte(_, text) => &text,
            DisassembledInstruction::ThreeByte(_, text) => &text,
        }
    }
}

trait OneByteDecoder {
    fn decode(&self, opcode: u8) -> DecodeResult<Instruction>;
}

pub(crate) trait TwoByteDataDecoder {
    fn decode(&self, opcode: u8, data: u8) -> DecodeResult<Instruction>;
}

trait TwoByteOffsetDecoder {
    fn decode(&self, opcode: u8, data: address::AddressOffset) -> DecodeResult<Instruction>;
}

trait TwoByteAddressDecoder {
    fn decode(&self, opcode: u8, data: address::HighAddress) -> DecodeResult<Instruction>;
}

trait ThreeByteAddressDecoder {
    fn decode(&self, opcode: u8, data: address::LiteralAddress) -> DecodeResult<Instruction>;
}

trait ThreeByteDataDecoder {
    fn decode(&self, opcode: u8, data: u16) -> DecodeResult<Instruction>;
}

fn read_byte(iter: &mut dyn Iterator<Item = u8>) -> DecodeResult<u8> {
    iter.next().ok_or(DecodeError::IncompleteInstruction)
}

fn read_word(iter: &mut dyn Iterator<Item = u8>) -> DecodeResult<u16> {
    let low_byte = read_byte(iter)?;
    let high_byte = read_byte(iter)?;

    Ok(u16::from_le_bytes([low_byte, high_byte]))
}

impl InstructionDecoder {
    pub(crate) fn decode(
        &self,
        opcode: u8,
        iter: &mut dyn Iterator<Item = u8>,
    ) -> DecodeResult<Instruction> {
        use InstructionDecoder::*;
        match self {
            Basic(instr) => Ok(instr.clone()),
            OneByte(decoder) => decoder.decode(opcode),
            TwoByteAddress(decoder) => decoder.decode(opcode, read_byte(iter)?.into()),
            TwoByteData(decoder) => decoder.decode(opcode, read_byte(iter)?),
            TwoByteOffset(decoder) => decoder.decode(opcode, read_byte(iter)?.into()),
            ThreeByteData(decoder) => decoder.decode(opcode, read_word(iter)?),
            ThreeByteAddress(decoder) => decoder.decode(opcode, read_word(iter)?.into()),
        }
    }

    #[cfg(feature = "disassembler")]
    pub(crate) fn disassemble(
        &self,
        opcode: u8,
        iter: &mut dyn Iterator<Item = u8>,
    ) -> DecodeResult<DisassembledInstruction> {
        use crate::disassembler::Disassemble;
        use DisassembledInstruction as dis;
        use InstructionDecoder::*;
        match self {
            Basic(instr) => Ok(dis::OneByte(opcode, Disassemble::disassemble(instr))),
            OneByte(decoder) => Ok(dis::OneByte(
                opcode,
                Disassemble::disassemble(&decoder.decode(opcode)?),
            )),
            TwoByteAddress(decoder) => {
                let byte = read_byte(iter)?;
                let instr = decoder.decode(opcode, byte.into())?;
                Ok(dis::TwoByte(
                    u16::from_le_bytes([byte, opcode]),
                    Disassemble::disassemble(&instr),
                ))
            }
            TwoByteData(decoder) => {
                let byte = read_byte(iter)?;
                let instr = decoder.decode(opcode, byte)?;
                Ok(dis::TwoByte(
                    u16::from_le_bytes([byte, opcode]),
                    Disassemble::disassemble(&instr),
                ))
            }
            TwoByteOffset(decoder) => {
                let byte = read_byte(iter)?;
                let instr = decoder.decode(opcode, byte.into())?;
                Ok(dis::TwoByte(
                    u16::from_le_bytes([byte, opcode]),
                    Disassemble::disassemble(&instr),
                ))
            }
            ThreeByteData(decoder) => {
                let word = read_word(iter)?;
                let instr = decoder.decode(opcode, word)?;
                let [first, second] = word.to_le_bytes();
                Ok(dis::ThreeByte(
                    u32::from_be_bytes([0, opcode, first, second]),
                    Disassemble::disassemble(&instr),
                ))
            }
            ThreeByteAddress(decoder) => {
                let word = read_word(iter)?;
                let instr = decoder.decode(opcode, word.into())?;
                let [first, second] = word.to_le_bytes();
                Ok(dis::ThreeByte(
                    u32::from_be_bytes([0, opcode, first, second]),
                    Disassemble::disassemble(&instr),
                ))
            }
        }
    }
}

impl Decoder {
    pub fn new() -> Decoder {
        use instructions::ALOp as al;
        use instructions::Carry as carry;
        use instructions::Condition as cond;
        use instructions::Increment as inc;
        use instructions::RotateDirection as rotdir;
        use registers::AccRegister as aw;
        use registers::ByteRegister as b;
        use registers::StackRegister as sw;
        use registers::WordRegister as w;
        use InstructionDecoder::*;

        let mut instruction_decoders: Vec<InstructionDecoder> = Vec::with_capacity(0xff);
        for _ in 0..=0xff {
            // Default to literal
            instruction_decoders.push(OneByte(Box::new(idecoders::Literal)));
        }

        // 0x
        instruction_decoders[0x00] = Basic(Instruction::NOP);
        instruction_decoders[0x01] = ThreeByteData(Box::new(idecoders::LoadConstant16));
        instruction_decoders[0x02] = Basic(instructions::Load::MemoryRegister(w::BC, b::A).into());
        instruction_decoders[0x03] = Basic(instructions::RegisterAL::Increment16(sw::BC).into());
        instruction_decoders[0x04] = Basic(instructions::RegisterAL::Increment(b::B).into());
        instruction_decoders[0x05] = Basic(instructions::RegisterAL::Decrement(b::B).into());
        instruction_decoders[0x06] = TwoByteData(Box::new(idecoders::LoadConstant));
        instruction_decoders[0x07] = Basic(Instruction::Rotate(rotdir::Left, carry::Carry));

        instruction_decoders[0x08] = ThreeByteAddress(Box::new(idecoders::StoreSP));
        instruction_decoders[0x09] = Basic(instructions::RegisterAL::Add16(sw::BC).into());
        instruction_decoders[0x0A] = Basic(instructions::Load::RegisterMemory(b::A, w::BC).into());
        instruction_decoders[0x0B] = Basic(instructions::RegisterAL::Decrement16(sw::BC).into());
        instruction_decoders[0x0C] = Basic(instructions::RegisterAL::Increment(b::C).into());
        instruction_decoders[0x0D] = Basic(instructions::RegisterAL::Decrement(b::C).into());
        instruction_decoders[0x0E] = TwoByteData(Box::new(idecoders::LoadConstant));
        instruction_decoders[0x0F] = Basic(Instruction::Rotate(rotdir::Right, carry::Carry));

        // 1x
        instruction_decoders[0x10] = TwoByteData(Box::new(idecoders::Stop));
        instruction_decoders[0x11] = ThreeByteData(Box::new(idecoders::LoadConstant16));
        instruction_decoders[0x12] = Basic(instructions::Load::MemoryRegister(w::DE, b::A).into());
        instruction_decoders[0x13] = Basic(instructions::RegisterAL::Increment16(sw::SP).into());
        instruction_decoders[0x14] = Basic(instructions::RegisterAL::Increment(b::D).into());
        instruction_decoders[0x15] = Basic(instructions::RegisterAL::Decrement(b::D).into());
        instruction_decoders[0x16] = TwoByteData(Box::new(idecoders::LoadConstant));
        instruction_decoders[0x17] = Basic(Instruction::Rotate(rotdir::Left, carry::NoCarry));

        instruction_decoders[0x18] = TwoByteOffset(Box::new(idecoders::RelativeJump));
        instruction_decoders[0x19] = Basic(instructions::RegisterAL::Add16(sw::DE).into());
        instruction_decoders[0x1A] = Basic(instructions::Load::RegisterMemory(b::A, w::BC).into());
        instruction_decoders[0x1B] = Basic(instructions::RegisterAL::Decrement16(sw::DE).into());
        instruction_decoders[0x1C] = Basic(instructions::RegisterAL::Increment(b::E).into());
        instruction_decoders[0x1D] = Basic(instructions::RegisterAL::Decrement(b::E).into());
        instruction_decoders[0x1E] = TwoByteData(Box::new(idecoders::LoadConstant));
        instruction_decoders[0x1F] = Basic(Instruction::Rotate(rotdir::Right, carry::NoCarry));

        // 2x
        instruction_decoders[0x20] = TwoByteOffset(Box::new(idecoders::ConditionalRelativeJump));
        instruction_decoders[0x21] = ThreeByteData(Box::new(idecoders::LoadConstant16));
        instruction_decoders[0x22] = Basic(instructions::Load::Increment16A(inc::Increment).into());
        instruction_decoders[0x23] = Basic(instructions::RegisterAL::Increment16(sw::HL).into());
        instruction_decoders[0x24] = Basic(instructions::RegisterAL::Increment(b::H).into());
        instruction_decoders[0x25] = Basic(instructions::RegisterAL::Decrement(b::H).into());
        instruction_decoders[0x26] = TwoByteData(Box::new(idecoders::LoadConstant));
        instruction_decoders[0x27] = Basic(Instruction::AToBCD);

        instruction_decoders[0x28] = TwoByteOffset(Box::new(idecoders::ConditionalRelativeJump));
        instruction_decoders[0x29] = Basic(instructions::RegisterAL::Add16(sw::HL).into());
        instruction_decoders[0x2A] = Basic(instructions::Load::AIncrement16(inc::Increment).into());
        instruction_decoders[0x2B] = Basic(instructions::RegisterAL::Decrement16(sw::HL).into());
        instruction_decoders[0x2C] = Basic(instructions::RegisterAL::Increment(b::L).into());
        instruction_decoders[0x2D] = Basic(instructions::RegisterAL::Decrement(b::L).into());
        instruction_decoders[0x2E] = TwoByteData(Box::new(idecoders::LoadConstant));
        instruction_decoders[0x2F] = Basic(Instruction::InvertA);

        // 3x
        instruction_decoders[0x30] = TwoByteOffset(Box::new(idecoders::ConditionalRelativeJump));
        instruction_decoders[0x31] = ThreeByteData(Box::new(idecoders::LoadConstant16));
        instruction_decoders[0x32] = Basic(instructions::Load::Increment16A(inc::Decrement).into());
        instruction_decoders[0x33] = Basic(instructions::RegisterAL::Increment16(sw::HL).into());
        instruction_decoders[0x34] = Basic(Instruction::MemoryIncrement(inc::Increment));
        instruction_decoders[0x35] = Basic(Instruction::MemoryIncrement(inc::Decrement));
        instruction_decoders[0x36] = TwoByteData(Box::new(idecoders::LoadConstant));
        instruction_decoders[0x37] = Basic(Instruction::SetCarry);

        instruction_decoders[0x38] = TwoByteOffset(Box::new(idecoders::ConditionalRelativeJump));
        instruction_decoders[0x39] = Basic(instructions::RegisterAL::Add16(sw::HL).into());
        instruction_decoders[0x3A] = Basic(instructions::Load::AIncrement16(inc::Decrement).into());
        instruction_decoders[0x3B] = Basic(instructions::RegisterAL::Decrement16(sw::HL).into());
        instruction_decoders[0x3C] = Basic(instructions::RegisterAL::Increment(b::A).into());
        instruction_decoders[0x3D] = Basic(instructions::RegisterAL::Decrement(b::A).into());
        instruction_decoders[0x3E] = TwoByteData(Box::new(idecoders::LoadConstant));
        instruction_decoders[0x3F] = Basic(Instruction::InvertCarry);

        // 4x
        instruction_decoders[0x40] = Basic(instructions::Load::RegisterRegister(b::B, b::B).into());
        instruction_decoders[0x41] = Basic(instructions::Load::RegisterRegister(b::B, b::C).into());
        instruction_decoders[0x42] = Basic(instructions::Load::RegisterRegister(b::B, b::D).into());
        instruction_decoders[0x43] = Basic(instructions::Load::RegisterRegister(b::B, b::E).into());
        instruction_decoders[0x44] = Basic(instructions::Load::RegisterRegister(b::B, b::H).into());
        instruction_decoders[0x45] = Basic(instructions::Load::RegisterRegister(b::B, b::L).into());
        instruction_decoders[0x46] = Basic(instructions::Load::RegisterMemory(b::B, w::HL).into());
        instruction_decoders[0x47] = Basic(instructions::Load::RegisterRegister(b::B, b::A).into());

        instruction_decoders[0x48] = Basic(instructions::Load::RegisterRegister(b::C, b::B).into());
        instruction_decoders[0x49] = Basic(instructions::Load::RegisterRegister(b::C, b::C).into());
        instruction_decoders[0x4A] = Basic(instructions::Load::RegisterRegister(b::C, b::D).into());
        instruction_decoders[0x4B] = Basic(instructions::Load::RegisterRegister(b::C, b::E).into());
        instruction_decoders[0x4C] = Basic(instructions::Load::RegisterRegister(b::C, b::H).into());
        instruction_decoders[0x4D] = Basic(instructions::Load::RegisterRegister(b::C, b::L).into());
        instruction_decoders[0x4E] = Basic(instructions::Load::RegisterMemory(b::C, w::HL).into());
        instruction_decoders[0x4F] = Basic(instructions::Load::RegisterRegister(b::C, b::A).into());

        // 5x
        instruction_decoders[0x50] = Basic(instructions::Load::RegisterRegister(b::D, b::B).into());
        instruction_decoders[0x51] = Basic(instructions::Load::RegisterRegister(b::D, b::C).into());
        instruction_decoders[0x52] = Basic(instructions::Load::RegisterRegister(b::D, b::D).into());
        instruction_decoders[0x53] = Basic(instructions::Load::RegisterRegister(b::D, b::E).into());
        instruction_decoders[0x54] = Basic(instructions::Load::RegisterRegister(b::D, b::H).into());
        instruction_decoders[0x55] = Basic(instructions::Load::RegisterRegister(b::D, b::L).into());
        instruction_decoders[0x56] = Basic(instructions::Load::RegisterMemory(b::D, w::HL).into());
        instruction_decoders[0x57] = Basic(instructions::Load::RegisterRegister(b::D, b::A).into());

        instruction_decoders[0x58] = Basic(instructions::Load::RegisterRegister(b::E, b::B).into());
        instruction_decoders[0x59] = Basic(instructions::Load::RegisterRegister(b::E, b::C).into());
        instruction_decoders[0x5A] = Basic(instructions::Load::RegisterRegister(b::E, b::D).into());
        instruction_decoders[0x5B] = Basic(instructions::Load::RegisterRegister(b::E, b::E).into());
        instruction_decoders[0x5C] = Basic(instructions::Load::RegisterRegister(b::E, b::H).into());
        instruction_decoders[0x5D] = Basic(instructions::Load::RegisterRegister(b::E, b::L).into());
        instruction_decoders[0x5E] = Basic(instructions::Load::RegisterMemory(b::E, w::HL).into());
        instruction_decoders[0x5F] = Basic(instructions::Load::RegisterRegister(b::E, b::A).into());

        // 6x
        instruction_decoders[0x60] = Basic(instructions::Load::RegisterRegister(b::H, b::B).into());
        instruction_decoders[0x61] = Basic(instructions::Load::RegisterRegister(b::H, b::C).into());
        instruction_decoders[0x62] = Basic(instructions::Load::RegisterRegister(b::H, b::D).into());
        instruction_decoders[0x63] = Basic(instructions::Load::RegisterRegister(b::H, b::E).into());
        instruction_decoders[0x64] = Basic(instructions::Load::RegisterRegister(b::H, b::H).into());
        instruction_decoders[0x65] = Basic(instructions::Load::RegisterRegister(b::H, b::L).into());
        instruction_decoders[0x66] = Basic(instructions::Load::RegisterMemory(b::H, w::HL).into());
        instruction_decoders[0x67] = Basic(instructions::Load::RegisterRegister(b::H, b::A).into());

        instruction_decoders[0x68] = Basic(instructions::Load::RegisterRegister(b::L, b::B).into());
        instruction_decoders[0x69] = Basic(instructions::Load::RegisterRegister(b::L, b::C).into());
        instruction_decoders[0x6A] = Basic(instructions::Load::RegisterRegister(b::L, b::D).into());
        instruction_decoders[0x6B] = Basic(instructions::Load::RegisterRegister(b::L, b::E).into());
        instruction_decoders[0x6C] = Basic(instructions::Load::RegisterRegister(b::L, b::H).into());
        instruction_decoders[0x6D] = Basic(instructions::Load::RegisterRegister(b::L, b::L).into());
        instruction_decoders[0x6E] = Basic(instructions::Load::RegisterMemory(b::L, w::HL).into());
        instruction_decoders[0x6F] = Basic(instructions::Load::RegisterRegister(b::L, b::A).into());

        // 7x
        instruction_decoders[0x70] = Basic(instructions::Load::MemoryRegister(w::HL, b::B).into());
        instruction_decoders[0x71] = Basic(instructions::Load::MemoryRegister(w::HL, b::C).into());
        instruction_decoders[0x72] = Basic(instructions::Load::MemoryRegister(w::HL, b::D).into());
        instruction_decoders[0x73] = Basic(instructions::Load::MemoryRegister(w::HL, b::E).into());
        instruction_decoders[0x74] = Basic(instructions::Load::MemoryRegister(w::HL, b::H).into());
        instruction_decoders[0x75] = Basic(instructions::Load::MemoryRegister(w::HL, b::L).into());
        instruction_decoders[0x76] = Basic(Instruction::Halt);
        instruction_decoders[0x77] = Basic(instructions::Load::MemoryRegister(w::HL, b::A).into());

        instruction_decoders[0x78] = Basic(instructions::Load::RegisterRegister(b::A, b::B).into());
        instruction_decoders[0x79] = Basic(instructions::Load::RegisterRegister(b::A, b::C).into());
        instruction_decoders[0x7A] = Basic(instructions::Load::RegisterRegister(b::A, b::D).into());
        instruction_decoders[0x7B] = Basic(instructions::Load::RegisterRegister(b::A, b::E).into());
        instruction_decoders[0x7C] = Basic(instructions::Load::RegisterRegister(b::A, b::H).into());
        instruction_decoders[0x7D] = Basic(instructions::Load::RegisterRegister(b::A, b::L).into());
        instruction_decoders[0x7E] = Basic(instructions::Load::RegisterMemory(b::A, w::HL).into());
        instruction_decoders[0x7F] = Basic(instructions::Load::RegisterRegister(b::A, b::A).into());

        // 8x
        instruction_decoders[0x80] = Basic(instructions::RegisterAL::ByteOp(al::Add, b::B).into());
        instruction_decoders[0x81] = Basic(instructions::RegisterAL::ByteOp(al::Add, b::C).into());
        instruction_decoders[0x82] = Basic(instructions::RegisterAL::ByteOp(al::Add, b::D).into());
        instruction_decoders[0x83] = Basic(instructions::RegisterAL::ByteOp(al::Add, b::E).into());
        instruction_decoders[0x84] = Basic(instructions::RegisterAL::ByteOp(al::Add, b::H).into());
        instruction_decoders[0x85] = Basic(instructions::RegisterAL::ByteOp(al::Add, b::L).into());
        instruction_decoders[0x86] = Basic(Instruction::MemoryAL(al::Add));
        instruction_decoders[0x87] = Basic(instructions::RegisterAL::ByteOp(al::Add, b::A).into());

        instruction_decoders[0x88] =
            Basic(instructions::RegisterAL::ByteOp(al::AddCarry, b::B).into());
        instruction_decoders[0x89] =
            Basic(instructions::RegisterAL::ByteOp(al::AddCarry, b::C).into());
        instruction_decoders[0x8A] =
            Basic(instructions::RegisterAL::ByteOp(al::AddCarry, b::D).into());
        instruction_decoders[0x8B] =
            Basic(instructions::RegisterAL::ByteOp(al::AddCarry, b::E).into());
        instruction_decoders[0x8C] =
            Basic(instructions::RegisterAL::ByteOp(al::AddCarry, b::H).into());
        instruction_decoders[0x8D] =
            Basic(instructions::RegisterAL::ByteOp(al::AddCarry, b::L).into());
        instruction_decoders[0x8E] = Basic(Instruction::MemoryAL(al::AddCarry));
        instruction_decoders[0x8F] =
            Basic(instructions::RegisterAL::ByteOp(al::AddCarry, b::A).into());

        // 9x
        instruction_decoders[0x90] = Basic(instructions::RegisterAL::ByteOp(al::Sub, b::B).into());
        instruction_decoders[0x91] = Basic(instructions::RegisterAL::ByteOp(al::Sub, b::C).into());
        instruction_decoders[0x92] = Basic(instructions::RegisterAL::ByteOp(al::Sub, b::D).into());
        instruction_decoders[0x93] = Basic(instructions::RegisterAL::ByteOp(al::Sub, b::E).into());
        instruction_decoders[0x94] = Basic(instructions::RegisterAL::ByteOp(al::Sub, b::H).into());
        instruction_decoders[0x95] = Basic(instructions::RegisterAL::ByteOp(al::Sub, b::L).into());
        instruction_decoders[0x96] = Basic(Instruction::MemoryAL(al::Sub));
        instruction_decoders[0x97] = Basic(instructions::RegisterAL::ByteOp(al::Sub, b::A).into());

        instruction_decoders[0x98] =
            Basic(instructions::RegisterAL::ByteOp(al::SubCarry, b::B).into());
        instruction_decoders[0x99] =
            Basic(instructions::RegisterAL::ByteOp(al::SubCarry, b::C).into());
        instruction_decoders[0x9A] =
            Basic(instructions::RegisterAL::ByteOp(al::SubCarry, b::D).into());
        instruction_decoders[0x9B] =
            Basic(instructions::RegisterAL::ByteOp(al::SubCarry, b::E).into());
        instruction_decoders[0x9C] =
            Basic(instructions::RegisterAL::ByteOp(al::SubCarry, b::H).into());
        instruction_decoders[0x9D] =
            Basic(instructions::RegisterAL::ByteOp(al::SubCarry, b::L).into());
        instruction_decoders[0x9E] = Basic(Instruction::MemoryAL(al::SubCarry));
        instruction_decoders[0x9F] =
            Basic(instructions::RegisterAL::ByteOp(al::SubCarry, b::A).into());

        // Ax
        instruction_decoders[0xA0] = Basic(instructions::RegisterAL::ByteOp(al::And, b::B).into());
        instruction_decoders[0xA1] = Basic(instructions::RegisterAL::ByteOp(al::And, b::C).into());
        instruction_decoders[0xA2] = Basic(instructions::RegisterAL::ByteOp(al::And, b::D).into());
        instruction_decoders[0xA3] = Basic(instructions::RegisterAL::ByteOp(al::And, b::E).into());
        instruction_decoders[0xA4] = Basic(instructions::RegisterAL::ByteOp(al::And, b::H).into());
        instruction_decoders[0xA5] = Basic(instructions::RegisterAL::ByteOp(al::And, b::L).into());
        instruction_decoders[0xA6] = Basic(Instruction::MemoryAL(al::And));
        instruction_decoders[0xA7] = Basic(instructions::RegisterAL::ByteOp(al::And, b::A).into());

        instruction_decoders[0xA8] = Basic(instructions::RegisterAL::ByteOp(al::Xor, b::B).into());
        instruction_decoders[0xA9] = Basic(instructions::RegisterAL::ByteOp(al::Xor, b::C).into());
        instruction_decoders[0xAA] = Basic(instructions::RegisterAL::ByteOp(al::Xor, b::D).into());
        instruction_decoders[0xAB] = Basic(instructions::RegisterAL::ByteOp(al::Xor, b::E).into());
        instruction_decoders[0xAC] = Basic(instructions::RegisterAL::ByteOp(al::Xor, b::H).into());
        instruction_decoders[0xAD] = Basic(instructions::RegisterAL::ByteOp(al::Xor, b::L).into());
        instruction_decoders[0xAE] = Basic(Instruction::MemoryAL(al::Xor));
        instruction_decoders[0xAF] = Basic(instructions::RegisterAL::ByteOp(al::Xor, b::A).into());

        // Bx
        instruction_decoders[0xB0] = Basic(instructions::RegisterAL::ByteOp(al::Or, b::B).into());
        instruction_decoders[0xB1] = Basic(instructions::RegisterAL::ByteOp(al::Or, b::C).into());
        instruction_decoders[0xB2] = Basic(instructions::RegisterAL::ByteOp(al::Or, b::D).into());
        instruction_decoders[0xB3] = Basic(instructions::RegisterAL::ByteOp(al::Or, b::E).into());
        instruction_decoders[0xB4] = Basic(instructions::RegisterAL::ByteOp(al::Or, b::H).into());
        instruction_decoders[0xB5] = Basic(instructions::RegisterAL::ByteOp(al::Or, b::L).into());
        instruction_decoders[0xB6] = Basic(Instruction::MemoryAL(al::Or));
        instruction_decoders[0xB7] = Basic(instructions::RegisterAL::ByteOp(al::Or, b::A).into());

        instruction_decoders[0xB8] =
            Basic(instructions::RegisterAL::ByteOp(al::Compare, b::B).into());
        instruction_decoders[0xB9] =
            Basic(instructions::RegisterAL::ByteOp(al::Compare, b::C).into());
        instruction_decoders[0xBA] =
            Basic(instructions::RegisterAL::ByteOp(al::Compare, b::D).into());
        instruction_decoders[0xBB] =
            Basic(instructions::RegisterAL::ByteOp(al::Compare, b::E).into());
        instruction_decoders[0xBC] =
            Basic(instructions::RegisterAL::ByteOp(al::Compare, b::H).into());
        instruction_decoders[0xBD] =
            Basic(instructions::RegisterAL::ByteOp(al::Compare, b::L).into());
        instruction_decoders[0xBE] = Basic(Instruction::MemoryAL(al::Compare));
        instruction_decoders[0xBF] =
            Basic(instructions::RegisterAL::ByteOp(al::Compare, b::A).into());

        // Cx
        instruction_decoders[0xC0] = Basic(instructions::Jump::ReturnIf(cond::NonZero).into());
        instruction_decoders[0xC1] = Basic(instructions::Stack::Pop(aw::BC).into());
        instruction_decoders[0xC2] = ThreeByteAddress(Box::new(idecoders::ConditionalJump));
        instruction_decoders[0xC3] = ThreeByteAddress(Box::new(idecoders::Jump));
        instruction_decoders[0xC4] = ThreeByteAddress(Box::new(idecoders::ConditionalCall));
        instruction_decoders[0xC5] = Basic(instructions::Stack::Push(aw::BC).into());
        instruction_decoders[0xC6] = TwoByteData(Box::new(idecoders::ConstantAL));
        instruction_decoders[0xC7] = Basic(instructions::Jump::CallSystem(0x00.into()).into());

        instruction_decoders[0xC8] = Basic(instructions::Jump::ReturnIf(cond::Zero).into());
        instruction_decoders[0xC9] = Basic(instructions::Jump::Return.into());
        instruction_decoders[0xCA] = ThreeByteAddress(Box::new(idecoders::ConditionalJump));
        instruction_decoders[0xCB] = TwoByteData(Box::new(idecoders::Extended));
        instruction_decoders[0xCC] = ThreeByteAddress(Box::new(idecoders::ConditionalCall));
        instruction_decoders[0xCD] = ThreeByteAddress(Box::new(idecoders::Call));
        instruction_decoders[0xCE] = TwoByteData(Box::new(idecoders::ConstantAL));
        instruction_decoders[0xCF] = Basic(instructions::Jump::CallSystem(0x08.into()).into());

        // Dx
        instruction_decoders[0xD0] = Basic(instructions::Jump::ReturnIf(cond::NoCarry).into());
        instruction_decoders[0xD1] = Basic(instructions::Stack::Pop(aw::DE).into());
        instruction_decoders[0xD2] = ThreeByteAddress(Box::new(idecoders::ConditionalJump));
        instruction_decoders[0xD3] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xD4] = ThreeByteAddress(Box::new(idecoders::ConditionalCall));
        instruction_decoders[0xD5] = Basic(instructions::Stack::Push(aw::DE).into());
        instruction_decoders[0xD6] = TwoByteData(Box::new(idecoders::ConstantAL));
        instruction_decoders[0xD7] = Basic(instructions::Jump::CallSystem(0x10.into()).into());

        instruction_decoders[0xD8] = Basic(instructions::Jump::ReturnIf(cond::Carry).into());
        instruction_decoders[0xD9] = Basic(instructions::Jump::ReturnInterrupt.into());
        instruction_decoders[0xDA] = ThreeByteAddress(Box::new(idecoders::ConditionalJump));
        instruction_decoders[0xDB] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xDC] = ThreeByteAddress(Box::new(idecoders::ConditionalCall));
        instruction_decoders[0xDD] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xDE] = TwoByteData(Box::new(idecoders::ConstantAL));
        instruction_decoders[0xDF] = Basic(instructions::Jump::CallSystem(0x18.into()).into());

        // Ex
        instruction_decoders[0xE0] = TwoByteAddress(Box::new(idecoders::HighOffsetA));
        instruction_decoders[0xE1] = Basic(instructions::Stack::Pop(aw::HL).into());
        instruction_decoders[0xE2] = Basic(instructions::Load::MemoryOffsetA.into());
        instruction_decoders[0xE3] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xE4] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xE5] = Basic(instructions::Stack::Push(aw::HL).into());
        instruction_decoders[0xE6] = TwoByteData(Box::new(idecoders::ConstantAL));
        instruction_decoders[0xE7] = Basic(instructions::Jump::CallSystem(0x20.into()).into());

        instruction_decoders[0xE8] = TwoByteOffset(Box::new(idecoders::AddToStack));
        instruction_decoders[0xE9] = Basic(instructions::Jump::RegisterJump.into());
        instruction_decoders[0xEA] = ThreeByteAddress(Box::new(idecoders::StoreAddress));
        instruction_decoders[0xEB] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xEC] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xED] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xEE] = TwoByteData(Box::new(idecoders::ConstantAL));
        instruction_decoders[0xEF] = Basic(instructions::Jump::CallSystem(0x28.into()).into());

        // Fx
        instruction_decoders[0xF0] = TwoByteAddress(Box::new(idecoders::AHighOffset));
        instruction_decoders[0xF1] = Basic(instructions::Stack::Pop(aw::AF).into());
        instruction_decoders[0xF2] = Basic(instructions::Load::AMemoryOffset.into());
        instruction_decoders[0xF3] = Basic(Instruction::DisableInterrupts);
        instruction_decoders[0xF4] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xF5] = Basic(instructions::Stack::Push(aw::HL).into());
        instruction_decoders[0xF6] = TwoByteData(Box::new(idecoders::ConstantAL));
        instruction_decoders[0xF7] = Basic(instructions::Jump::CallSystem(0x30.into()).into());

        instruction_decoders[0xF8] = TwoByteOffset(Box::new(idecoders::CalcStackOffset));
        instruction_decoders[0xF9] = Basic(instructions::Stack::SetStackPointer.into());
        instruction_decoders[0xFA] = ThreeByteAddress(Box::new(idecoders::LoadAddress));
        instruction_decoders[0xFB] = Basic(Instruction::EnableInterrupts);
        instruction_decoders[0xFC] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xFD] = OneByte(Box::new(idecoders::Literal));
        instruction_decoders[0xFE] = TwoByteData(Box::new(idecoders::ConstantAL));
        instruction_decoders[0xFF] = Basic(instructions::Jump::CallSystem(0x38.into()).into());

        Decoder {
            instruction_decoders,
        }
    }

    /// Decodes a given instruction.
    ///
    /// The opcode should be the first byte of the instruction, while `iter`
    /// should be an iterator from the following byte, which will be used if needed.
    ///
    /// This returns an internal representation of the instruction useful for emulation.
    pub fn decode(
        &self,
        opcode: u8,
        iter: &mut dyn Iterator<Item = u8>,
    ) -> DecodeResult<Instruction> {
        let idecoder = self
            .instruction_decoders
            .get(opcode as usize)
            .ok_or(DecodeError::UnknownOpcode(opcode))?;
        idecoder.decode(opcode, iter)
    }

    #[cfg(feature = "disassembler")]
    /// Disassembles a given instruction.
    ///
    /// The opcode should be the first byte of the instruction, while `iter`
    /// should be an iterator from the following byte, which will be used if needed.
    ///
    /// This returns an representation of the instruction useful for display.
    pub fn disassemble(
        &self,
        opcode: u8,
        iter: &mut dyn Iterator<Item = u8>,
    ) -> DecodeResult<DisassembledInstruction> {
        let idecoder = self
            .instruction_decoders
            .get(opcode as usize)
            .ok_or(DecodeError::UnknownOpcode(opcode))?;
        idecoder.disassemble(opcode, iter)
    }
}

/// Decode an entire ROM or memory space in a single method.
///
/// This returns an internal representation of the instructions useful for emulation.
///
/// If you require more granular decoding, create a [`Decoder`](struct.Decoder.html) and
/// use [`Decoder.decode`](struct.Decoder.html#method.decode)
pub fn decode(data: &[u8]) -> DecodeResult<Vec<instructions::Instruction>> {
    let decoder = Decoder::new();
    let mut output = Vec::new();
    let mut iter = data.iter().copied();
    while let Some(byte) = iter.next() {
        let instruction = decoder.decode(byte, &mut iter)?;
        output.push(instruction);
    }
    Ok(output)
}

/// Disassemble an entire ROM or memory space in a single method.
///
/// This returns an representation of the instruction useful for display.
///
/// If you require more granular decoding, create a [`Decoder`](struct.Decoder.html) and
/// use [`Decoder.disassemble`](struct.Decoder.html#method.disassemble)
pub fn disassemble(data: &[u8]) -> DecodeResult<Vec<DisassembledInstruction>> {
    let decoder = Decoder::new();
    let mut output = Vec::new();
    let mut iter = data.iter().copied();
    while let Some(byte) = iter.next() {
        let instruction = decoder.disassemble(byte, &mut iter)?;
        output.push(instruction);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    pub fn test_decode_basic() {
        use registers::StackRegister as sw;
        let data = [0x00, 0x23];

        let decoded = decode(&data);

        let expected = vec![
            Instruction::NOP,
            instructions::RegisterAL::Increment16(sw::HL).into(),
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_one_byte() {
        let data = [0xE4, 0xD3, 0xE3, 0xE4];

        let decoded = decode(&data);

        let expected = vec![
            Instruction::Literal(0xE4),
            Instruction::Literal(0xD3),
            Instruction::Literal(0xE3),
            Instruction::Literal(0xE4),
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_two_byte_offset() {
        let data = [0x18, 0x20, 0x00];

        let decoded = decode(&data);

        let expected = vec![
            instructions::Jump::RelativeJump(0x20.into()).into(),
            Instruction::NOP,
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_two_byte_address() {
        let data = [0xF0, 0x12, 0x00];

        let decoded = decode(&data);

        let expected = vec![
            instructions::Load::AHighOffset(0x12.into()).into(),
            Instruction::NOP,
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_two_byte_data() {
        let data = [0x10, 0x00, 0x00];

        let decoded = decode(&data);

        let expected = vec![Instruction::Stop, Instruction::NOP];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_three_byte_address() {
        let data = [0x08, 0x20, 0x10, 0x00];

        let decoded = decode(&data);

        let expected = vec![
            instructions::Stack::StoreStackPointerMemory(0x1020.into()).into(),
            Instruction::NOP,
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_three_byte_data() {
        let data = [0x11, 0x13, 0x25, 0x00];

        let decoded = decode(&data);

        let expected = vec![
            instructions::Load::Constant16(registers::StackRegister::DE, 0x2513).into(),
            Instruction::NOP,
        ];
        assert_eq!(decoded, Ok(expected));
    }
}

#[cfg(all(feature = "disassembler", test))]
mod disassember_tests {
    use super::*;
    use alloc::vec;
    use DisassembledInstruction as dis;

    #[test]
    pub fn test_decode_basic() {
        let data = [0x00, 0x23];

        let decoded = disassemble(&data);

        let expected = vec![
            dis::OneByte(0x00, "NOP".into()),
            dis::OneByte(0x23, "INC HL".into()),
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_one_byte() {
        let data = [0xE4, 0xD3, 0xE3, 0xE4];

        let decoded = disassemble(&data);

        let expected = vec![
            dis::OneByte(0xE4, "DAT E4h".into()),
            dis::OneByte(0xD3, "DAT D3h".into()),
            dis::OneByte(0xE3, "DAT E3h".into()),
            dis::OneByte(0xE4, "DAT E4h".into()),
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_two_byte_offset() {
        let data = [0x18, 0x20, 0x00];

        let decoded = disassemble(&data);

        let expected = vec![
            dis::TwoByte(0x1820, "JR PC+20h".into()),
            dis::OneByte(0x00, "NOP".into()),
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_two_byte_address() {
        let data = [0xF0, 0x12, 0x00];

        let decoded = disassemble(&data);

        let expected = vec![
            dis::TwoByte(0xF012, "LD A, $FF12h".into()),
            dis::OneByte(0x00, "NOP".into()),
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_two_byte_data() {
        let data = [0x10, 0x00, 0x00];

        let decoded = disassemble(&data);

        let expected = vec![
            dis::TwoByte(0x1000, "STOP 0".into()),
            dis::OneByte(0x00, "NOP".into()),
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_three_byte_address() {
        let data = [0x08, 0x20, 0x10, 0x00];

        let decoded = disassemble(&data);

        let expected = vec![
            dis::ThreeByte(0x08_2010, "LD $1020h, SP".into()),
            dis::OneByte(0x00, "NOP".into()),
        ];
        assert_eq!(decoded, Ok(expected));
    }

    #[test]
    pub fn test_decode_three_byte_data() {
        let data = [0x11, 0x13, 0x25, 0x00];

        let decoded = disassemble(&data);

        let expected = vec![
            dis::ThreeByte(0x11_1325, "LD DE, 2513h".into()),
            dis::OneByte(0x00, "NOP".into()),
        ];
        assert_eq!(decoded, Ok(expected));
    }
}
