#![no_std]
//! olympia_core provides definitions of fundamental types for
//! olympia that are required by both olympia_core and
//! olympia_derive.

extern crate alloc;

pub mod address;
pub mod derive;
pub mod disasm;
pub mod instructions;
pub mod registers;
