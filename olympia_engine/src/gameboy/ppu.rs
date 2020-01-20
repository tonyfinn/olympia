use alloc::collections::VecDeque;

use crate::gameboy::cpu::Interrupt;
use crate::gameboy::memory::Memory;

const VISIBLE_WIDTH: u8 = 160;
const VISIBLE_LINES: u8 = 144;
const TOTAL_LINES: u8 = 154;
const OAM_SCAN_CYCLES: u8 = 20;
const LINE_CYCLES: u8 = 114;

const MODE_LCDSTAT_MASK: u8 = 3;
const MATCH_FLAG_LCDSTAT_MASK: u8 = 1 << 2;
const HBLANK_LCDSTAT_MASK: u8 = 1 << 3;
const VBLANK_LCDSTAT_MASK: u8 = 1 << 4;
const OAM_SCAN_LCDSTAT_MASK: u8 = 1 << 5;
const LINE_MATCH_INT_LCDSTAT_MASK: u8 = 1 << 6;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PPUPhase {
    ObjectScan,
    Drawing,
    HBlank,
    VBlank,
}

pub enum Palette {
    Background,
    Window,
    Sprite0,
    Sprite1,
}

pub struct QueuedPixel {
    palette: Palette,
    palette_index: u8,
}

pub(crate) struct PPU {
    framebuffer: [u8; (VISIBLE_LINES as usize) * (VISIBLE_WIDTH as usize)],
    pixel_queue: VecDeque<QueuedPixel>,
    phase: PPUPhase,
    current_line: u8,
    cycles_on_line: u8,
    next_pixel: u8,
}

impl PPU {
    fn new() -> PPU {
        PPU {
            framebuffer: [0; (VISIBLE_LINES as usize) * (VISIBLE_WIDTH as usize)],
            pixel_queue: VecDeque::new(),
            phase: PPUPhase::ObjectScan,
            current_line: 0,
            cycles_on_line: 0,
            next_pixel: 0,
        }
    }

    pub(crate) fn run_cycle(&mut self, mem: &mut Memory) {
        if self.is_enabled(mem) {
            self.update_phase(mem);
        }
    }

    fn should_trigger_line_interrupt(&self, lcdstat: u8, check_line: u8, current_line: u8) -> bool {
        (((lcdstat & MATCH_FLAG_LCDSTAT_MASK) != 0) == (current_line == check_line))
            && ((lcdstat & LINE_MATCH_INT_LCDSTAT_MASK) != 0)
    }

    fn update_phase(&mut self, mem: &mut Memory) {
        self.cycles_on_line += 1;
        if self.cycles_on_line == LINE_CYCLES {
            self.cycles_on_line = 0;
            self.next_pixel = 0;
            self.current_line += 1;
            if self.current_line == TOTAL_LINES {
                self.current_line = 0;
            }
            if self.should_trigger_line_interrupt(
                mem.registers.lcdstat,
                mem.registers.lyc,
                self.current_line,
            ) {
                Interrupt::LCDStatus.set(&mut mem.registers.ie);
            }
            if self.current_line == VISIBLE_LINES {
                self.phase = PPUPhase::VBlank;
                mem.registers.lcdstat = (mem.registers.lcdstat & !MODE_LCDSTAT_MASK) | 0b01;
                Interrupt::VBlank.set(&mut mem.registers.ie);
                if (mem.registers.lcdstat & VBLANK_LCDSTAT_MASK) != 0 {
                    Interrupt::LCDStatus.set(&mut mem.registers.ie);
                }
            } else if self.current_line < VISIBLE_LINES {
                self.phase = PPUPhase::ObjectScan;
                mem.registers.lcdstat = (mem.registers.lcdstat & !MODE_LCDSTAT_MASK) | 0b10;
                if (mem.registers.lcdstat & OAM_SCAN_LCDSTAT_MASK) != 0 {
                    Interrupt::LCDStatus.set(&mut mem.registers.ie);
                }
            }
            mem.registers.ly = self.current_line;
        } else if self.cycles_on_line == OAM_SCAN_CYCLES && self.current_line < VISIBLE_LINES {
            self.phase = PPUPhase::Drawing;
            mem.registers.lcdstat = (mem.registers.lcdstat & !MODE_LCDSTAT_MASK) | 0b11;
        } else if self.next_pixel >= VISIBLE_WIDTH {
            self.phase = PPUPhase::HBlank;
            mem.registers.lcdstat = (mem.registers.lcdstat & !MODE_LCDSTAT_MASK) | 0b00;
            if (mem.registers.lcdstat & HBLANK_LCDSTAT_MASK) != 0 {
                Interrupt::LCDStatus.set(&mut mem.registers.ie);
            }
        }
    }

    fn is_enabled(&self, mem: &Memory) -> bool {
        (mem.registers.lcdc & 0x80) == 0
    }
}

