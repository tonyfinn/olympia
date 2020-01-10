//! This crate represents instruction components
//! that can be applied to any instruction.
//! It does not contain instructions themselves,
//! these are included in `olympia_engine`

use crate::address;
use crate::registers;

use alloc::vec::Vec;
use core::convert::From;

#[derive(Debug, PartialEq, Eq)]
/// Represents a value that failed to parse
pub struct ParseError(u8);

/// Details to embed/extract a param into an opcode.
///
/// align the value to a byte,
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct OpcodePosition {
    /// mask is the bitmask to extract only those bytes that belong to this value.
    pub mask: u8,
    /// shift should represent how much of a right shift is needed to
    pub shift: u8,
}

/// A value that can be embedded in an opcode
pub trait EmbeddableParam: Sized {
    /// Extracts the value from a given opcode. The value should be aligned to a byte for this method
    fn extract(value: u8) -> Result<Self, ParseError>;
    /// Returns a value to be embedded in an opcode.
    fn embed(&self) -> u8;

    /// Extracts a value from the given opcode.
    fn extract_from_opcode(opcode: u8, pos: OpcodePosition) -> Result<Self, ParseError> {
        <Self as EmbeddableParam>::extract((opcode & pos.mask) >> pos.shift)
    }

    /// Embeds a value in the given opcode
    fn embed_to_opcode(&self, opcode: u8, pos: OpcodePosition) -> u8 {
        let rest_of_opcode = opcode & !pos.mask;
        let embed_value = self.embed() << pos.shift;
        rest_of_opcode | embed_value
    }
}

impl EmbeddableParam for u8 {
    fn extract(value: u8) -> Result<u8, ParseError> {
        Ok(value)
    }

    fn embed(&self) -> u8 {
        *self
    }
}

pub trait AppendableParam {
    fn as_bytes(&self) -> Vec<u8>;
}

impl AppendableParam for u8 {
    fn as_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().iter().copied().collect()
    }
}

impl AppendableParam for u16 {
    fn as_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().iter().copied().collect()
    }
}

impl AppendableParam for i8 {
    fn as_bytes(&self) -> Vec<u8> {
        self.to_le_bytes().iter().copied().collect()
    }
}

impl AppendableParam for address::LiteralAddress {
    fn as_bytes(&self) -> Vec<u8> {
        u16::from(*self).as_bytes()
    }
}

impl AppendableParam for address::AddressOffset {
    fn as_bytes(&self) -> Vec<u8> {
        u8::from(*self).as_bytes()
    }
}

