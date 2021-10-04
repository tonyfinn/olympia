use crate::events;
use crate::rom::Cartridge;
use derive_more::Display;

use olympia_core::address;

pub(crate) const TIMER_DIVIDER_REGISTER: u16 = 0xff04;
pub(crate) const TIMER_COUNTER_REGISTER: u16 = 0xff05;
pub(crate) const TIMER_MODULO_REGISTER: u16 = 0xff06;
pub(crate) const TIMER_CONTROL_REGISTER: u16 = 0xff07;

pub(crate) const DMA_REGISTER_ADDR: u16 = 0xff46;

pub(crate) const LCD_CONTROL_ADDR: u16 = 0xFF40;
pub(crate) const LCD_STATUS_ADDR: u16 = 0xFF41;
pub(crate) const SCROLL_Y_ADDR: u16 = 0xFF42;
pub(crate) const SCROLL_X_ADDR: u16 = 0xFF43;
pub(crate) const WINDOW_Y_ADDR: u16 = 0xFF4A;
pub(crate) const WINDOW_X_ADDR: u16 = 0xFF4B;
pub(crate) const CURRENT_LINE_ADDR: u16 = 0xFF44;
pub(crate) const LINE_CHECK_ADDR: u16 = 0xFF45;

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
pub const MODEL_RESERVED: MemoryRegion = MemoryRegion::new(0xFEA0, 0x60, "modelreserved");

#[derive(PartialEq, Eq, Debug, Clone, Display)]
/// Represents a failure to read from memory.
pub enum MemoryError {
    /// The address maps to the Cartridge ROM area,
    /// but the currently loaded cartridge does not have
    /// ROM at this address. This can happen for MBC1/SROM cartridges
    /// that have less than 8KB of storage
    #[display(fmt = "Invalid address in ROM: 0x{:X}", "_0")]
    InvalidRomAddress(u16),
    /// The address maps to the Cartridge RAM area,
    /// but the currently loaded cartridge does not have
    /// RAM at this address. This can happen for cartridges
    /// that have < 2KB of RAM, including no RAM
    #[display(fmt = "Invalid address in RAM: 0x{:X}", "_0")]
    InvalidRamAddress(u16),
    /// The address maps to an area that is unmapped for the
    /// current gameboy model. This currently includes unimplemented registers,
    /// or registers that only exist on Game Boy Color
    #[display(fmt = "Unmapped address: 0x{:X}", "_0")]
    UnmappedAddress(u16),
}

pub type MemoryResult<T> = Result<T, MemoryError>;

pub(crate) struct MemoryIterator<'a> {
    addr: address::LiteralAddress,
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

fn masked_write(current: &mut u8, new: u8, mask: u8) {
    *current = (new & mask) | (*current & !mask);
}

pub struct MemoryRegisters {
    /// Write upper byte of start addresses here to trigger DMA transfers
    /// to OAM RAM
    pub(crate) dma: u8,
    /// Bit 7 = LCD on/off, Bit 6 = Window code area, bit 5 = window on/off
    /// bit 4 = BG tile area (1 = fully overlapping, 0 = 50% overlap)
    /// bit 3 = BG code area, bit 2 = sprite size (1 = 8x16, 0 = 8x8)
    /// bit 1  = object layer enable, bit 0 = bg layer enable
    pub(crate) lcdc: u8,
    /// Bits 3-6 control interrupts, bit 2 inverts line checks, bit 0-1
    /// exposes current PPU mode
    pub(crate) lcdstat: u8,
    /// Scroll the screen vertically this many pixels
    pub(crate) scy: u8,
    /// Scroll the screen horizontally this many pixels
    pub(crate) scx: u8,
    /// Current line being drawn by the PPU
    pub(crate) ly: u8,
    /// Line to check LY against for interrupts on specific line
    pub(crate) lyc: u8,
    /// Y Pixel offset (in screen co-ordinates, not tile map) to start window
    pub(crate) wy: u8,
    /// X Pixel offset (in screen co-ordinates, not tile map) to start window
    pub(crate) wx: u8,
    /// Interrupts where their conditions have been triggered
    pub(crate) iflag: u8,
    /// Interrupts that are enabled and can cause CPU interrupts
    pub(crate) ie: u8,
    /// Divider - Contains clock ticking at 16384 hz
    pub(crate) div: u8,
    /// Timer Counter - Countains click ticks at user defined rate
    pub(crate) tima: u8,
    /// Timer modulo - Contains value to reset timer to after overflow
    pub(crate) tma: u8,
    /// Timer control - Controls TIMA enabled + tick rate
    pub(crate) tac: u8,
}

