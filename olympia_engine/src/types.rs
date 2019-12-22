#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct LiteralAddress(pub u16);

impl LiteralAddress {
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
pub struct HighAddress(pub u8);

impl From<u8> for HighAddress {
    fn from(addr: u8) -> Self {
        HighAddress(addr)
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub struct PCOffset(pub i8);

impl From<u8> for PCOffset {
    fn from(addr: u8) -> Self {
        PCOffset(i8::from_le_bytes([addr]))
    }
}
