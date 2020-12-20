use alloc::collections::VecDeque;

use crate::{
    events::{EventEmitter, HBlankEvent, PPUEvent, VBlankEvent},
    gameboy::{cpu::Interrupt, memory::{Memory, OAM_RAM}},
};

const VISIBLE_WIDTH: u8 = 160;
const VISIBLE_LINES: u8 = 144;
const TOTAL_LINES: u8 = 154;
const OAM_SCAN_CYCLES: u16 = 20;
const LINE_CYCLES: u16 = 114;

const MODE_MASK: u8 = 3;
const MODE_HBLANK: u8 = 0b00;
const MODE_VBLANK: u8 = 0b01;
const MODE_OAMSCAN: u8 = 0b10;
const MODE_DRAWING: u8 = 0b11;

const LCDSTAT_MATCH_ON_EQUAL: u8 = 1 << 2;
const LCDSTAT_HBLANK_INTERRUPT: u8 = 1 << 3;
const LCDSTAT_VBLANK_INTERRUPT: u8 = 1 << 4;
const LCDSTAT_OAM_SCAN_INTERRUPT: u8 = 1 << 5;
const LCDSTAT_LINE_MATCH_INTERRUPT: u8 = 1 << 6;

const LCDC_SPRITE_ENABLE: u8 = 1 << 1;
const LCDC_LARGE_SPRITE: u8 = 1 << 2;
const LCDC_HIGH_BG_MAP: u8 = 1 << 3;
const LCDC_LOW_BG_TILES: u8 = 1 << 4;
const LCDC_WINDOW_ENABLED: u8 = 1 << 5;
const LCDC_HIGH_WINDOW_MAP: u8 = 1 << 6;
const LCDC_ENABLED: u8 = 1 << 7;