impl MemoryRegisters {
    fn new() -> MemoryRegisters {
        MemoryRegisters {
            dma: 0,
            lcdc: 0x91,
            lcdstat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            wy: 0,
            wx: 0,
            iflag: 0,
            ie: 0,
            div: 0x18,
            tima: 0,
            tma: 0,
            tac: 0xF8,
        }
    }

    fn read(&self, addr: u16) -> Option<u8> {
        match addr {
            TIMER_DIVIDER_REGISTER => Some(self.div),
            TIMER_COUNTER_REGISTER => Some(self.tima),
            TIMER_MODULO_REGISTER => Some(self.tma),
            TIMER_CONTROL_REGISTER => Some(self.tac),

            DMA_REGISTER_ADDR => Some(self.dma),

            LCD_CONTROL_ADDR => Some(self.lcdc),
            LCD_STATUS_ADDR => Some(self.lcdstat),
            SCROLL_Y_ADDR => Some(self.scy),
            SCROLL_X_ADDR => Some(self.scx),
            CURRENT_LINE_ADDR => Some(self.ly),
            LINE_CHECK_ADDR => Some(self.lyc),
            WINDOW_Y_ADDR => Some(self.wy),
            WINDOW_X_ADDR => Some(self.wx),

            INTERRUPT_FLAG_ADDR => Some(self.iflag),
            INTERRUPT_ENABLE_ADDR => Some(self.ie),

            _ => None,
        }
    }

    fn write(&mut self, addr: u16, value: u8) {
        match addr {
            TIMER_DIVIDER_REGISTER => self.div = 0,
            TIMER_COUNTER_REGISTER => self.tima = value,
            TIMER_MODULO_REGISTER => self.tma = value,
            TIMER_CONTROL_REGISTER => masked_write(&mut self.tac, value, 0b111),

            DMA_REGISTER_ADDR => self.dma = value,

            LCD_CONTROL_ADDR => self.lcdc = value,
            // Top bit doesn't exist
            // Lower two bits are mode flag
            LCD_STATUS_ADDR => masked_write(&mut self.lcdstat, value, 0b0111_1100),
            SCROLL_Y_ADDR => self.scy = value,
            SCROLL_X_ADDR => self.scx = value,
            CURRENT_LINE_ADDR => (), // Read only
            LINE_CHECK_ADDR => self.lyc = value,
            WINDOW_Y_ADDR => self.wy = value,
            WINDOW_X_ADDR => self.wx = value,

            INTERRUPT_FLAG_ADDR => masked_write(&mut self.iflag, value, 0x1F),
            INTERRUPT_ENABLE_ADDR => masked_write(&mut self.ie, value, 0x1F),

            _ => (),
        }
    }
}

fn is_mem_register(addr: u16) -> bool {
    MEM_REGISTERS.contains(addr) || addr == 0xffff
}

pub struct MemoryData {
    cpuram: [u8; 127],
    oamram: [u8; 160],
    sysram: [u8; 0x2000],
    vram: [u8; 0x2000],
    cartridge: Cartridge,
    pub(crate) registers: MemoryRegisters,
}

pub struct Memory {
    data: MemoryData,
    pub events: events::EventEmitter<events::MemoryEvent>,
}

impl Memory {
    pub fn new(cartridge: Cartridge) -> Memory {
        Memory {
            data: MemoryData {
                cpuram: [0u8; 127],
                oamram: [0u8; 160],
                sysram: [0u8; 0x2000],
                vram: [0u8; 0x2000],
                cartridge,
                registers: MemoryRegisters::new(),
            },
            events: events::EventEmitter::new(),
        }
    }

    pub fn registers(&self) -> &MemoryRegisters {
        &self.data.registers
    }

    pub fn registers_mut(&mut self) -> &mut MemoryRegisters {
        &mut self.data.registers
    }

    pub fn read_u8<A: Into<address::LiteralAddress>>(&self, target: A) -> MemoryResult<u8> {
        let address = target.into();
        let result = self.read_u8_internal(address);

        if let Ok(value) = result {
            self.events.emit(events::MemoryEvent::read(address, value));
        };

        result
    }

