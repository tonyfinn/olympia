//! Contains operations on CPU registers

pub use crate::instructions::{AccRegister, ByteRegisterTarget, StackRegister};

use core::convert::TryFrom;

use alloc::string::String;

pub struct RegisterParseError(String);

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
/// All 8-bit registers
pub enum ByteRegister {
    A,
    F,
    B,
    C,
    D,
    E,
    H,
    L,
}

impl core::str::FromStr for ByteRegister {
    type Err = RegisterParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "A" => Ok(ByteRegister::A),
            "B" => Ok(ByteRegister::B),
            "C" => Ok(ByteRegister::C),
            "D" => Ok(ByteRegister::D),
            "E" => Ok(ByteRegister::E),
            "F" => Ok(ByteRegister::F),
            "H" => Ok(ByteRegister::H),
            "L" => Ok(ByteRegister::L),
            _ => Err(RegisterParseError(String::from(s))),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
/// All 16-bit registers
pub enum WordRegister {
    AF,
    BC,
    DE,
    HL,
    SP,
    PC,
}

impl WordRegister {
    pub fn all() -> [WordRegister; 6] {
        use WordRegister as wr;
        [wr::AF, wr::BC, wr::DE, wr::HL, wr::SP, wr::PC]
    }

    pub fn contains(&self, byte_reg: ByteRegister) -> bool {
        use ByteRegister as br;
        match self {
            WordRegister::AF => byte_reg == br::A || byte_reg == br::B,
            WordRegister::BC => byte_reg == br::B || byte_reg == br::C,
            WordRegister::DE => byte_reg == br::D || byte_reg == br::E,
            WordRegister::HL => byte_reg == br::H || byte_reg == br::L,
            _ => false,
        }
    }
}

impl core::str::FromStr for WordRegister {
    type Err = RegisterParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "AF" => Ok(WordRegister::AF),
            "BC" => Ok(WordRegister::BC),
            "DE" => Ok(WordRegister::DE),
            "HL" => Ok(WordRegister::HL),
            "SP" => Ok(WordRegister::SP),
            "PC" => Ok(WordRegister::PC),
            _ => Err(RegisterParseError(String::from(s))),
        }
    }
}

impl From<AccRegister> for WordRegister {
    fn from(reg: AccRegister) -> WordRegister {
        match reg {
            AccRegister::AF => WordRegister::AF,
            AccRegister::BC => WordRegister::BC,
            AccRegister::DE => WordRegister::DE,
            AccRegister::HL => WordRegister::HL,
        }
    }
}

impl From<StackRegister> for WordRegister {
    fn from(reg: StackRegister) -> WordRegister {
        match reg {
            StackRegister::SP => WordRegister::SP,
            StackRegister::BC => WordRegister::BC,
            StackRegister::DE => WordRegister::DE,
            StackRegister::HL => WordRegister::HL,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum WordByte {
    High,
    Low,
}

impl ByteRegister {
    /// Returns whether this 8-bit register is the high or low byte
    /// of its 16bit register
    pub fn lookup_byte(self) -> WordByte {
        match self {
            ByteRegister::A => WordByte::High,
            ByteRegister::F => WordByte::Low,
            ByteRegister::B => WordByte::High,
            ByteRegister::C => WordByte::Low,
            ByteRegister::D => WordByte::High,
            ByteRegister::E => WordByte::Low,
            ByteRegister::H => WordByte::High,
            ByteRegister::L => WordByte::Low,
        }
    }

    /// Returns which 16-bit register this 8-bit register
    /// is part of
    pub fn lookup_word_register(self) -> WordRegister {
        match self {
            ByteRegister::A => WordRegister::AF,
            ByteRegister::F => WordRegister::AF,
            ByteRegister::B => WordRegister::BC,
            ByteRegister::C => WordRegister::BC,
            ByteRegister::D => WordRegister::DE,
            ByteRegister::E => WordRegister::DE,
            ByteRegister::H => WordRegister::HL,
            ByteRegister::L => WordRegister::HL,
        }
    }
}

impl TryFrom<ByteRegisterTarget> for ByteRegister {
    type Error = ();

    fn try_from(lookup: ByteRegisterTarget) -> Result<ByteRegister, ()> {
        match lookup {
            ByteRegisterTarget::A => Ok(ByteRegister::A),
            ByteRegisterTarget::B => Ok(ByteRegister::B),
            ByteRegisterTarget::C => Ok(ByteRegister::C),
            ByteRegisterTarget::D => Ok(ByteRegister::D),
            ByteRegisterTarget::E => Ok(ByteRegister::E),
            ByteRegisterTarget::H => Ok(ByteRegister::H),
            ByteRegisterTarget::L => Ok(ByteRegister::L),
            ByteRegisterTarget::HLIndirect => Err(()),
        }
    }
}

/// Represents a CPU flag set after some instructions.
///
/// Note that many instructions leave flags alone,
/// and others may repurpose them for side channel information.
#[derive(Debug)]
pub enum Flag {
    /// The last arithmetic operation resulted in 0
    Zero,
    /// The last arithmetic operation was a subtract type operation
    /// 0 = Add, 1 = Sub
    AddSubtract,
    /// The last arithmetic operation contained a carry between nibbles
    HalfCarry,
    /// The last arithmetic operation overflowed or underflowed
    Carry,
}

impl Flag {
    /// Returns which bit of the flag register represents this flag
    pub fn bit(&self) -> u8 {
        match self {
            Flag::Zero => 7,
            Flag::AddSubtract => 6,
            Flag::HalfCarry => 5,
            Flag::Carry => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_register() {
        assert_eq!(ByteRegister::A.lookup_word_register(), WordRegister::AF);
        assert_eq!(ByteRegister::F.lookup_word_register(), WordRegister::AF);
        assert_eq!(ByteRegister::B.lookup_word_register(), WordRegister::BC);
        assert_eq!(ByteRegister::C.lookup_word_register(), WordRegister::BC);
        assert_eq!(ByteRegister::D.lookup_word_register(), WordRegister::DE);
        assert_eq!(ByteRegister::E.lookup_word_register(), WordRegister::DE);
        assert_eq!(ByteRegister::H.lookup_word_register(), WordRegister::HL);
        assert_eq!(ByteRegister::L.lookup_word_register(), WordRegister::HL);
    }

    #[test]
    fn test_flag_bit() {
        assert_eq!(Flag::Zero.bit(), 7);
        assert_eq!(Flag::AddSubtract.bit(), 6);
        assert_eq!(Flag::HalfCarry.bit(), 5);
        assert_eq!(Flag::Carry.bit(), 4);
    }
}
