use crate::address;

use crate::instructions::{ALOp, ByteRegisterOffset, Condition};
use crate::registers;

use alloc::format;
use alloc::string::{String, ToString};

pub trait Disassemble: alloc::fmt::Debug {
    fn disassemble(&self) -> String {
        format!("{:?}", self)
    }
}

impl Disassemble for ALOp {
    fn disassemble(&self) -> String {
        match self {
            ALOp::Add => "ADD",
            ALOp::AddCarry => "ADC",
            ALOp::Sub => "SUB",
            ALOp::SubCarry => "SBC",
            ALOp::And => "AND",
            ALOp::Xor => "XOR",
            ALOp::Or => "OR",
            ALOp::Compare => "CP",
        }
        .to_string()
    }
}

impl Disassemble for Condition {
    fn disassemble(&self) -> String {
        match self {
            Condition::NonZero => "NZ",
            Condition::Zero => "Z",
            Condition::NoCarry => "NC",
            Condition::Carry => "C",
        }
        .to_string()
    }
}

impl Disassemble for address::LiteralAddress {
    fn disassemble(&self) -> String {
        let address::LiteralAddress(raw_addr) = self;
        format!("${:X}h", raw_addr)
    }
}

impl Disassemble for address::HighAddress {
    fn disassemble(&self) -> String {
        let address::HighAddress(raw_addr) = self;
        let value = i8::from_le_bytes([*raw_addr]);
        let addr = if value > 0 {
            let val_u16 = value as u16;
            0xFF00u16.wrapping_add(val_u16)
        } else {
            let val_u16 = i16::from(value).abs() as u16;
            0xFF00u16 - val_u16
        };
        format!("${:X}h", addr)
    }
}

impl Disassemble for address::AddressOffset {
    fn disassemble(&self) -> String {
        if self.0 < 0 {
            format!("-{:X}h", self.0.abs())
        } else {
            format!("{:X}h", self.0.abs())
        }
    }
}

impl Disassemble for registers::WordRegister {}
impl Disassemble for registers::StackRegister {}
impl Disassemble for registers::AccRegister {}
impl Disassemble for registers::ByteRegister {}

impl Disassemble for registers::ByteRegisterTarget {
    fn disassemble(&self) -> String {
        match self {
            registers::ByteRegisterTarget::HLIndirect => "(HL)".into(),
            _ => format!("{:?}", self),
        }
    }
}

impl Disassemble for ByteRegisterOffset {
    fn disassemble(&self) -> String {
        format!("({:?})", self.0)
    }
}

impl Disassemble for u8 {
    fn disassemble(&self) -> String {
        format!("{:X?}h", self)
    }
}
impl Disassemble for i8 {
    fn disassemble(&self) -> String {
        if *self < 0 {
            format!("-{:X}h", self.abs())
        } else {
            format!("{:X}h", self)
        }
    }
}

impl Disassemble for u16 {
    fn disassemble(&self) -> String {
        format!("{:X?}h", self)
    }
}
