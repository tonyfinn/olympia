use crate::address;
use crate::registers;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
/// Gameboy events that frontends might be interested in
pub enum Event {
    /// A write occured to a memory mapped location
    MemoryWrite {
        address: address::LiteralAddress,
        /// Value written to that location
        value: u8,
        /// The actual new value after the write
        new_value: u8,
    },
    /// A write occured to a named register
    RegisterWrite {
        reg: registers::WordRegister,
        value: u16,
    },
    /// The PPU reached its hblank cycle
    HBlank,
    /// The PPU reached its vblank cycle
    VBlank,
}

pub type EventHandler = dyn Fn(Event) -> ();
