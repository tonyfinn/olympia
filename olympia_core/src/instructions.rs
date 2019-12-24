//! This crate represents instruction components
//! that can be applied to any instruction.
//! It does not contain instructions themselves,
//! these are included in `olympia_engine`

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

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Whether to include the Carry bit in operations.
/// This does not affect setting the carry bit, only
/// reading it.
pub enum Carry {
    Carry,
    NoCarry,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Direction to rotate/shift operand
pub enum RotateDirection {
    Left,
    Right,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Whether an instruction should increment or decrement
/// its operand.
pub enum Increment {
    Increment,
    Decrement,
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

/// A operand that can either target a 8-bit
/// register (excluding F) or the memory address
/// referenced by the HL register.
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

/// All types of operand allowed to be embedded
/// in an opcode
pub enum InnerInstructionParam {
    ALOp,
    Increment,
    RotateDirection,
    Carry,
    Condition,
    ByteRegister,
    WordRegister,
}

/// All types of param that can be expected in
/// subsequent bytes to the opcode
pub enum AppendedParam {
    LiteralAddress,
    HighAddress,
    AddressOffset,
    Literal16,
    Literal8,
}

/// Defines whether an opcode is part of the standard
/// table or the extended table.
///
/// The extended opcodes are represented by the sequence
/// CB xx where xx is the opcode, while non extended opcodes
/// are represented simply as xx where xx is the opcode.
pub enum ExtensionType {
    None,
    Extended,
}
