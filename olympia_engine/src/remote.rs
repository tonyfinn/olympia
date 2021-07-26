//! Handle communicating with an emulator over a remote channel
//!
//! This is useful for running the emulator in for example another
//! thread or a web worker.
//!
//! ## Usage
//!
//! A front end should provide its own implementation of the [`RemoteEmulatorChannel`]
//! and [`RemoteEventListeners`] traits that uses the front end environments own mechanisms
//! to communicate, and then create an instance of [`RemoteEmulator`].
//!
//! The front end can then use methods on [`RemoteEmulator`] to control the emulator
//!
//! [`RemoteEmulatorChannel`]: ./trait.RemoteEmulatorChannel.html
//! [`RemoteEventListeners`]: ./trait.RemoteEventListeners.html
//! [`RemoteEmulator`]: ./struct.RemoteEmulator.html

mod commands;
mod events;
mod remote_emulator;

pub use commands::{
    CommandId, EmulatorCommand, EmulatorResponse, Error, ExecMode, ExecTime, LoadRomError,
    QueryMemoryResponse, QueryRegistersResponse, RemoteEmulatorOutput, Result,
    ToggleBreakpointResponse,
};

pub use events::{AdapterEventWrapper, Event, EventSendError, RemoteEventListeners, Sender};

pub use remote_emulator::{EmulatorCommandExecution, RemoteEmulator, RemoteEmulatorChannel};
