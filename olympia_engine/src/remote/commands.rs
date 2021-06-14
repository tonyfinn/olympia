use derive_more::{Display, From, TryInto};

#[cfg(feature = "std")]
use derive_more::Error;

use alloc::{string::String, vec::Vec};

use crate::{
    gameboy::StepError, gbdebug::Breakpoint, registers::WordRegister, remote::Event,
    rom::CartridgeLoadError,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UiBreakpoint {
    pub active: bool,
    pub breakpoint: Breakpoint,
}

impl From<Breakpoint> for UiBreakpoint {
    fn from(breakpoint: Breakpoint) -> UiBreakpoint {
        UiBreakpoint {
            active: true,
            breakpoint,
        }
    }
}

/// The running/not running state of the remote emulator
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ExecMode {
    /// The emulator is not running as it has not yet loaded
    Unloaded,
    /// The emulator is not running as it paused
    Paused,
    /// The emulator is not running as has hit a breakpoint
    HitBreakpoint(UiBreakpoint),
    /// The emulator is running at actual gameboy speed
    Standard,
    /// The emulator is running as fast as possible
    Uncapped,
}

#[derive(PartialEq, Eq, From, Display, Debug)]
#[cfg_attr(feature = "std", derive(Error))]
/// A failure to load a ROM
pub enum LoadRomError {
    #[display(fmt = "Could not parse ROM: {}", "_0")]
    InvalidRom(CartridgeLoadError),
    #[display(fmt = "Could not load ROM: {}", "_0")]
    #[from(ignore)]
    #[cfg_attr(feature = "std", error(ignore))]
    Io(String),
}

#[derive(Debug, Display, From, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Error))]
/// A problem encountered by a remote emulator
pub enum Error {
    #[display(fmt = "Error during emulation: {}", "_0")]
    Exec(StepError),
    #[display(fmt = "Failed loading ROM: {}", "_0")]
    Load(LoadRomError),
    #[display(fmt = "Action cannot be performed without a ROM loaded")]
    NoRomLoaded,
}

/// Result of a remote emulator operation
pub type Result<T> = core::result::Result<T, Error>;

/// The values of all 16-bit registers
#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub struct QueryRegistersResponse {
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub sp: u16,
    pub pc: u16,
}

impl QueryRegistersResponse {
    pub fn read_u16(&self, register: WordRegister) -> u16 {
        match register {
            WordRegister::AF => self.af,
            WordRegister::BC => self.bc,
            WordRegister::DE => self.de,
            WordRegister::HL => self.hl,
            WordRegister::SP => self.sp,
            WordRegister::PC => self.pc,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
/// The memory data at a requested address
pub struct QueryMemoryResponse {
    /// The first address in memory represented by the data
    pub start_addr: u16,
    /// The data in memory at that space.
    ///
    /// None indicates memory that should not be available
    pub data: Vec<Option<u8>>,
}

#[derive(Debug, Clone)]
/// A single command for the remote emulator execute
pub enum EmulatorCommand {
    /// Load a rom from a given file path
    LoadRom(Vec<u8>),
    /// Query all registers
    QueryRegisters,
    /// Query memory from the start address (inclusive)
    /// to end address (inclusive)
    QueryMemory(u16, u16),
    /// Run a single step
    Step,
    /// Find out how much time has elapsed in the emulation core
    QueryExecTime,
    /// Set the exec mode - paused, 1x speed or fast forward
    SetMode(ExecMode),
    /// Add a breakpoint
    AddBreakpoint(UiBreakpoint),
}

#[derive(Debug, PartialEq, PartialOrd, From)]
/// The time the emulator has been running
pub struct ExecTime(f64);

impl ExecTime {
    /// Get a duration object for the execution time
    pub fn duration(&self) -> core::time::Duration {
        core::time::Duration::from_secs_f64(self.0)
    }
}

#[derive(Debug, From, TryInto, PartialEq)]
/// A response to an emulator command
pub enum EmulatorResponse {
    LoadRom(core::result::Result<(), LoadRomError>),
    QueryRegisters(Result<QueryRegistersResponse>),
    QueryMemory(Result<QueryMemoryResponse>),
    Step(Result<()>),
    QueryExecTime(Result<ExecTime>),
    SetMode(core::result::Result<ExecMode, ()>),
    AddBreakpoint(core::result::Result<(), ()>),
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
/// An identifier for a running command
pub struct CommandId(pub u64);

#[derive(From, TryInto)]
/// Events, Errors and Responses from a remote emulator
pub enum RemoteEmulatorOutput {
    Event(Event),
    Error(Error),
    Response(CommandId, EmulatorResponse),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_register_response_lookup() {
        let response = QueryRegistersResponse {
            af: 0x1234,
            bc: 0x2345,
            de: 0x3456,
            hl: 0x4567,
            pc: 0x5678,
            sp: 0x6789,
        };

        assert_eq!(response.read_u16(WordRegister::AF), 0x1234);
        assert_eq!(response.read_u16(WordRegister::BC), 0x2345);
        assert_eq!(response.read_u16(WordRegister::DE), 0x3456);
        assert_eq!(response.read_u16(WordRegister::HL), 0x4567);
        assert_eq!(response.read_u16(WordRegister::PC), 0x5678);
        assert_eq!(response.read_u16(WordRegister::SP), 0x6789);
    }

    #[test]
    fn load_rom_error_equality() {
        let invalid_rom_a1 = LoadRomError::InvalidRom(CartridgeLoadError::UnsupportedRamSize(0x80));
        let invalid_rom_a2 = LoadRomError::InvalidRom(CartridgeLoadError::UnsupportedRamSize(0x80));
        let invalid_rom_b =
            LoadRomError::InvalidRom(CartridgeLoadError::UnsupportedCartridgeType(0x12));

        let io_error_a1 = LoadRomError::Io("Interrupted: Foo".into());
        let io_error_a2 = LoadRomError::Io("Interrupted: Foo".into());
        let io_error_b = LoadRomError::Io("404 foo not found".into());

        assert_eq!(invalid_rom_a1, invalid_rom_a2);
        assert_eq!(io_error_a1, io_error_a2);

        assert_ne!(invalid_rom_a1, invalid_rom_b);
        assert_ne!(io_error_a2, io_error_b);
        assert_ne!(invalid_rom_a1, io_error_a1);
    }
}
