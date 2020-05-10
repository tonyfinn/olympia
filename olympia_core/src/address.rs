//! Represents a variety of addressing types for
//! emulation.

use derive_more::{Display, From, FromStr, Into};

#[derive(PartialEq, Eq, Debug, Copy, Clone, From, FromStr, Into, Display)]
/// Represents a literal memory address
#[display(fmt = "[{}]", _0)]
pub struct LiteralAddress(pub u16);

impl LiteralAddress {
    /// Get the address immediately following this one, wrapping if needed
    pub fn next(self) -> LiteralAddress {
        LiteralAddress(self.0.wrapping_add(1))
    }
}

impl From<[u8; 2]> for LiteralAddress {
    fn from(bytes: [u8; 2]) -> Self {
        LiteralAddress(u16::from_le_bytes(bytes))
    }
}

impl From<HighAddress> for LiteralAddress {
    fn from(addr: HighAddress) -> Self {
        LiteralAddress(u16::from(addr.0) + 0xff00)
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone, From, Into)]
/// Represents an address in high memory (offset from 0xFF00)
pub struct HighAddress(pub u8);

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
#[doc(hidden)]
pub struct OffsetResolveResult {
    pub addr: LiteralAddress,
    pub half_carry: bool,
    pub carry: bool,
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
/// Represents an address that is offset from the program counter
pub struct AddressOffset(pub i8);

impl From<u8> for AddressOffset {
    fn from(addr: u8) -> Self {
        AddressOffset(i8::from_le_bytes([addr]))
    }
}

impl From<AddressOffset> for u8 {
    fn from(addr: AddressOffset) -> Self {
        addr.0 as u8
    }
}

impl AddressOffset {

    /// Add offset to a given base to find a new address
    ///
    /// base is the address to offset from, which in the
    /// gameboy instruction set is based on the PC or SP
    /// register, depending on the instruction
    pub fn resolve(self, base: LiteralAddress) -> LiteralAddress {
        self.resolve_internal(base).addr
    }

    /// Returns new address, half carry and carry flags
    ///
    /// base is the address to offset from, which in the
    /// gameboy instruction set is based on the PC or SP
    /// register, depending on the instruction
    #[doc(hidden)]
    pub fn resolve_internal(self, base: LiteralAddress) -> OffsetResolveResult {
        use core::convert::TryFrom;
        let raw_base = base.0;
        let offset = self.0;
        let (new_addr, half_carry, carry) = if offset < 0 {
            let to_sub = u16::try_from(offset.abs()).unwrap();
            let (new, carry) = raw_base.overflowing_sub(to_sub);
            let half_carry = ((raw_base & 0xF) + 0x10) - (to_sub & 0xF) < 0x10;
            (new, half_carry, carry)
        } else {
            let to_add = u16::try_from(offset.abs()).unwrap();
            let half_add = ((raw_base & 0xF) + (to_add & 0xF)) & 0xF0;
            let (new, carry) = raw_base.overflowing_add(to_add);
            (new, half_add != 0, carry)
        };
        OffsetResolveResult {
            addr: new_addr.into(),
            half_carry,
            carry,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_convert_bytes_to_address() {
        assert_eq!(LiteralAddress::from([0x54, 0x32]), LiteralAddress(0x3254));
    }

    #[test]
    fn test_resolve_address_postive_offset() {
        let positive_offset = AddressOffset(0x2C);

        assert_eq!(
            positive_offset.resolve_internal(0x1000.into()),
            OffsetResolveResult {
                addr: 0x102C.into(),
                carry: false,
                half_carry: false,
            }
        );

        assert_eq!(
            positive_offset.resolve(0x1000.into()),
            LiteralAddress(0x102C),
        );

        assert_eq!(
            positive_offset.resolve_internal(0x1004.into()),
            OffsetResolveResult {
                addr: 0x1030.into(),
                carry: false,
                half_carry: true,
            }
        );

        assert_eq!(
            positive_offset.resolve_internal(0xFFF0.into()),
            OffsetResolveResult {
                addr: 0x001C.into(),
                carry: true,
                half_carry: false,
            }
        );

        assert_eq!(
            positive_offset.resolve_internal(0xFFFF.into()),
            OffsetResolveResult {
                addr: 0x002B.into(),
                carry: true,
                half_carry: true,
            }
        );
    }

    #[test]
    fn test_resolve_address_negative_offset() {
        let positive_offset = AddressOffset(-0x19);

        assert_eq!(
            positive_offset.resolve_internal(0x102C.into()),
            OffsetResolveResult {
                addr: 0x1013.into(),
                carry: false,
                half_carry: false,
            }
        );

        assert_eq!(
            positive_offset.resolve_internal(0x1004.into()),
            OffsetResolveResult {
                addr: 0x0FEB.into(),
                carry: false,
                half_carry: true,
            }
        );

        assert_eq!(
            positive_offset.resolve_internal(0x000A.into()),
            OffsetResolveResult {
                addr: 0xFFF1.into(),
                carry: true,
                half_carry: false,
            }
        );

        assert_eq!(
            positive_offset.resolve_internal(0x0000.into()),
            OffsetResolveResult {
                addr: 0xFFE7.into(),
                carry: true,
                half_carry: true,
            }
        );
    }
}
