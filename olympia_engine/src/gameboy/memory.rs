use crate::rom::Cartridge;
use crate::types;

pub(crate) const DMA_REGISTER_ADDR: u16 = 0xff46;
pub(crate) const INTERRUPT_ENABLE_ADDR: u16 = 0xffff;
pub(crate) const INTERRUPT_FLAG_ADDR: u16 = 0xff0f;

#[derive(PartialEq, Eq, Debug)]
pub struct MemoryRegion {
    pub start: u16,
    pub last: u16,
    pub len: u16,
    pub name: &'static str,
}

impl MemoryRegion {
    const fn new(start: u16, len: u16, name: &'static str) -> MemoryRegion {
        MemoryRegion {
            start,
            len,
            name,
            last: start + (len - 1),
        }
    }

    pub fn contains(&self, addr: u16) -> bool {
        addr >= self.start && addr <= self.last
    }
}

pub const STATIC_ROM: MemoryRegion = MemoryRegion::new(0x0000, 0x4000, "staticrom");
pub const SWITCHABLE_ROM: MemoryRegion = MemoryRegion::new(0x4000, 0x4000, "switchrom");
pub const CARTRIDGE_ROM: MemoryRegion = MemoryRegion::new(0x0000, 0x4000, "rom");
pub const VRAM: MemoryRegion = MemoryRegion::new(0x8000, 0x2000, "vram");
pub const CARTRIDGE_RAM: MemoryRegion = MemoryRegion::new(0xA000, 0x2000, "cartram");
pub const SYS_RAM: MemoryRegion = MemoryRegion::new(0xC000, 0x2000, "sysram");
pub const SYS_RAM_MIRROR: MemoryRegion = MemoryRegion::new(0xE000, 0x1E00, "sysram_mirror");
pub const OAM_RAM: MemoryRegion = MemoryRegion::new(0xFE00, 0xA0, "oamram");
pub const MEM_REGISTERS: MemoryRegion = MemoryRegion::new(0xFF00, 0x80, "memregisters");
pub const CPU_RAM: MemoryRegion = MemoryRegion::new(0xFF80, 0x7F, "cpuram");

#[derive(PartialEq, Eq, Debug, Clone)]
/// Represents a failure to read from memory.
pub enum MemoryError {
    /// The address maps to the Cartridge ROM area,
    /// but the currently loaded cartridge does not have
    /// ROM at this address. This can happen for MBC1/SROM cartridges
    /// that have less than 8KB of storage
    InvalidRomAddress(u16),
    /// The address maps to the Cartridge RAM area,
    /// but the currently loaded cartridge does not have
    /// RAM at this address. This can happen for cartridges
    /// that have < 2KB of RAM, including no RAM
    InvalidRamAddress(u16),
    /// The address maps to an area that is unmapped for the
    /// current gameboy model. This can include areas that are unmapped in
    /// all models, or registers that only exist on Game Boy Color
    UnmappedAddress(u16),
}

pub type MemoryResult<T> = Result<T, MemoryError>;

pub(crate) struct MemoryIterator<'a> {
    addr: types::LiteralAddress,
    mem: &'a Memory,
}

impl<'a> Iterator for MemoryIterator<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let val = self.mem.read_u8(self.addr);
        self.addr = self.addr.next();
        Some(val.unwrap_or(0))
    }
}

pub struct MemoryRegisters {
    pub(crate) dma: u8,
    pub(crate) iflag: u8,
    pub(crate) ie: u8,
}

impl MemoryRegisters {
    fn new() -> MemoryRegisters {
        MemoryRegisters {
            dma: 0,
            iflag: 0,
            ie: 0,
        }
    }

    fn read(&self, addr: u16) -> u8 {
        match addr {
            DMA_REGISTER_ADDR => self.dma,
            INTERRUPT_FLAG_ADDR => self.iflag,
            INTERRUPT_ENABLE_ADDR => self.ie,
            _ => 0,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            DMA_REGISTER_ADDR => self.dma = value,
            INTERRUPT_FLAG_ADDR => self.iflag = value & 0x1F,
            INTERRUPT_ENABLE_ADDR => self.ie = value & 0x1F,
            _ => (),
        }
    }
}

fn is_mem_register(addr: u16) -> bool {
    MEM_REGISTERS.contains(addr) || addr == 0xffff
}

pub struct Memory {
    cpuram: [u8; 127],
    oamram: [u8; 160],
    sysram: [u8; 0x2000],
    vram: [u8; 0x2000],
    cartridge: Cartridge,
    pub(crate) registers: MemoryRegisters,
}

impl Memory {
    pub fn new(cartridge: Cartridge) -> Memory {
        Memory {
            cpuram: [0u8; 127],
            oamram: [0u8; 160],
            sysram: [0u8; 0x2000],
            vram: [0u8; 0x2000],
            cartridge,
            registers: MemoryRegisters::new(),
        }
    }

