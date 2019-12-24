//! Contains operations on CPU registers

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

/// Represents a CPU flag set after some instructions.
///
/// Note that many instructions leave flags alone,
/// and others may repurpose them for side channel information.
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