    pub(crate) fn read_u8_internal(
        &self,
        address: address::LiteralAddress,
    ) -> Result<u8, MemoryError> {
        let addr = address.0;
        if CARTRIDGE_ROM.contains(addr) || SWITCHABLE_ROM.contains(addr) {
            self.data
                .cartridge
                .read(addr)
                .map_err(|_| MemoryError::InvalidRomAddress(addr))
        } else if VRAM.contains(addr) {
            Ok(self.data.vram[(addr - VRAM.start) as usize])
        } else if CARTRIDGE_RAM.contains(addr) {
            self.data
                .cartridge
                .read(addr)
                .map_err(|_| MemoryError::InvalidRamAddress(addr))
        } else if SYS_RAM.contains(addr) {
            Ok(self.data.sysram[(addr - SYS_RAM.start) as usize])
        } else if SYS_RAM_MIRROR.contains(addr) {
            Ok(self.data.sysram[(addr - SYS_RAM_MIRROR.start) as usize])
        } else if OAM_RAM.contains(addr) {
            Ok(self.data.oamram[(addr - OAM_RAM.start) as usize])
        } else if CPU_RAM.contains(addr) {
            Ok(self.data.cpuram[(addr - CPU_RAM.start) as usize])
        } else if MODEL_RESERVED.contains(addr) {
            // DMG/SGB behaviour is to return 0
            Ok(0)
        } else if is_mem_register(addr) {
            self.data
                .registers
                .read(addr)
                .ok_or_else(|| MemoryError::UnmappedAddress(addr))
        } else {
            Err(MemoryError::UnmappedAddress(addr))
        }
    }

    pub fn write_u8<A: Into<address::LiteralAddress>>(
        &mut self,
        target: A,
        value: u8,
    ) -> MemoryResult<()> {
        let address = target.into();
        let write_result = self.write_u8_internal(address, value);

        if write_result.is_ok() {
            // need to read the actual new value in case of partial registers
            // unmapped memory, or writes to ROM address space
            let new_value = self.read_u8_internal(address).unwrap_or(0xFF);
            self.events
                .emit(events::MemoryEvent::write(address, value, new_value));
        }

        write_result
    }

    pub(crate) fn write_u8_internal(
        &mut self,
        address: address::LiteralAddress,
        value: u8,
    ) -> Result<(), MemoryError> {
        let addr = address.0;
        let write_result = if CARTRIDGE_ROM.contains(addr) || SWITCHABLE_ROM.contains(addr) {
            self.data
                .cartridge
                .write(addr, value)
                .map_err(|_| MemoryError::InvalidRomAddress(addr))
        } else if VRAM.contains(addr) {
            self.data.vram[(addr - VRAM.start) as usize] = value;
            Ok(())
        } else if CARTRIDGE_RAM.contains(addr) {
            self.data
                .cartridge
                .write(addr, value)
                .map_err(|_| MemoryError::InvalidRamAddress(addr))
        } else if SYS_RAM.contains(addr) {
            self.data.sysram[(addr - SYS_RAM.start) as usize] = value;
            Ok(())
        } else if SYS_RAM_MIRROR.contains(addr) {
            self.data.sysram[(addr - SYS_RAM_MIRROR.start) as usize] = value;
            Ok(())
        } else if OAM_RAM.contains(addr) {
            self.data.oamram[(addr - OAM_RAM.start) as usize] = value;
            Ok(())
        } else if is_mem_register(addr) {
            self.data.registers.write(addr, value);
            Ok(())
        } else if CPU_RAM.contains(addr) {
            self.data.cpuram[(addr - CPU_RAM.start) as usize] = value;
            Ok(())
        } else if MODEL_RESERVED.contains(addr) {
            // DMG/SGB behaviour is to ignore usage
            Ok(())
        } else {
            panic!("Tried to write to unmapped address {} - all addresses should be implemented for writes", address)
        };
        write_result
    }

    pub(crate) fn offset_iter(&self, start: address::LiteralAddress) -> MemoryIterator {
        MemoryIterator {
            addr: start,
            mem: &self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::vec::Vec;

    #[test]
    fn test_write_vram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(VRAM.start, 0xff).unwrap();
        assert_eq!(memory.data.vram[0], 0xff);
    }

    #[test]
    fn test_write_sysram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(SYS_RAM.start, 0xff).unwrap();
        assert_eq!(memory.data.sysram[0], 0xff);
    }