impl AppendableParam for address::HighAddress {
    fn as_bytes(&self) -> Vec<u8> {
        u8::from(*self).as_bytes()
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Checks for conditional instructions
pub enum Condition {
    /// The Zero flag is not set
    NonZero,
    /// The Zero flag is set
    Zero,
    /// The Carry flag is not set
    NoCarry,
    /// The Carry flag is set.
    Carry,
}

impl EmbeddableParam for Condition {
    fn extract(value: u8) -> Result<Condition, ParseError> {
        match value {
            0b00 => Ok(Condition::NonZero),
            0b01 => Ok(Condition::Zero),
            0b10 => Ok(Condition::NoCarry),
            0b11 => Ok(Condition::Carry),
            _ => Err(ParseError(value)),
        }
    }

    fn embed(&self) -> u8 {
        match self {
            Condition::NonZero => 0b00,
            Condition::Zero => 0b01,
            Condition::NoCarry => 0b10,
            Condition::Carry => 0b11,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Whether to include the Carry bit in operations.
/// This does not affect setting the carry bit, only
/// reading it.
pub enum Carry {
    Carry,
    NoCarry,
}

impl EmbeddableParam for Carry {
    fn extract(value: u8) -> Result<Carry, ParseError> {
        match value {
            0 => Ok(Carry::Carry),
            1 => Ok(Carry::NoCarry),
            _ => Err(ParseError(value)),
        }
    }

    fn embed(&self) -> u8 {
        match self {
            Carry::Carry => 0,
            Carry::NoCarry => 1,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Direction to rotate/shift operand
pub enum RotateDirection {
    Left,
    Right,
}

impl EmbeddableParam for RotateDirection {
    fn extract(value: u8) -> Result<RotateDirection, ParseError> {
        match value {
            0 => Ok(RotateDirection::Left),
            1 => Ok(RotateDirection::Right),
            _ => Err(ParseError(value)),
        }
    }

    fn embed(&self) -> u8 {
        match self {
            RotateDirection::Left => 0,
            RotateDirection::Right => 1,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Whether an instruction should increment or decrement
/// its operand.
pub enum Increment {
    Increment,
    Decrement,
}

impl EmbeddableParam for Increment {
    fn extract(value: u8) -> Result<Increment, ParseError> {
        match value {
            0 => Ok(Increment::Increment),
            1 => Ok(Increment::Decrement),
            _ => Err(ParseError(value)),
        }
    }

    fn embed(&self) -> u8 {
        match self {
            Increment::Increment => 0,
            Increment::Decrement => 1,
        }
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// All supported ALU operations
pub enum ALOp {
    Add,
    AddCarry,
    Sub,
    SubCarry,
    And,
    Xor,
    Or,
    Compare,
}

impl EmbeddableParam for ALOp {
    fn extract(value: u8) -> Result<ALOp, ParseError> {
        match value {
            0b000 => Ok(ALOp::Add),
            0b001 => Ok(ALOp::AddCarry),
            0b010 => Ok(ALOp::Sub),
            0b011 => Ok(ALOp::SubCarry),
            0b100 => Ok(ALOp::And),
            0b101 => Ok(ALOp::Xor),
            0b110 => Ok(ALOp::Or),
            0b111 => Ok(ALOp::Compare),
            _ => Err(ParseError(value)),
        }
    }

    fn embed(&self) -> u8 {
        match self {
            ALOp::Add => 0b000,
            ALOp::AddCarry => 0b001,
            ALOp::Sub => 0b010,
            ALOp::SubCarry => 0b011,
            ALOp::And => 0b100,
            ALOp::Xor => 0b101,
            ALOp::Or => 0b110,
            ALOp::Compare => 0b111,
        }
    }
}

/// A operand that can either target a 8-bit
/// register (excluding F) or the memory address
/// referenced by the HL register.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum ByteRegisterTarget {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
    HLIndirect,
}

impl EmbeddableParam for ByteRegisterTarget {
    fn extract(value: u8) -> Result<ByteRegisterTarget, ParseError> {
        match value {
            0b000 => Ok(ByteRegisterTarget::B),
            0b001 => Ok(ByteRegisterTarget::C),
            0b010 => Ok(ByteRegisterTarget::D),
            0b011 => Ok(ByteRegisterTarget::E),
            0b100 => Ok(ByteRegisterTarget::H),
            0b101 => Ok(ByteRegisterTarget::L),
            0b110 => Ok(ByteRegisterTarget::HLIndirect),
            0b111 => Ok(ByteRegisterTarget::A),
            _ => Err(ParseError(value)),
        }
    }

    fn embed(&self) -> u8 {
        match self {
            ByteRegisterTarget::B => 0b000,
            ByteRegisterTarget::C => 0b001,
            ByteRegisterTarget::D => 0b010,
            ByteRegisterTarget::E => 0b011,
            ByteRegisterTarget::H => 0b100,
            ByteRegisterTarget::L => 0b101,
            ByteRegisterTarget::HLIndirect => 0b110,
            ByteRegisterTarget::A => 0b111,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
/// 16bit Register group that includes the accumalator
/// register.
///
/// This is mainly used for operatiions that target the
/// stack as it gives extra flexibility for targeting the
/// stack. Note that writing to the F register in this manner
/// only sets the high nibble of the F register.
pub enum AccRegister {
    BC,
    DE,
    HL,
    AF,
}

impl EmbeddableParam for AccRegister {
    fn extract(value: u8) -> Result<AccRegister, ParseError> {
        match value {
            0b00 => Ok(AccRegister::BC),
            0b01 => Ok(AccRegister::DE),
            0b10 => Ok(AccRegister::HL),
            0b11 => Ok(AccRegister::AF),
            _ => Err(ParseError(value)),
        }
    }

    fn embed(&self) -> u8 {
        match self {
            AccRegister::BC => 0b00,
            AccRegister::DE => 0b01,
            AccRegister::HL => 0b10,
            AccRegister::AF => 0b11,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
/// Registers group that includes the stack register
///
/// This is mainly used for operations that do not operate on the stack,
/// such as 16-bit transfers. This is because
/// the stack target is implicit in operations that do
/// operate on the stack
pub enum StackRegister {
    BC,
    DE,
    HL,
    SP,
}

impl EmbeddableParam for StackRegister {
    fn extract(value: u8) -> Result<StackRegister, ParseError> {
        match value {
            0b00 => Ok(StackRegister::BC),
            0b01 => Ok(StackRegister::DE),
            0b10 => Ok(StackRegister::HL),
            0b11 => Ok(StackRegister::SP),
            _ => Err(ParseError(value)),
        }
    }

    fn embed(&self) -> u8 {
        match self {
            StackRegister::BC => 0b00,
            StackRegister::DE => 0b01,
            StackRegister::HL => 0b10,
            StackRegister::SP => 0b11,
        }
    }
}

/// All types of operand allowed to be embedded
/// in an opcode
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum InnerParam {
    ALOp,
    Increment,
    RotateDirection,
    Carry,
    Condition,
    ByteRegisterTarget,
    AccRegister,
    StackRegister,
    Literal8,
}

/// All types of param that can be expected in
/// subsequent bytes to the opcode
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum AppendedParam {
    LiteralAddress,
    HighAddress,
    AddressOffset,
    Literal16,
    Literal8,
    LiteralSigned8,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct ByteRegisterOffset(registers::ByteRegister);

impl From<ByteRegisterOffset> for registers::ByteRegister {
    fn from(offset: ByteRegisterOffset) -> registers::ByteRegister {
        offset.0
    }
}

impl From<registers::ByteRegister> for ByteRegisterOffset {
    fn from(reg: registers::ByteRegister) -> ByteRegisterOffset {
        ByteRegisterOffset(reg)
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Represents a param that has a constant value
pub enum ConstantParam {
    ByteRegister(registers::ByteRegister),
    WordRegister(registers::WordRegister),
    ByteRegisterOffset(registers::ByteRegister),
    LiteralAddress(address::LiteralAddress),
}

/// Represents a parameter for a given opcode
///
/// Parameters can either be:
///
/// 1. embedded in the opcode, in which case they require a mask to identify
/// which bytes belong to the opcode, such as in `MV reg, reg`
///
/// 2. Appended after the opcode in subsequent bytes such as in `ADD A, d8`
///
/// 3. Implied by the instruction opcode, but not actually present in it, such
///    as in `JP HL`
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum ParamType {
    Appended(AppendedParam),
    Inner { pos: OpcodePosition, ty: InnerParam },
    Constant(ConstantParam),
}

/// Defines whether an opcode is part of the standard
/// table or the extended table.
///
/// The extended opcodes are represented by the sequence
/// CB xx where xx is the opcode, while non extended opcodes
/// are represented simply as xx where xx is the opcode.
#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum ExtensionType {
    None,
    Extended,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Position that a param occupies in an instruction
pub enum ParamPosition {
    Dest,
    Src,
    Single,
    AddSrc,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Instruction parameter definition
pub struct ParamDefinition {
    pub param_type: ParamType,
    pub pos: ParamPosition,
}

#[derive(PartialEq, Eq, Debug, Clone)]
pub struct InstructionDefinition {
    pub opcodes: &'static [u8],
    pub label: &'static str,
    pub extension_type: ExtensionType,
    pub params: &'static [ParamDefinition],
}

/// An instruction with embedded opcode parameters
/// included, but not those parameters that are in
/// subsequent instructions
pub trait InstructionOpcode {
    type FullInstruction: Instruction;
    fn definition() -> &'static InstructionDefinition
    where
        Self: Sized;
    fn from_opcode(opcode: u8) -> Self
    where
        Self: Sized;
    fn build_instruction(&self, data: &mut dyn Iterator<Item = u8>) -> Self::FullInstruction;
}

pub trait Instruction {
    fn definition() -> &'static InstructionDefinition
    where
        Self: Sized;
    fn as_bytes(&self) -> Vec<u8>;
}

#[cfg(test)]
mod test {
    use super::*;
    use alloc::vec;

    #[test]
    fn embed_u8() {
        assert_eq!(EmbeddableParam::embed(&0x23), 0x23);
    }

    #[test]
    fn extract_condition() {
        assert_eq!(Condition::extract(0), Ok(Condition::NonZero));
        assert_eq!(Condition::extract(1), Ok(Condition::Zero));
        assert_eq!(Condition::extract(2), Ok(Condition::NoCarry));
        assert_eq!(Condition::extract(3), Ok(Condition::Carry));
        assert_eq!(Condition::extract(4), Err(ParseError(4)));
    }

    #[test]
    fn embed_condition() {
        assert_eq!(Condition::NonZero.embed(), 0);
        assert_eq!(Condition::Zero.embed(), 1);
        assert_eq!(Condition::NoCarry.embed(), 2);
        assert_eq!(Condition::Carry.embed(), 3);
    }

    #[test]
    fn extract_carry() {
        assert_eq!(Carry::extract(0), Ok(Carry::Carry));
        assert_eq!(Carry::extract(1), Ok(Carry::NoCarry));
        assert_eq!(Carry::extract(4), Err(ParseError(4)));
    }

    #[test]
    fn embed_carry() {
        assert_eq!(Carry::Carry.embed(), 0);
        assert_eq!(Carry::NoCarry.embed(), 1);
    }

    #[test]
    fn extract_rotate_direction() {
        assert_eq!(RotateDirection::extract(0), Ok(RotateDirection::Left));
        assert_eq!(RotateDirection::extract(1), Ok(RotateDirection::Right));
        assert_eq!(RotateDirection::extract(4), Err(ParseError(4)));
    }

    #[test]
    fn embed_rotate_direction() {
        assert_eq!(RotateDirection::Left.embed(), 0);
        assert_eq!(RotateDirection::Right.embed(), 1);
    }

    #[test]
    fn extract_increment() {
        assert_eq!(Increment::extract(0), Ok(Increment::Increment));
        assert_eq!(Increment::extract(1), Ok(Increment::Decrement));
        assert_eq!(Increment::extract(4), Err(ParseError(4)));
    }

    #[test]
    fn embed_increment() {
        assert_eq!(Increment::Increment.embed(), 0);
        assert_eq!(Increment::Decrement.embed(), 1);
    }

    #[test]
    fn extract_alop() {
        assert_eq!(ALOp::extract(0), Ok(ALOp::Add));
        assert_eq!(ALOp::extract(1), Ok(ALOp::AddCarry));
        assert_eq!(ALOp::extract(2), Ok(ALOp::Sub));
        assert_eq!(ALOp::extract(3), Ok(ALOp::SubCarry));
        assert_eq!(ALOp::extract(4), Ok(ALOp::And));
        assert_eq!(ALOp::extract(5), Ok(ALOp::Xor));
        assert_eq!(ALOp::extract(6), Ok(ALOp::Or));
        assert_eq!(ALOp::extract(7), Ok(ALOp::Compare));
        assert_eq!(ALOp::extract(9), Err(ParseError(9)));
    }

    #[test]
    fn embed_alop() {
        assert_eq!(ALOp::Add.embed(), 0);
        assert_eq!(ALOp::AddCarry.embed(), 1);
        assert_eq!(ALOp::Sub.embed(), 2);
        assert_eq!(ALOp::SubCarry.embed(), 3);
        assert_eq!(ALOp::And.embed(), 4);
        assert_eq!(ALOp::Xor.embed(), 5);
        assert_eq!(ALOp::Or.embed(), 6);
        assert_eq!(ALOp::Compare.embed(), 7);
    }

    #[test]
    fn extract_byte_register_lookup() {
        assert_eq!(ByteRegisterTarget::extract(0), Ok(ByteRegisterTarget::B));
        assert_eq!(ByteRegisterTarget::extract(1), Ok(ByteRegisterTarget::C));
        assert_eq!(ByteRegisterTarget::extract(2), Ok(ByteRegisterTarget::D));
        assert_eq!(ByteRegisterTarget::extract(3), Ok(ByteRegisterTarget::E));
        assert_eq!(ByteRegisterTarget::extract(4), Ok(ByteRegisterTarget::H));
        assert_eq!(ByteRegisterTarget::extract(5), Ok(ByteRegisterTarget::L));
        assert_eq!(
            ByteRegisterTarget::extract(6),
            Ok(ByteRegisterTarget::HLIndirect)
        );
        assert_eq!(ByteRegisterTarget::extract(7), Ok(ByteRegisterTarget::A));
        assert_eq!(ByteRegisterTarget::extract(9), Err(ParseError(9)));
    }

    #[test]
    fn embed_byte_register_lookup() {
        assert_eq!(ByteRegisterTarget::B.embed(), 0);
        assert_eq!(ByteRegisterTarget::C.embed(), 1);
        assert_eq!(ByteRegisterTarget::D.embed(), 2);
        assert_eq!(ByteRegisterTarget::E.embed(), 3);
        assert_eq!(ByteRegisterTarget::H.embed(), 4);
        assert_eq!(ByteRegisterTarget::L.embed(), 5);
        assert_eq!(ByteRegisterTarget::HLIndirect.embed(), 6);
        assert_eq!(ByteRegisterTarget::A.embed(), 7);
    }

    #[test]
    fn extract_acc_register() {
        assert_eq!(AccRegister::extract(0), Ok(AccRegister::BC));
        assert_eq!(AccRegister::extract(1), Ok(AccRegister::DE));
        assert_eq!(AccRegister::extract(2), Ok(AccRegister::HL));
        assert_eq!(AccRegister::extract(3), Ok(AccRegister::AF));
        assert_eq!(AccRegister::extract(9), Err(ParseError(9)));
    }

    #[test]
    fn embed_acc_register() {
        assert_eq!(AccRegister::BC.embed(), 0);
        assert_eq!(AccRegister::DE.embed(), 1);
        assert_eq!(AccRegister::HL.embed(), 2);
        assert_eq!(AccRegister::AF.embed(), 3);
    }

    #[test]
    fn extract_stack_register() {
        assert_eq!(StackRegister::extract(0), Ok(StackRegister::BC));
        assert_eq!(StackRegister::extract(1), Ok(StackRegister::DE));
        assert_eq!(StackRegister::extract(2), Ok(StackRegister::HL));
        assert_eq!(StackRegister::extract(3), Ok(StackRegister::SP));
        assert_eq!(StackRegister::extract(9), Err(ParseError(9)));
    }

    #[test]
    fn embed_stack_register() {
        assert_eq!(StackRegister::BC.embed(), 0);
        assert_eq!(StackRegister::DE.embed(), 1);
        assert_eq!(StackRegister::HL.embed(), 2);
        assert_eq!(StackRegister::SP.embed(), 3);
    }

    #[test]
    fn embed_to_opcode() {
        assert_eq!(
            AccRegister::DE.embed_to_opcode(
                0x30,
                OpcodePosition {
                    mask: 0x0C,
                    shift: 2
                }
            ),
            0x34
        );
    }

    #[test]
    fn append_i8() {
        assert_eq!(AppendableParam::as_bytes(&-1i8), vec![0xFF]);
    }

    #[test]
    fn append_u8() {
        assert_eq!(AppendableParam::as_bytes(&0x33u8), vec![0x33]);
    }

    #[test]
    fn append_u16() {
        assert_eq!(AppendableParam::as_bytes(&0x2333u16), vec![0x33, 0x23]);
    }

    #[test]
    fn append_literal_address() {
        assert_eq!(AppendableParam::as_bytes(&address::LiteralAddress(0x2333)), vec![0x33, 0x23]);
    }

    #[test]
    fn append_high_address() {
        assert_eq!(AppendableParam::as_bytes(&address::HighAddress(0x23)), vec![0x23]);
    }

    #[test]
    fn append_address_offset() {
        assert_eq!(AppendableParam::as_bytes(&address::AddressOffset(-16i8)), vec![0xF0]);
    }
}
