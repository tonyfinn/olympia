#[derive(PartialEq, Eq, Debug)]
#[derive(Copy, Clone)]
pub struct MemoryAddress(pub u16);

impl From<u16> for MemoryAddress {
    fn from(addr: u16) -> Self {
        MemoryAddress(addr)
    }
}

#[derive(PartialEq, Eq, Debug)]
#[derive(Copy, Clone)]
pub struct HighAddress(pub u8);

impl From<u8> for HighAddress {
    fn from(addr: u8) -> Self {
        HighAddress(addr)
    }
}


#[derive(PartialEq, Eq, Debug)]
#[derive(Copy, Clone)]
pub struct PCOffset(pub i8);

impl From<u8> for PCOffset {
    fn from(addr: u8) -> Self {
        PCOffset(i8::from_le_bytes([addr]))
    }
}