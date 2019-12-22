#![cfg_attr(not(feature = "std"), no_std)]
//! This crate represents the shared logic of `olympia` across
//! all frontends.
//!
//! The best modules to start looking in are the [`gameboy`] module which contains
//! the emulation core, and [`rom`] which contains the logic for parsing ROMs
//! and handling gameboy cartridge memory controllers.
//!
//! By default, it is `no_std` compatible, and has the following optional features:
//!
//! * `std` - This feature can be enabled in a `std` environment to enable niceties
//!   like `Display`/`Error` implementations on error types.
//! * `disassembler` - This feature can be enabled in any environment to enable support
//!   for dissambling gameboy instructions.
//!
//! [`gameboy`]: gameboy/index.html
//! [`rom`]: rom/index.html

#[macro_use]
extern crate alloc;

pub mod decoder;
pub mod gameboy;
mod instructions;
pub mod registers;
pub mod rom;
pub mod types;

#[cfg(feature = "disassembler")]
pub mod disassembler;