const MEM_LOW_TILES: u16 = 0x8000;
const MEM_HIGH_TILES: u16 = 0x8800;
const MEM_LOW_MAP: u16 = 0x9800;
const MEM_HIGH_MAP: u16 = 0x9C00;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PPUPhase {
    ObjectScan,
    Drawing,
    HBlank,
    VBlank,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Palette {
    Background,
    Window,
    Sprite0,
    Sprite1,
}

impl Default for Palette {
    fn default() -> Self {
        Palette::Background
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct GBPixel {
    pub palette: Palette,
    pub index: u8,
}

impl GBPixel {
    pub fn new(palette: Palette, index: u8) -> GBPixel {
        GBPixel { palette, index }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct Sprite {
    y: u8,
    x: u8,
    tile: u8,
    flags: u8,
}

impl Sprite {
    fn from_oam_ram(mem: &Memory, index: u8) -> Sprite {
        let sprite_offset  = OAM_RAM.start + (4 * u16::from(index));
        let y = mem.read_u8(sprite_offset).unwrap();
        let x = mem.read_u8(sprite_offset + 1).unwrap();
        let tile = mem.read_u8(sprite_offset + 2).unwrap();
        let flags = mem.read_u8(sprite_offset + 3).unwrap();

        Sprite {
            y,
            x,
            tile,
            flags
        }
    }

    fn visible_on_line(&self, y: u8, height: u8) -> bool {
        (y >= self.y) && (y < (self.y + height))
    }
}

pub(crate) struct PPU {
    framebuffer: [GBPixel; (VISIBLE_LINES as usize) * (VISIBLE_WIDTH as usize)],
    pixel_queue: VecDeque<GBPixel>,
    phase: PPUPhase,
    current_line: u8,
    clocks_on_line: u16,
    current_pixel: u8,
    line_sprites: Vec<Sprite>,
    pub(crate) events: EventEmitter<PPUEvent>,
}

impl PPU {
    fn new() -> PPU {
        PPU {
            framebuffer: [GBPixel::default(); (VISIBLE_LINES as usize) * (VISIBLE_WIDTH as usize)],
            pixel_queue: VecDeque::new(),
            phase: PPUPhase::ObjectScan,
            current_line: 0,
            clocks_on_line: 0,
            current_pixel: 0,
            line_sprites: Vec::with_capacity(10),
            events: EventEmitter::new(),
        }
    }

    pub(crate) fn run_cycle(&mut self, mem: &mut Memory) {
        if self.is_enabled(mem) {
            for i in 0..4 {
                if self.phase == PPUPhase::Drawing {
                    self.draw(mem);
                }
                if i % 2 == 0 {
                    self.update_phase(mem);
                }
            }
        }
    }

    fn should_trigger_line_interrupt(&self, lcdstat: u8, check_line: u8, current_line: u8) -> bool {
        (((lcdstat & LCDSTAT_MATCH_ON_EQUAL) != 0) == (current_line == check_line))
            && ((lcdstat & LCDSTAT_LINE_MATCH_INTERRUPT) != 0)
    }

    fn update_phase(&mut self, mem: &mut Memory) {
        if self.clocks_on_line == 0 {
            self.oam_scan(mem);
        }
        self.clocks_on_line += 2; // Draw 1 pixel every 2 clocks
        let cycles_on_line = self.clocks_on_line / 4;
        if cycles_on_line == LINE_CYCLES {
            self.end_of_line(mem);
        } else if self.current_pixel >= VISIBLE_WIDTH && self.phase == PPUPhase::Drawing {
            let pixels = self.pixel_queue.drain(..).collect();
            self.events.emit(HBlankEvent { pixels }.into());
            self.phase = PPUPhase::HBlank;
            mem.registers_mut().lcdstat = (mem.registers().lcdstat & !MODE_MASK) | MODE_HBLANK;
            if (mem.registers().lcdstat & LCDSTAT_HBLANK_INTERRUPT) != 0 {
                Interrupt::LCDStatus.set(&mut mem.registers_mut().ie);
            }
        } else if cycles_on_line == OAM_SCAN_CYCLES && self.current_line < VISIBLE_LINES {
            self.phase = PPUPhase::Drawing;
            mem.registers_mut().lcdstat = (mem.registers().lcdstat & !MODE_MASK) | MODE_DRAWING;
        }
    }

    fn oam_scan(&mut self, mem: &mut Memory) {
        let mut sprites: Vec<Sprite> = Vec::with_capacity(10);
        for i in 0..40u8 {
            let sprite = Sprite::from_oam_ram(mem, i);
            if sprite.visible_on_line(self.current_line, self.sprite_height(mem)) {
                sprites.push(sprite);
            }
            if sprites.len() == 10 {
                break;
            }
        }
        self.line_sprites = sprites
    }

    fn end_of_line(&mut self, mem: &mut Memory) {
        self.clocks_on_line = 0;
        self.current_pixel = 0;
        self.current_line += 1;
        if self.current_line == TOTAL_LINES {
            self.current_line = 0;
        }
        if self.should_trigger_line_interrupt(
            mem.registers().lcdstat,
            mem.registers().lyc,
            self.current_line,
        ) {
            Interrupt::LCDStatus.set(&mut mem.registers_mut().ie);
        }
        if self.current_line == VISIBLE_LINES {
            self.events.emit(VBlankEvent.into());
            self.phase = PPUPhase::VBlank;
            mem.registers_mut().lcdstat = (mem.registers().lcdstat & !MODE_MASK) | MODE_VBLANK;
            Interrupt::VBlank.set(&mut mem.registers_mut().ie);
            if (mem.registers().lcdstat & LCDSTAT_VBLANK_INTERRUPT) != 0 {
                Interrupt::LCDStatus.set(&mut mem.registers_mut().ie);
            }
        } else if self.current_line < VISIBLE_LINES {
            self.phase = PPUPhase::ObjectScan;
            mem.registers_mut().lcdstat = (mem.registers().lcdstat & !MODE_MASK) | MODE_OAMSCAN;
            if (mem.registers().lcdstat & LCDSTAT_OAM_SCAN_INTERRUPT) != 0 {
                Interrupt::LCDStatus.set(&mut mem.registers_mut().ie);
            }
        }
        mem.registers_mut().ly = self.current_line;
    }

    fn read_pixel_palette_index(&self, mem: &Memory, tile_base: u16, x: u8, y: u8) -> u8 {
        let lower_addr = tile_base + (u16::from(y) * 2);

        let lower_byte = mem.read_u8(lower_addr).unwrap_or(0);
        let upper_byte = mem.read_u8(lower_addr + 1).unwrap_or(0);

        let upper_byte_value = (upper_byte >> (7 - x)) & 1;
        let lower_byte_value = (lower_byte >> (7 - x)) & 1;

        lower_byte_value | (upper_byte_value << 1)
    }

    fn draw(&mut self, mem: &Memory) {
        if self.current_pixel >= VISIBLE_WIDTH {
            return;
        }
        let actual_x = mem.registers().scx + self.current_pixel;
        let actual_y = mem.registers().scy + self.current_line;

        let pixel = self.calculate_pixel(mem, actual_x, actual_y);
        self.pixel_queue.push_back(pixel);
        let fb_index = usize::from(actual_x) + (usize::from(actual_y) * usize::from(VISIBLE_WIDTH));
        self.framebuffer[fb_index] = pixel;

        self.current_pixel += 1;
    }

    fn calculate_sprite_pixel(&mut self, mem: &Memory, x: u8, y: u8) -> Option<GBPixel> {
        for sprite in self.line_sprites.iter() {
            if x >= sprite.x && x < sprite.x + 8 {
                let sprite_px = x - sprite.x;
                let sprite_py = y - sprite.y;
                let tile_base = MEM_LOW_TILES + (u16::from(sprite.tile) * 0x10);
                let palette_index = self.read_pixel_palette_index(mem, tile_base, sprite_px, sprite_py);

                if palette_index == 0 {
                    return None
                }

                let palette = if (sprite.flags & 0x10) == 0 {
                    Palette::Sprite0
                } else {
                    Palette::Sprite1
                };

                return Some(GBPixel::new(palette, palette_index));
            }
        }
        None
    }

    fn calculate_pixel(&mut self, mem: &Memory, x: u8, y: u8) -> GBPixel {
        if self.sprites_enabled(mem) {
            if let Some(px) = self.calculate_sprite_pixel(mem, x, y) {
                return px;
            }
        }

        let tile_x = x / 8;
        let tile_y = y / 8;

        let is_window = (self.current_pixel >= mem.registers().wx)
            && (self.current_line >= mem.registers().wy)
            && self.window_enabled(mem);

        let map_offset = if is_window {
            self.window_map_offset(mem)
        } else {
            self.background_map_offset(mem)
        };

        let tile_id_addr = map_offset + (u16::from(tile_y) * 32) + u16::from(tile_x);
        let tile_at_pixel = mem.read_u8(tile_id_addr).unwrap_or(0);

        let tile_base = self.background_tile_offset(mem) + (u16::from(tile_at_pixel) * 0x10);
        let tile_offset_x = x % 8;
        let tile_offset_y = y % 8;

        let palette_index =
            self.read_pixel_palette_index(mem, tile_base, tile_offset_x, tile_offset_y);
        let palette = if is_window {
            Palette::Window
        } else {
            Palette::Background
        };
        GBPixel::new(palette, palette_index)
    }

    fn sprites_enabled(&self, mem: &Memory) -> bool {
        (mem.registers().lcdc & LCDC_SPRITE_ENABLE) != 0
    }

    fn sprite_height(&self, mem: &Memory) -> u8 {
        if mem.registers().lcdc & LCDC_LARGE_SPRITE == 0 {
            8
        } else {
            16
        }
    }

    fn background_map_offset(&self, mem: &Memory) -> u16 {
        if (mem.registers().lcdc & LCDC_HIGH_BG_MAP) == 0 {
            MEM_LOW_MAP
        } else {
            MEM_HIGH_MAP
        }
    }

    fn background_tile_offset(&self, mem: &Memory) -> u16 {
        if (mem.registers().lcdc & LCDC_LOW_BG_TILES) == 0 {
            MEM_HIGH_TILES
        } else {
            MEM_LOW_TILES
        }
    }

    fn window_enabled(&self, mem: &Memory) -> bool {
        (mem.registers().lcdc & LCDC_WINDOW_ENABLED) != 0
    }

    fn window_map_offset(&self, mem: &Memory) -> u16 {
        if (mem.registers().lcdc & LCDC_HIGH_WINDOW_MAP) == 0 {
            MEM_LOW_MAP
        } else {
            MEM_HIGH_MAP
        }
    }

    fn is_enabled(&self, mem: &Memory) -> bool {
        (mem.registers().lcdc & LCDC_ENABLED) != 0
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
    use alloc::boxed::Box;
    use alloc::rc::Rc;
    use alloc::vec::Vec;
    use core::cell::RefCell;

    fn create_memory() -> Memory {
        let cart = Cartridge::from_data(vec![0; 0x1000]).unwrap();
        Memory::new(cart)
    }

    fn gameboy_graphics(pixels: [u8; 8]) -> [u8; 2] {
        let mut lower_byte = 0;
        let mut upper_byte = 0;

        for pixel in pixels.iter() {
            let lower_bit = pixel & 1;
            let upper_bit = (pixel & 2) >> 1;

            lower_byte = (lower_byte << 1) | lower_bit;
            upper_byte = (upper_byte << 1) | upper_bit;
        }

        [lower_byte, upper_byte]
    }

    #[test]
    fn hblank_end_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 100;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        ppu.current_pixel = VISIBLE_WIDTH;
        memory.registers_mut().lcdstat = MODE_HBLANK;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.current_line, 101);
        assert_eq!(ppu.current_pixel, 0);
        assert_eq!(ppu.clocks_on_line, 0);
        assert_eq!(ppu.phase, PPUPhase::ObjectScan);
        assert_eq!(memory.registers().lcdstat, 0b10);
    }

    #[test]
    fn hblank_end_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 100;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        memory.registers_mut().lcdstat = MODE_HBLANK | LCDSTAT_OAM_SCAN_INTERRUPT;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers().ie).expect("No interrupt triggered");
        assert_eq!(lcd_active_interrupt, Interrupt::LCDStatus);
    }

    #[test]
    fn lyc_match_eq_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 100;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        memory.registers_mut().lyc = 101;
        memory.registers_mut().lcdstat =
            MODE_HBLANK | LCDSTAT_LINE_MATCH_INTERRUPT | LCDSTAT_MATCH_ON_EQUAL;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers().ie).expect("No interrupt triggered");
        assert_eq!(lcd_active_interrupt, Interrupt::LCDStatus);
    }

    #[test]
    fn lyc_no_match_eq_no_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 101;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        memory.registers_mut().lyc = 101;
        memory.registers_mut().lcdstat =
            MODE_HBLANK | LCDSTAT_LINE_MATCH_INTERRUPT | LCDSTAT_MATCH_ON_EQUAL;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt = Interrupt::test(0x02, memory.registers().ie);
        assert!(lcd_active_interrupt.is_none());
    }

    #[test]
    fn lyc_match_ne_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 101;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        memory.registers_mut().lyc = 101;
        memory.registers_mut().lcdstat = MODE_HBLANK | LCDSTAT_LINE_MATCH_INTERRUPT;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers().ie).expect("No interrupt triggered");
        assert_eq!(lcd_active_interrupt, Interrupt::LCDStatus);
    }

    #[test]
    fn lyc_no_match_ne_no_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = 100;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        memory.registers_mut().lyc = 101;
        memory.registers_mut().lcdstat = MODE_HBLANK | LCDSTAT_LINE_MATCH_INTERRUPT;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt = Interrupt::test(0x02, memory.registers().ie);
        assert!(lcd_active_interrupt.is_none());
    }

    #[test]
    fn hblank_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::Drawing;
        ppu.current_line = 101;
        ppu.clocks_on_line = (LINE_CYCLES - 30) * 4;
        ppu.current_pixel = VISIBLE_WIDTH;
        memory.registers_mut().lcdstat = 0b11;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.phase, PPUPhase::HBlank);
        assert_eq!(memory.registers().lcdstat & MODE_MASK, MODE_HBLANK);
    }

    #[test]
    fn hblank_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::Drawing;
        ppu.current_line = 101;
        ppu.clocks_on_line = (LINE_CYCLES - 30) * 4;
        ppu.current_pixel = VISIBLE_WIDTH;
        memory.registers_mut().lcdstat = MODE_DRAWING | LCDSTAT_HBLANK_INTERRUPT;
        ppu.update_phase(&mut memory);
        let active_interrupt =
            Interrupt::test(0x1F, memory.registers().ie).expect("No interrupt triggered");
        assert_eq!(active_interrupt, Interrupt::LCDStatus);
    }

    #[test]
    fn hblank_event() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        let expected_pixels = vec![
            GBPixel::new(Palette::Background, 0),
            GBPixel::new(Palette::Background, 1),
            GBPixel::new(Palette::Background, 2),
        ];
        let recieved_events = Rc::new(RefCell::new(Vec::new()));
        let recvd_events_clone = recieved_events.clone();
        ppu.events.on(Box::new(move |evt| {
            recvd_events_clone.borrow_mut().push(evt.clone())
        }));
        ppu.phase = PPUPhase::Drawing;
        ppu.current_line = 101;
        ppu.clocks_on_line = (LINE_CYCLES - 30) * 4;
        ppu.current_pixel = VISIBLE_WIDTH;
        ppu.pixel_queue = expected_pixels.iter().cloned().collect();
        memory.registers_mut().lcdstat = MODE_DRAWING | LCDSTAT_HBLANK_INTERRUPT;

        ppu.update_phase(&mut memory);

        let events = recieved_events.borrow();

        assert_eq!(
            *events,
            vec![PPUEvent::HBlank(HBlankEvent {
                pixels: expected_pixels
            })]
        );
    }

    #[test]
    fn vblank_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = VISIBLE_LINES - 1;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        ppu.current_pixel = VISIBLE_WIDTH;
        memory.registers_mut().lcdstat = 0b11;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.phase, PPUPhase::VBlank);
        assert_eq!(memory.registers().lcdstat & MODE_MASK, MODE_VBLANK);
    }

    #[test]
    fn vblank_end_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = TOTAL_LINES - 1;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        ppu.current_pixel = VISIBLE_WIDTH;
        memory.registers_mut().lcdstat = 0b11;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.phase, PPUPhase::ObjectScan);
        assert_eq!(memory.registers().lcdstat & MODE_MASK, MODE_OAMSCAN);
        assert_eq!(ppu.current_line, 0);
        assert_eq!(ppu.current_pixel, 0);
        assert_eq!(ppu.clocks_on_line, 0);
    }

    #[test]
    fn vblank_interrupt() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = VISIBLE_LINES - 1;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        memory.registers_mut().lcdstat = MODE_HBLANK
            | LCDSTAT_LINE_MATCH_INTERRUPT
            | LCDSTAT_OAM_SCAN_INTERRUPT
            | LCDSTAT_VBLANK_INTERRUPT
            | LCDSTAT_LINE_MATCH_INTERRUPT;
        ppu.update_phase(&mut memory);
        let lcd_active_interrupt =
            Interrupt::test(0x02, memory.registers().ie).expect("No interrupt triggered");
        assert_eq!(lcd_active_interrupt, Interrupt::LCDStatus);
        let vblank_active_interrupt =
            Interrupt::test(0x01, memory.registers().ie).expect("No interrupt triggered");
        assert_eq!(vblank_active_interrupt, Interrupt::VBlank);
    }

    #[test]
    fn vblank_event() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        let recieved_events = Rc::new(RefCell::new(Vec::new()));
        let recvd_events_clone = recieved_events.clone();
        ppu.events.on(Box::new(move |evt| {
            recvd_events_clone.borrow_mut().push(evt.clone())
        }));

        ppu.phase = PPUPhase::HBlank;
        ppu.current_line = VISIBLE_LINES - 1;
        ppu.clocks_on_line = (LINE_CYCLES * 4) - 1;
        memory.registers_mut().lcdstat = MODE_HBLANK
            | LCDSTAT_LINE_MATCH_INTERRUPT
            | LCDSTAT_OAM_SCAN_INTERRUPT
            | LCDSTAT_VBLANK_INTERRUPT
            | LCDSTAT_LINE_MATCH_INTERRUPT;

        ppu.update_phase(&mut memory);

        let events = recieved_events.borrow();

        assert_eq!(*events, vec![PPUEvent::VBlank(VBlankEvent)]);
    }

    #[test]
    fn scan_end_update_phase() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();
        ppu.phase = PPUPhase::ObjectScan;
        ppu.current_line = 100;
        ppu.clocks_on_line = (OAM_SCAN_CYCLES * 4) - 2;
        ppu.current_pixel = 0;
        memory.registers_mut().lcdstat = MODE_OAMSCAN;
        ppu.update_phase(&mut memory);
        assert_eq!(ppu.phase, PPUPhase::Drawing);
        assert_eq!(memory.registers().lcdstat & MODE_MASK, MODE_DRAWING);
        assert_eq!(ppu.current_line, 100);
        assert_eq!(ppu.current_pixel, 0);
        assert_eq!(ppu.clocks_on_line, OAM_SCAN_CYCLES * 4);
    }

    #[test]
    fn draw_phase_basic_bg() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();

        memory.registers_mut().lcdc = LCDC_ENABLED;

        let [lower, upper] = gameboy_graphics([3, 2, 1, 0, 3, 3, 3, 3]);
        memory.write_u8(MEM_HIGH_TILES + 0x10, lower).unwrap();
        memory.write_u8(MEM_HIGH_TILES + 0x11, upper).unwrap();
        memory.write_u8(MEM_LOW_MAP, 1).unwrap();

        let expected_pixels = vec![
            GBPixel::new(Palette::Background, 3),
            GBPixel::new(Palette::Background, 2),
            GBPixel::new(Palette::Background, 1),
            GBPixel::new(Palette::Background, 0),
            GBPixel::new(Palette::Background, 3),
            GBPixel::new(Palette::Background, 3),
            GBPixel::new(Palette::Background, 3),
            GBPixel::new(Palette::Background, 3),
        ];

        for _ in 0..8 {
            ppu.draw(&memory);
        }

        assert_eq!(
            expected_pixels,
            ppu.pixel_queue.drain(..).collect::<Vec<GBPixel>>()
        );
        assert_eq!(expected_pixels, Vec::from(&ppu.framebuffer[0..8]));
    }

    #[test]
    fn draw_phase_bg_low_tiles_no_window() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();

        memory.registers_mut().lcdc = LCDC_ENABLED | LCDC_LOW_BG_TILES;
        memory.write_u8(MEM_LOW_TILES + 0x10, 0xFF).unwrap();
        memory.write_u8(MEM_LOW_TILES + 0x11, 0xFF).unwrap();
        memory.write_u8(MEM_LOW_MAP, 1).unwrap();

        ppu.draw(&memory);

        assert_eq!(GBPixel::new(Palette::Background, 3), ppu.pixel_queue[0]);
        assert_eq!(GBPixel::new(Palette::Background, 3), ppu.framebuffer[0]);
    }

    #[test]
    fn draw_phase_bg_high_map_low_tiles_no_window() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();

        memory.registers_mut().lcdc = LCDC_ENABLED | LCDC_LOW_BG_TILES | LCDC_HIGH_BG_MAP;
        memory.write_u8(MEM_LOW_TILES + 0x10, 0xFF).unwrap();
        memory.write_u8(MEM_LOW_TILES + 0x11, 0xFF).unwrap();
        memory.write_u8(MEM_HIGH_MAP, 1).unwrap();

        ppu.draw(&memory);

        assert_eq!(GBPixel::new(Palette::Background, 3), ppu.pixel_queue[0]);
        assert_eq!(GBPixel::new(Palette::Background, 3), ppu.framebuffer[0]);
    }

    #[test]
    fn draw_phase_window_transition() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();

        memory.registers_mut().lcdc = LCDC_ENABLED | LCDC_WINDOW_ENABLED | LCDC_HIGH_BG_MAP;
        memory.registers_mut().wx = 4;
        memory.registers_mut().wy = 0;
        let [t1_lower, t1_upper] = gameboy_graphics([3, 2, 1, 0, 3, 3, 3, 3]);
        let [t2_lower, t2_upper] = gameboy_graphics([0, 0, 0, 0, 1, 2, 1, 2]);
        memory.write_u8(MEM_HIGH_TILES + 0x10, t1_lower).unwrap();
        memory.write_u8(MEM_HIGH_TILES + 0x11, t1_upper).unwrap();
        memory.write_u8(MEM_HIGH_TILES + 0x20, t2_lower).unwrap();
        memory.write_u8(MEM_HIGH_TILES + 0x21, t2_upper).unwrap();
        memory.write_u8(MEM_HIGH_MAP, 1).unwrap();
        memory.write_u8(MEM_LOW_MAP, 2).unwrap();

        let expected_pixels = vec![
            GBPixel::new(Palette::Background, 3),
            GBPixel::new(Palette::Background, 2),
            GBPixel::new(Palette::Background, 1),
            GBPixel::new(Palette::Background, 0),
            GBPixel::new(Palette::Window, 1),
            GBPixel::new(Palette::Window, 2),
            GBPixel::new(Palette::Window, 1),
            GBPixel::new(Palette::Window, 2),
        ];

        for _ in 0..8 {
            ppu.draw(&memory);
        }

        assert_eq!(
            expected_pixels,
            ppu.pixel_queue.drain(..).collect::<Vec<GBPixel>>()
        );
        assert_eq!(expected_pixels, Vec::from(&ppu.framebuffer[0..8]));
    }

    #[test]
    fn draw_phase_window_transition_window_high() {
        let mut ppu = PPU::new();
        let mut memory = create_memory();

        memory.registers_mut().lcdc = LCDC_ENABLED | LCDC_WINDOW_ENABLED | LCDC_HIGH_WINDOW_MAP;
        memory.registers_mut().wx = 4;
        memory.registers_mut().wy = 0;
        let [t1_lower, t1_upper] = gameboy_graphics([3, 2, 1, 0, 3, 3, 3, 3]);
        let [t2_lower, t2_upper] = gameboy_graphics([0, 0, 0, 0, 1, 2, 1, 2]);
        memory.write_u8(MEM_HIGH_TILES + 0x10, t1_lower).unwrap();
        memory.write_u8(MEM_HIGH_TILES + 0x11, t1_upper).unwrap();
        memory.write_u8(MEM_HIGH_TILES + 0x20, t2_lower).unwrap();
        memory.write_u8(MEM_HIGH_TILES + 0x21, t2_upper).unwrap();
        memory.write_u8(MEM_LOW_MAP, 1).unwrap();
        memory.write_u8(MEM_HIGH_MAP, 2).unwrap();

        let expected_pixels = vec![
            GBPixel::new(Palette::Background, 3),
            GBPixel::new(Palette::Background, 2),
            GBPixel::new(Palette::Background, 1),
            GBPixel::new(Palette::Background, 0),
            GBPixel::new(Palette::Window, 1),
            GBPixel::new(Palette::Window, 2),
            GBPixel::new(Palette::Window, 1),
            GBPixel::new(Palette::Window, 2),
        ];

        for _ in 0..8 {
            ppu.draw(&memory);
        }

        assert_eq!(
            expected_pixels,
            ppu.pixel_queue.drain(..).collect::<Vec<GBPixel>>()
        );
        assert_eq!(expected_pixels, Vec::from(&ppu.framebuffer[0..8]));
    }
}