    pub fn read_u8<A: Into<types::LiteralAddress>>(&self, target: A) -> MemoryResult<u8> {
        let types::LiteralAddress(addr) = target.into();
        if CARTRIDGE_ROM.contains(addr) {
            self.cartridge
                .read(addr)
                .map_err(|_| MemoryError::InvalidRomAddress(addr))
        } else if VRAM.contains(addr) {
            Ok(self.vram[(addr - VRAM.start) as usize])
        } else if CARTRIDGE_RAM.contains(addr) {
            self.cartridge
                .read(addr)
                .map_err(|_| MemoryError::InvalidRamAddress(addr))
        } else if SYS_RAM.contains(addr) {
            Ok(self.sysram[(addr - SYS_RAM.start) as usize])
        } else if SYS_RAM_MIRROR.contains(addr) {
            Ok(self.sysram[(addr - SYS_RAM_MIRROR.start) as usize])
        } else if OAM_RAM.contains(addr) {
            Ok(self.oamram[(addr - OAM_RAM.start) as usize])
        } else if CPU_RAM.contains(addr) {
            Ok(self.cpuram[(addr - CPU_RAM.start) as usize])
        } else if is_mem_register(addr) {
            Ok(self.registers.read(addr))
        } else {
            Err(MemoryError::UnmappedAddress(addr))
        }
    }

    pub fn write_u8<A: Into<types::LiteralAddress>>(
        &mut self,
        target: A,
        value: u8,
    ) -> MemoryResult<()> {
        let types::LiteralAddress(addr) = target.into();
        if CARTRIDGE_ROM.contains(addr) {
            self.cartridge
                .write(addr, value)
                .map_err(|_| MemoryError::InvalidRomAddress(addr))
        } else if VRAM.contains(addr) {
            self.vram[(addr - VRAM.start) as usize] = value;
            Ok(())
        } else if CARTRIDGE_RAM.contains(addr) {
            self.cartridge
                .write(addr, value)
                .map_err(|_| MemoryError::InvalidRamAddress(addr))
        } else if SYS_RAM.contains(addr) {
            self.sysram[(addr - SYS_RAM.start) as usize] = value;
            Ok(())
        } else if SYS_RAM_MIRROR.contains(addr) {
            self.sysram[(addr - SYS_RAM_MIRROR.start) as usize] = value;
            Ok(())
        } else if OAM_RAM.contains(addr) {
            self.oamram[(addr - OAM_RAM.start) as usize] = value;
            Ok(())
        } else if is_mem_register(addr) {
            self.registers.write(addr, value);
            Ok(())
        } else if CPU_RAM.contains(addr) {
            self.cpuram[(addr - CPU_RAM.start) as usize] = value;
            Ok(())
        } else {
            Err(MemoryError::UnmappedAddress(addr))
        }
    }

    pub(crate) fn offset_iter(&self, start: types::LiteralAddress) -> MemoryIterator {
        MemoryIterator {
            addr: start,
            mem: &self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_vram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(VRAM.start, 0xff).unwrap();
        assert_eq!(memory.vram[0], 0xff);
    }

    #[test]
    fn test_write_sysram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(SYS_RAM.start, 0xff).unwrap();
        assert_eq!(memory.sysram[0], 0xff);
    }

    #[test]
    fn test_write_sysram_mirror() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(SYS_RAM_MIRROR.start, 0xff).unwrap();
        assert_eq!(memory.sysram[0], 0xff);
    }

    #[test]
    fn test_write_oamram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(OAM_RAM.start, 0xff).unwrap();
        assert_eq!(memory.oamram[0], 0xff);
    }

    #[test]
    fn test_write_cpuram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(CPU_RAM.start, 0xff).unwrap();
        assert_eq!(memory.cpuram[0], 0xff);
    }

    #[test]
    fn test_read_vram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.vram[0] = 0xff;

        assert_eq!(memory.read_u8(VRAM.start).unwrap(), 0xff);
    }

    #[test]
    fn test_read_sysram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.sysram[0] = 0xff;

        assert_eq!(memory.read_u8(SYS_RAM.start).unwrap(), 0xff);
    }

    #[test]
    fn test_read_sysram_mirror() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.sysram[0] = 0xff;

        assert_eq!(memory.read_u8(SYS_RAM_MIRROR.start).unwrap(), 0xff);
    }

    #[test]
    fn test_read_oamram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.oamram[0] = 0xff;

        assert_eq!(memory.read_u8(OAM_RAM.start).unwrap(), 0xff);
    }

    #[test]
    fn test_read_cpuram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.cpuram[0] = 0xff;

        assert_eq!(memory.read_u8(CPU_RAM.start).unwrap(), 0xff);
    }

    #[test]
    fn test_dma() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(DMA_REGISTER_ADDR, 0x12).unwrap();

        assert_eq!(memory.registers.dma, 0x12);

        memory.registers.dma = 0x34;
        assert_eq!(memory.read_u8(DMA_REGISTER_ADDR).unwrap(), 0x34);
    }

    #[test]
    fn test_interrupt_registers() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(INTERRUPT_FLAG_ADDR, 0xFF).unwrap();
        memory.write_u8(INTERRUPT_ENABLE_ADDR, 0xFE).unwrap();

        assert_eq!(memory.registers.iflag, 0x1F);
        assert_eq!(memory.registers.ie, 0x1E);

        memory.registers.iflag = 0x04;
        memory.registers.ie = 0x12;

        assert_eq!(memory.read_u8(INTERRUPT_FLAG_ADDR).unwrap(), 0x04);
        assert_eq!(memory.read_u8(INTERRUPT_ENABLE_ADDR).unwrap(), 0x12);
    }

    #[test]
    fn test_unmapped_address() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        let addr = 0xFEC0;

        assert_eq!(
            memory.read_u8(addr),
            Err(MemoryError::UnmappedAddress(addr))
        );
        assert_eq!(
            memory.write_u8(addr, 0xFE),
            Err(MemoryError::UnmappedAddress(addr))
        );
    }
}
