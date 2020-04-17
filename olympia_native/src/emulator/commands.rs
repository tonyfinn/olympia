use derive_more::{From, TryInto};
use olympia_engine::{
    debug::Breakpoint, events::Event as EngineEvent, gameboy::StepError, registers::WordRegister,
    rom::CartridgeError,
};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct UiBreakpoint {
    pub(crate) active: bool,
    pub(crate) breakpoint: Breakpoint,
}

impl From<Breakpoint> for UiBreakpoint {
    fn from(breakpoint: Breakpoint) -> UiBreakpoint {
        UiBreakpoint {
            active: true,
            breakpoint,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum ExecMode {
    Unloaded,
    Paused,
    HitBreakpoint(UiBreakpoint),
    Standard,
    Uncapped,
}

#[derive(Error, Debug)]
pub(crate) enum LoadRomError {
    #[error("Could not parse ROM: {0}")]
    InvalidRom(#[from] CartridgeError),
    #[error("Could not load ROM: {0}")]
    Unreadable(#[from] std::io::Error),
}

impl PartialEq for LoadRomError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LoadRomError::InvalidRom(cart_error), LoadRomError::InvalidRom(other_cart_error)) => {
                cart_error == other_cart_error
            }
            (LoadRomError::Unreadable(err), LoadRomError::Unreadable(other_err)) => {
                err.kind() == other_err.kind()
            }
            _ => false,
        }
    }
}

impl Eq for LoadRomError {}

#[derive(Error, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    #[error("Error during emulation: {0}")]
    Exec(#[from] StepError),
    #[error("Failed loading ROM: {0}")]
    Load(#[from] LoadRomError),
    #[error("Action cannot be performed without a ROM loaded")]
    NoRomLoaded,
}

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub(crate) struct QueryRegistersResponse {
    pub(crate) af: u16,
    pub(crate) bc: u16,
    pub(crate) de: u16,
    pub(crate) hl: u16,
    pub(crate) sp: u16,
    pub(crate) pc: u16,
}

impl QueryRegistersResponse {
    pub(crate) fn read_u16(&self, register: WordRegister) -> u16 {
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct QueryMemoryResponse {
    pub start_addr: u16,
    pub data: Vec<Option<u8>>,
}

#[derive(Debug, Clone)]
pub(crate) enum EmulatorCommand {
    /// Load a rom from a given file path
    LoadRom(PathBuf),
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
pub(crate) struct ExecTime(f64);

impl From<ExecTime> for std::time::Duration {
    fn from(et: ExecTime) -> std::time::Duration {
        std::time::Duration::from_secs_f64(et.0)
    }
}

#[derive(Debug, From, TryInto, PartialEq)]
pub(crate) enum EmulatorResponse {
    LoadRom(std::result::Result<(), LoadRomError>),
    QueryRegisters(Result<QueryRegistersResponse>),
    QueryMemory(Result<QueryMemoryResponse>),
    Step(Result<()>),
    QueryExecTime(Result<ExecTime>),
    SetMode(std::result::Result<ExecMode, ()>),
    AddBreakpoint(std::result::Result<(), ()>),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Hash)]
pub struct CommandId(pub(crate) u64);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Continue(pub bool);

impl From<Continue> for glib::Continue {
    fn from(c: Continue) -> glib::Continue {
        glib::Continue(c.0)
    }
}

#[derive(From, TryInto)]
pub(crate) enum EmulatorThreadOutput {
    Event(EngineEvent),
    Error(Error),
    Response(CommandId, EmulatorResponse),
    ModeChange(ExecMode),
}
