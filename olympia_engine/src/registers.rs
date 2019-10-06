#[derive(Debug, PartialEq, Eq, Copy, Clone)]
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
pub enum WordRegister {
    AF,
    BC,
    DE,
    HL,
    SP,
    PC,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub(crate) enum WordByte {
    High,
    Low,
}

impl ByteRegister {
    pub(crate) fn lookup_byte(&self) -> WordByte {
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

    pub(crate) fn lookup_word_register(&self) -> WordRegister {
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
}
