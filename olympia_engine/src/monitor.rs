//! Methods useful for implementing debugging functionality

use crate::address;
use crate::events::{Event, MemoryEvent, RegisterWriteEvent};
use crate::gameboy;
use crate::registers;

use alloc::string::String;
use alloc::vec::Vec;
use core::convert::TryFrom;
use core::str::FromStr;
use derive_more::{Display, From};

/// Parse a user provided number
///
/// The following prefixes are recognised:
/// 0x (hex), 0b (binary), 0o (octal)
/// The following suffixes are recognised:
/// h (hex), b (binary)
///
/// All others are attempted to be
/// parsed as base 10
pub fn parse_number(src: &str) -> Result<u16, core::num::ParseIntError> {
    let lowered = src.to_lowercase();
    if lowered.starts_with("0x") {
        u16::from_str_radix(&src[2..], 16)
    } else if lowered.starts_with("0b") {
        u16::from_str_radix(&src[2..], 2)
    } else if lowered.starts_with("0o") {
        u16::from_str_radix(&src[2..], 8)
    } else if lowered.ends_with('h') {
        u16::from_str_radix(&src[..src.len() - 1], 16)
    } else if lowered.ends_with('b') {
        u16::from_str_radix(&src[..src.len() - 1], 2)
    } else {
        src.parse()
    }
}

#[derive(Debug, From, Clone, Copy, Display, PartialEq, Eq)]
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

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    #[display(fmt = ">")]
    GreaterThan,
    #[display(fmt = ">=")]
    GreaterThanEqual,
    #[display(fmt = "<")]
    LessThan,
    #[display(fmt = "<=")]
    LessThanEqual,
    #[display(fmt = "==")]
    Equal,
    #[display(fmt = "!=")]
    NotEqual,
}

impl Comparison {
    pub fn test(&self, value_to_test: u64, reference_value: u64) -> bool {
        match self {
            Comparison::GreaterThan => value_to_test > reference_value,
            Comparison::GreaterThanEqual => value_to_test >= reference_value,
            Comparison::LessThan => value_to_test < reference_value,
            Comparison::LessThanEqual => value_to_test <= reference_value,
            Comparison::Equal => value_to_test == reference_value,
            Comparison::NotEqual => value_to_test != reference_value,
        }
    }
}

impl FromStr for Comparison {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            ">" => Ok(Comparison::GreaterThan),
            ">=" => Ok(Comparison::GreaterThanEqual),
            "<" => Ok(Comparison::LessThan),
            "<=" => Ok(Comparison::LessThanEqual),
            "=" => Ok(Comparison::Equal),
            "==" => Ok(Comparison::Equal),
            "!=" => Ok(Comparison::NotEqual),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Display, Clone, Copy, PartialEq, Eq)]
pub enum BreakpointCondition {
    #[display(fmt = "{} {:X}", "_0", "_1")]
    Test(Comparison, u64),
    #[display(fmt = "Read")]
    Read,
    #[display(fmt = "Write")]
    Write,
}

#[derive(Debug, Display, Clone, PartialEq, Eq)]
/// A breakpoint that triggers when a monitored value is set to a given value.
#[display(fmt = "Breakpoint: {} {}", monitor, condition)]
pub struct Breakpoint {
    /// The value that should be checked
    pub monitor: RWTarget,
    /// Value to check against. For 8-bit registers or memory locations, only
    /// the lower 8-bits are checked
    pub condition: BreakpointCondition,
    /// Whether the breakpoint should be considered
    pub active: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct BreakpointIdentifier(u32);

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BreakpointState {
    Inactive,
    HitBreakpoint(Breakpoint),
}

impl Breakpoint {
    /// New breakpoint with the specified condition
    pub fn new(monitor: RWTarget, condition: BreakpointCondition) -> Breakpoint {
        Breakpoint {
            monitor,
            condition,
            active: true,
        }
    }

    /// Returns whether this breakpoint is active
    pub fn should_break(&self, gb: &gameboy::GameBoy) -> bool {
        let read_result = self.monitor.read(gb);
        use BreakpointCondition::*;
        if let Ok(value) = read_result {
            match self.condition {
                Test(cmp, reference_value) => cmp.test(value, reference_value),
                Read => false,
                Write => false,
            }
        } else {
            false
        }
    }
}

#[derive(Debug)]
pub struct DebugMonitor {
    breakpoints: Vec<(BreakpointIdentifier, Breakpoint)>,
    state: BreakpointState,
    next_identifier: u32,
}

impl DebugMonitor {
    pub fn new() -> DebugMonitor {
        DebugMonitor {
            breakpoints: Vec::new(),
            state: BreakpointState::Inactive,
            next_identifier: 0,
        }
    }

    pub fn resume(&mut self) {
        self.state = BreakpointState::Inactive
    }

    pub fn state(&self) -> BreakpointState {
        self.state.clone()
    }

    pub fn handle_event(&mut self, event: &Event) -> bool {
        match event {
            Event::Memory(MemoryEvent::Read { address, .. }) => self.handle_read((*address).into()),
            Event::Memory(MemoryEvent::Write {
                address, new_value, ..
            }) => self.handle_write((*address).into(), (*new_value).into()),
            Event::RegisterWrite(RegisterWriteEvent { reg, value }) => {
                self.handle_write((*reg).into(), (*value).into())
            }
            _ => false,
        }
    }

    pub fn add_breakpoint(&mut self, bp: Breakpoint) -> BreakpointIdentifier {
        let identifier = BreakpointIdentifier(self.next_identifier);
        self.breakpoints.push((identifier, bp));
        self.next_identifier += 1;
        identifier
    }

    pub fn remove_breakpoint(&mut self, id_to_remove: BreakpointIdentifier) -> Option<Breakpoint> {
        let idx = self
            .breakpoints
            .iter()
            .position(|(id, _)| *id == id_to_remove)?;

        let (_, bp) = self.breakpoints.remove(idx);
        Some(bp)
    }

    pub fn set_breakpoint_state(
        &mut self,
        id_to_update: BreakpointIdentifier,
        state: bool,
    ) -> Option<bool> {
        let breakpoint = self
            .breakpoints
            .iter_mut()
            .find(|(id, _)| id_to_update == *id);

        if let Some((_, bp)) = breakpoint {
            bp.active = state;
            Some(state)
        } else {
            log::warn!(
                "Tried to set state of invalid breakpoint {:?}",
                id_to_update
            );
            None
        }
    }

    fn handle_read(&mut self, target: RWTarget) -> bool {
        for (_id, bp) in self.breakpoints.iter() {
            if !bp.active {
                continue;
            }
            if bp.condition == BreakpointCondition::Read && target == bp.monitor {
                self.state = BreakpointState::HitBreakpoint(bp.clone());
                return true;
            }
        }
        false
    }

    fn handle_write(&mut self, target: RWTarget, value: u64) -> bool {
        for (_id, bp) in self.breakpoints.iter() {
            if !bp.active {
                continue;
            }
            if bp.condition == BreakpointCondition::Write && target == bp.monitor {
                self.state = BreakpointState::HitBreakpoint(bp.clone());
                return true;
            } else if let BreakpointCondition::Test(cmp, reference_value) = bp.condition {
                if target == bp.monitor && cmp.test(value, reference_value) {
                    log::info!("Broke on bp {} {} {}", value, cmp, reference_value);
                    self.state = BreakpointState::HitBreakpoint(bp.clone());
                    return true;
                }
            }
        }
        false
    }
}