    #[test]
    fn test_write_sysram_mirror() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(SYS_RAM_MIRROR.start, 0xff).unwrap();
        assert_eq!(memory.data.sysram[0], 0xff);
    }

    #[test]
    fn test_write_oamram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(OAM_RAM.start, 0xff).unwrap();
        assert_eq!(memory.data.oamram[0], 0xff);
    }

    #[test]
    fn test_write_cpuram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(CPU_RAM.start, 0xff).unwrap();
        assert_eq!(memory.data.cpuram[0], 0xff);
    }

    #[test]
    fn test_read_vram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.data.vram[0] = 0xff;

        assert_eq!(memory.read_u8(VRAM.start).unwrap(), 0xff);
    }

    #[test]
    fn test_read_sysram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.data.sysram[0] = 0xff;

        assert_eq!(memory.read_u8(SYS_RAM.start).unwrap(), 0xff);
    }

    #[test]
    fn test_read_sysram_mirror() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.data.sysram[0] = 0xff;

        assert_eq!(memory.read_u8(SYS_RAM_MIRROR.start).unwrap(), 0xff);
    }

    #[test]
    fn test_read_oamram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.data.oamram[0] = 0xff;

        assert_eq!(memory.read_u8(OAM_RAM.start).unwrap(), 0xff);
    }

    #[test]
    fn test_read_cpuram() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.data.cpuram[0] = 0xff;

        assert_eq!(memory.read_u8(CPU_RAM.start).unwrap(), 0xff);
    }

    #[test]
    fn test_dma() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(DMA_REGISTER_ADDR, 0x12).unwrap();

        assert_eq!(memory.data.registers.dma, 0x12);

        memory.data.registers.dma = 0x34;
        assert_eq!(memory.read_u8(DMA_REGISTER_ADDR).unwrap(), 0x34);
    }

    #[test]
    fn test_interrupt_registers() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.write_u8(INTERRUPT_FLAG_ADDR, 0xFF).unwrap();
        memory.write_u8(INTERRUPT_ENABLE_ADDR, 0xFE).unwrap();

        assert_eq!(memory.data.registers.iflag, 0x1F);
        assert_eq!(memory.data.registers.ie, 0x1E);

        memory.data.registers.iflag = 0x04;
        memory.data.registers.ie = 0x12;

        assert_eq!(memory.read_u8(INTERRUPT_FLAG_ADDR).unwrap(), 0x04);
        assert_eq!(memory.read_u8(INTERRUPT_ENABLE_ADDR).unwrap(), 0x12);
    }

    #[test]
    fn test_lcd_registers() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);

        memory.data.registers.lcdc = 0;
        memory.data.registers.lcdstat = 3;

        memory.write_u8(LCD_STATUS_ADDR, 0xFC).unwrap();
        memory.write_u8(LCD_CONTROL_ADDR, 0xFF).unwrap();
        memory.write_u8(SCROLL_Y_ADDR, 0xAA).unwrap();
        memory.write_u8(SCROLL_X_ADDR, 0x33).unwrap();
        memory.write_u8(WINDOW_Y_ADDR, 0x3A).unwrap();
        memory.write_u8(WINDOW_X_ADDR, 0xA3).unwrap();

        assert_eq!(memory.data.registers.lcdc, 0xFF);
        assert_eq!(memory.data.registers.lcdstat, 0x7F);

        assert_eq!(memory.read_u8(LCD_STATUS_ADDR).unwrap(), 0x7F);
        assert_eq!(memory.read_u8(LCD_CONTROL_ADDR).unwrap(), 0xFF);
        assert_eq!(memory.read_u8(SCROLL_Y_ADDR).unwrap(), 0xAA);
        assert_eq!(memory.read_u8(SCROLL_X_ADDR).unwrap(), 0x33);
        assert_eq!(memory.read_u8(WINDOW_Y_ADDR).unwrap(), 0x3A);
        assert_eq!(memory.read_u8(WINDOW_X_ADDR).unwrap(), 0xA3);
    }

    #[test]
    fn test_unmapped_address() {
        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let memory = Memory::new(cartridge);

        let addr = 0xFF50;

        assert_eq!(
            memory.read_u8(addr),
            Err(MemoryError::UnmappedAddress(addr))
        );
    }

    #[test]
    fn test_write_event() {
        use core::cell::RefCell;
        let event_log: Rc<RefCell<Vec<events::MemoryEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let handler_log = Rc::clone(&event_log);

        let handler = move |evt: &events::MemoryEvent| {
            handler_log.borrow_mut().push(evt.clone());
        };

        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.events.on(Box::new(handler));

        memory.write_u8(0x9000, 0x26).unwrap();

        let actual_events = event_log.borrow();

        assert_eq!(
            *actual_events,
            vec![events::MemoryEvent::write(0x9000.into(), 0x26, 0x26,).into()]
        );
    }

    #[test]
    fn test_write_unwriteable() {
        use core::cell::RefCell;
        let event_log: Rc<RefCell<Vec<events::MemoryEvent>>> = Rc::new(RefCell::new(Vec::new()));
        let handler_log = Rc::clone(&event_log);

        let handler = move |evt: &events::MemoryEvent| {
            handler_log.borrow_mut().push(evt.clone());
        };

        let cartridge = Cartridge::from_data(vec![0u8; 0x8000]).unwrap();
        let mut memory = Memory::new(cartridge);
        memory.events.on(Box::new(handler));

        memory.write_u8(0x1000, 0x26).unwrap();

        let actual_events = event_log.borrow();

        assert_eq!(
            *actual_events,
            vec![events::MemoryEvent::write(0x1000.into(), 0x26, 0x00,).into()]
        );
    }
}