impl Default for PPU {
    fn default() -> PPU {
        PPU::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rom::Cartridge;

    fn create_memory() -> Memory {
        let cart = Cartridge::from_data(vec![0; 0x1000]).unwrap();
        Memory::new(cart)
    }

    #[test]
    fn hblank_end_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 100;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        ppu.next_pixel = VISIBLE_WIDTH;
        memory.registers.lcdstat = 0b00;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.current_line, 101);
        assert_eq!(ppu.next_pixel, 0);
        assert_eq!(ppu.cycles_on_line, 0);
        assert_eq!(ppu.phase, PPUPhase::ObjectScan);
        assert_eq!(memory.registers.lcdstat, 0b10);
    }

    #[test]
    fn hblank_end_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 100;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        memory.registers.lcdstat = 0b0010_0000;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers.ie).expect("No interrupt triggered");
        assert_eq!(lcd_active_interrupt, Interrupt::LCDStatus);
    }

    #[test]
    fn lyc_match_eq_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 100;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        memory.registers.lyc = 101;
        memory.registers.lcdstat = 0b0100_0100;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers.ie).expect("No interrupt triggered");
        assert_eq!(lcd_active_interrupt, Interrupt::LCDStatus);
    }

    #[test]
    fn lyc_no_match_eq_no_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 101;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        memory.registers.lyc = 101;
        memory.registers.lcdstat = 0b0100_0100;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers.ie);
        assert!(lcd_active_interrupt.is_none());
    }

    #[test]
    fn lyc_match_ne_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 101;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        memory.registers.lyc = 101;
        memory.registers.lcdstat = 0b0100_0000;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers.ie).expect("No interrupt triggered");
        assert_eq!(lcd_active_interrupt, Interrupt::LCDStatus);
    }

    #[test]
    fn lyc_no_match_ne_no_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 100;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        memory.registers.lyc = 101;
        memory.registers.lcdstat = 0b0100_0000;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers.ie);
        assert!(lcd_active_interrupt.is_none());
    }

    #[test]
    fn hblank_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::Drawing;
        ppu.current_line = 101;
        ppu.cycles_on_line = LINE_CYCLES - 30;
        ppu.next_pixel = VISIBLE_WIDTH;
        memory.registers.lcdstat = 0b11;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.phase, PPUPhase::HBlank);
        assert_eq!(memory.registers.lcdstat, 0b00);
    }

    #[test]
    fn hblank_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::Drawing;
        ppu.current_line = 101;
        ppu.cycles_on_line = LINE_CYCLES - 30;
        ppu.next_pixel = VISIBLE_WIDTH;
        memory.registers.lcdstat = 0b1011;
        ppu.update_phase(&mut memory);
        let active_interrupt =
            Interrupt::test(0x1F, memory.registers.ie).expect("No interrupt triggered");
        assert_eq!(active_interrupt, Interrupt::LCDStatus);
    }

    #[test]
    fn vblank_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = VISIBLE_LINES - 1;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        ppu.next_pixel = VISIBLE_WIDTH;
        memory.registers.lcdstat = 0b11;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.phase, PPUPhase::VBlank);
        assert_eq!(memory.registers.lcdstat, 0b01);
    }

    #[test]
    fn vblank_end_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = TOTAL_LINES - 1;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        ppu.next_pixel = VISIBLE_WIDTH;
        memory.registers.lcdstat = 0b11;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.phase, PPUPhase::ObjectScan);
        assert_eq!(memory.registers.lcdstat, 0b10);
        assert_eq!(ppu.current_line, 0);
        assert_eq!(ppu.next_pixel, 0);
        assert_eq!(ppu.cycles_on_line, 0);
    }

    #[test]
    fn vblank_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = VISIBLE_LINES - 1;
        ppu.cycles_on_line = LINE_CYCLES - 1;
        memory.registers.lcdstat = 0b0111_1011;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers.ie).expect("No interrupt triggered");
        assert_eq!(lcd_active_interrupt, Interrupt::LCDStatus);
        let vblank_active_interrupt =
            Interrupt::test(0x01, memory.registers.ie).expect("No interrupt triggered");
        assert_eq!(vblank_active_interrupt, Interrupt::VBlank);
    }

    #[test]
    fn scan_end_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::ObjectScan;
        ppu.current_line = 100;
        ppu.cycles_on_line = OAM_SCAN_CYCLES - 1;
        ppu.next_pixel = 0;
        memory.registers.lcdstat = 0b10;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.phase, PPUPhase::Drawing);
        assert_eq!(memory.registers.lcdstat, 0b11);
        assert_eq!(ppu.current_line, 100);
        assert_eq!(ppu.next_pixel, 0);
        assert_eq!(ppu.cycles_on_line, OAM_SCAN_CYCLES);
    }
}
