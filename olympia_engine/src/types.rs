//! Represents a variety of fundamental types for emulation.

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Represents a literal memory address
pub struct LiteralAddress(pub u16);

impl LiteralAddress {
    /// Get the address immediately following this one, wrapping if needed
    pub fn next(self) -> LiteralAddress {
        let LiteralAddress(addr) = self;
        LiteralAddress(addr.wrapping_add(1))
    }
}

impl From<u16> for LiteralAddress {
    fn from(addr: u16) -> Self {
        LiteralAddress(addr)
    }
}

impl Into<u16> for LiteralAddress {
    fn into(self) -> u16 {
        let LiteralAddress(addr) = self;
        addr
    }
}

impl From<[u8; 2]> for LiteralAddress {
    fn from(bytes: [u8; 2]) -> Self {
        LiteralAddress(u16::from_le_bytes(bytes))
    }
}

impl From<HighAddress> for LiteralAddress {
    fn from(addr: HighAddress) -> Self {
        let HighAddress(offset) = addr;
        LiteralAddress(u16::from(offset) + 0xff00)
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Represents an address in high memory (offset from 0xFF00)
pub struct HighAddress(pub u8);

impl From<u8> for HighAddress {
    fn from(addr: u8) -> Self {
        HighAddress(addr)
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Represents an address that is offset from the program counter
pub struct AddressOffset(pub i8);

impl From<u8> for AddressOffset {
    fn from(addr: u8) -> Self {
        AddressOffset(i8::from_le_bytes([addr]))
    }
}

impl AddressOffset {
    /// Returns new address, half carry and carry flags
    pub(crate) fn resolve(self, pc: LiteralAddress) -> (LiteralAddress, bool, bool) {
        use std::convert::TryFrom;
        let LiteralAddress(raw_pc) = pc;
        let AddressOffset(offset) = self;
        let (new_addr, half_carry, carry) = if offset < 0 {
            let to_sub = u16::try_from(offset.abs()).unwrap();
            let (new, carry) = raw_pc.overflowing_sub(to_sub);
            let half_carry = (new & 0x10) != to_sub & 0x10;
            (new, half_carry, carry)
        } else {
            let to_add = u16::try_from(offset.abs()).unwrap();
            let half_add = ((raw_pc & 0xF) + (to_add & 0xF)) & 0xF0;
            let (new, carry) = raw_pc.overflowing_add(to_add);
            (new, half_add != 0, carry)
        };
        (LiteralAddress(new_addr), half_carry, carry)
    }
}
