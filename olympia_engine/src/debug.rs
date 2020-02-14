//! Methods useful for implementing debugging functionality

use crate::address;
use crate::gameboy;
use crate::registers;

use alloc::string::String;
use core::convert::TryFrom;
use core::str::FromStr;
use derive_more::{Display, From};

/// Parse a user provided number
///
/// Values beginning with 0x or ending with h (such as 0x123 or 35h)
/// are attempted to be parsed as base 16. All others are attempted to be
/// parsed as base 10
pub fn parse_number(src: &str) -> Result<u16, core::num::ParseIntError> {
    let lowered = src.to_lowercase();
    if lowered.starts_with("0x") {
        u16::from_str_radix(&src[2..], 16)
    } else if lowered.ends_with('h') {
        u16::from_str_radix(&src[..src.len() - 1], 16)
    } else {
        src.parse()
    }
}

#[derive(Debug, From, Clone, Copy, Display)]
/// Types of value that can be read or written
pub enum RWTarget {
    /// Byte at the given memory location
    #[display(fmt = "memory location {}", "_0")]
    Address(address::LiteralAddress),
    /// Byte in the given 8-bit register
    #[display(fmt = "register {:?}", "_0")]
    ByteRegister(registers::ByteRegister),
    /// Word in the given 16-bit register
    #[display(fmt = "register {:?}", "_0")]
    WordRegister(registers::WordRegister),
    #[display(fmt = "cycles")]
    Cycles,
    #[display(fmt = "time")]
    Time,
}

#[derive(Debug, From, Display)]
pub enum ReadError {
    #[display(fmt = "Could not read from the address at {}", "_0")]
    Memory(address::LiteralAddress),
}

#[cfg(feature = "std")]
impl std::error::Error for ReadError {}

#[derive(Debug, From, Display)]
pub enum WriteError {
    #[display(fmt = "Could not write to the address at {}", "_0")]
    Memory(address::LiteralAddress),
    #[display(fmt = "The value {:X} is too large for the destination", "_0")]
    ValueTooLarge(u16),
    #[display(fmt = "Cannot update locations of this type")]
    Immutable,
}

#[cfg(feature = "std")]
impl std::error::Error for WriteError {}

impl RWTarget {
    /// Reads the value at the given target
    pub fn read(self, gb: &gameboy::GameBoy) -> Result<u64, ReadError> {
        match self {
            RWTarget::Address(addr) => gb
                .read_memory_u8(addr)
                .map(u64::from)
                .map_err(|_| addr.into()),
            RWTarget::ByteRegister(reg) => Ok(u64::from(gb.read_register_u8(reg))),
            RWTarget::WordRegister(reg) => Ok(u64::from(gb.read_register_u16(reg))),
            RWTarget::Cycles => Ok(gb.cycles_elapsed()),
            RWTarget::Time => Ok(gb.cycles_elapsed() / (1024 * 1024)),
        }
    }
    /// Writes the value at the given target
    ///
    /// Returns the previous value if successful
    pub fn write(self, gb: &mut gameboy::GameBoy, val: u16) -> Result<u64, WriteError> {
        let current_value = self.read(gb);
        match self {
            RWTarget::Address(addr) => {
                let value = u8::try_from(val).map_err(|_| WriteError::ValueTooLarge(val))?;
                gb.write_memory_u8(addr, value)
                    .map_err(|_| WriteError::Memory(addr))?;
            }
            RWTarget::ByteRegister(reg) => {
                let value = u8::try_from(val).map_err(|_| WriteError::ValueTooLarge(val))?;
                gb.write_register_u8(reg, value);
            }
            RWTarget::WordRegister(reg) => gb.write_register_u16(reg, val),
            RWTarget::Cycles | RWTarget::Time => return Err(WriteError::Immutable),
        }
        Ok(current_value.unwrap())
    }
}

/// Indicates a value could not be parsed as a Read/Write target
#[derive(Debug, Display)]
#[display(fmt = "{} is not a valid register or memory location", _0)]
pub struct TargetParseError(String);

impl FromStr for RWTarget {
    type Err = TargetParseError;

    fn from_str(s: &str) -> Result<RWTarget, TargetParseError> {
        if s == "cycles" {
            return Ok(RWTarget::Cycles);
        } else if s == "time" {
            return Ok(RWTarget::Time);
        }
        parse_number(s)
            .map(|val| address::LiteralAddress(val).into())
            .map_err(|_| ())
            .or_else(|_| {
                s.to_uppercase()
                    .parse::<registers::WordRegister>()
                    .map(|wr| wr.into())
                    .map_err(|_| ())
            })
            .or_else(|_| {
                s.to_uppercase()
                    .parse::<registers::ByteRegister>()
                    .map(|br| br.into())
                    .map_err(|_| ())
            })
            .map_err(|_| TargetParseError(s.into()))
    }
}

#[derive(Debug, Display, Clone)]
/// A breakpoint that triggers when a monitored value is set to a given value.
#[display(fmt = "Breakpoint: {} == {:X}", monitor, value)]
pub struct Breakpoint {
    /// The value that should be checked
    pub monitor: RWTarget,
    /// Value to check against. For 8-bit registers or memory locations, only
    /// the lower 8-bits are checked
    pub value: u64,
}

impl Breakpoint {
    pub fn new(monitor: RWTarget, value: u64) -> Breakpoint {
        Breakpoint { monitor, value }
    }

    /// Returns whether this breakpoint is active
    pub fn should_break(&self, gb: &gameboy::GameBoy) -> bool {
        let read_result = self.monitor.read(gb);
        read_result.is_ok() && read_result.unwrap() == self.value
    }
}
