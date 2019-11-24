#![cfg_attr(not(feature = "std"), no_std)]

#[macro_use]
extern crate alloc;

pub mod gameboy;
pub mod decoder;
mod instructions;
mod registers;
pub mod rom;
mod types;

#[cfg(feature = "disassembler")]
mod disassembler;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
