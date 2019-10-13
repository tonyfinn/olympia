#![no_std]

#[macro_use]
extern crate alloc;

pub mod cpu;
pub mod decoder;
mod types;
mod instructions;
mod registers;

#[cfg(feature="disassembler")]
mod disassembler;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
