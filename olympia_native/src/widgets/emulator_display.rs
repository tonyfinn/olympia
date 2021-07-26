use crate::builder_struct;

use glib::clone;
use gtk::prelude::*;
use gtk::WidgetExt;
use log::trace;
use olympia_engine::{
    events::{HBlankEvent, VBlankEvent},
    gameboy::GBPixel,
    remote::RemoteEmulator,
};
use std::cell::RefCell;
use std::rc::Rc;

pub const HEIGHT: usize = 144;
pub const WIDTH: usize = 160;
pub const INITIAL_SCALE: usize = 2;
pub const BPP: usize = 4;

struct Buffer {
    front: Vec<GBPixel>,
    front_pixels: Vec<u8>,
    back: Vec<GBPixel>,
    back_pixels: Vec<u8>,
    image_surface: Option<cairo::ImageSurface>,
    scale: usize,
    width: usize,
    height: usize,
}

const COLORS: [(u8, u8, u8); 4] = [(255, 255, 255), (176, 176, 176), (128, 128, 128), (0, 0, 0)];

impl Buffer {
    fn new(width: usize, height: usize, scale: usize) -> Buffer {
        let px_width = width * scale;
        let px_height = height * scale;
        Buffer {
            front: vec![GBPixel::default(); width * height],
            front_pixels: vec![0; BPP * px_width * px_height],
            back: vec![GBPixel::default(); width * height],
            back_pixels: vec![0; BPP * px_width * px_height],
            image_surface: None,
            scale,
            width,
            height,
        }
    }

    fn draw_pixel(&mut self, gb_x: usize, gb_y: usize, pixel: &GBPixel) {
        let color = COLORS[usize::from(pixel.index)];
        if gb_x >= WIDTH {
            panic!("X co-ord too large {}", gb_x);
        }
        if gb_y >= HEIGHT {
            panic!("Y co-ord too large {}", gb_y);
        }
        let render_x_start = gb_x * self.scale;
        let render_y_start = gb_y * self.scale;
        for x_subpx in 0..self.scale {
            for y_subpx in 0..self.scale {
                let render_x_px = render_x_start + x_subpx;
                let render_y_px = render_y_start + y_subpx;
                let row_width = self.width * BPP * self.scale;
                let idx = (render_y_px * row_width) + (render_x_px * BPP);
                self.back_pixels[idx + 0] = color.0;
                self.back_pixels[idx + 1] = color.1;
                self.back_pixels[idx + 2] = color.2;
            }
        }
    }

    fn build_image_surface(&self) -> Result<cairo::ImageSurface, cairo::Error> {
        let result = cairo::ImageSurface::create_for_data(
            self.front_pixels.clone(),
            cairo::Format::Rgb24,
            (self.width * self.scale) as i32,
            (self.height * self.scale) as i32,
            cairo::Format::Rgb24
                .stride_for_width((self.width * self.scale) as u32)
                .unwrap(),
        );
        if let Err(e) = result {
            eprintln!("Image surface build error: {}", e);
        }

        result
    }

    fn hblank(&mut self, evt: HBlankEvent) {
        for (x, pixel) in evt.pixels.iter().enumerate() {
            self.draw_pixel(x, evt.current_line.into(), pixel);
        }
    }

    fn vblank(&mut self) {
        std::mem::swap(&mut self.front, &mut self.back);
        std::mem::swap(&mut self.front_pixels, &mut self.back_pixels);
        trace!("Renderer VBLANK");
        self.image_surface = self.build_image_surface().ok();
    }
}

builder_struct!(
    pub struct EmulatorDisplayWidget {
        #[ogtk(id = "EmulatorView")]
        drawing_area: gtk::DrawingArea,
    }
);

pub struct EmulatorDisplay {
    #[allow(dead_code)]
    context: glib::MainContext,
    emu: Rc<RemoteEmulator>,
    widget: EmulatorDisplayWidget,
    buffer: Rc<RefCell<Buffer>>,
}

impl EmulatorDisplay {
    pub(crate) fn from_widget(
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
        widget: EmulatorDisplayWidget,
    ) -> Rc<EmulatorDisplay> {
        let display = Rc::new(EmulatorDisplay {
            context,
            emu,
            widget,
            buffer: Rc::new(RefCell::new(Buffer::new(WIDTH, HEIGHT, INITIAL_SCALE))),
        });
        display.connect_ppu_events();
        display.connect_ui_events();
        display
    }

    pub(crate) fn connect_ui_events(self: &Rc<Self>) {
        self.widget.drawing_area.connect_draw(clone!(@weak self as display => @default-return Inhibit(false), move |_drawing_area, cr| {
            if let Some(ref surface) = display.buffer.borrow().image_surface {
                cr.set_source_surface(surface, 0.0, 0.0);
                cr.paint();
            }
            Inhibit(false)
        }));
    }

    pub(crate) fn hblank(&self, evt: HBlankEvent) {
        self.buffer.borrow_mut().hblank(evt)
    }

    pub(crate) fn vblank(&self) {
        self.buffer.borrow_mut().vblank();
        let scale = self.buffer.borrow().scale;
        self.widget.drawing_area.queue_draw_area(
            0,
            0,
            (scale * WIDTH) as i32,
            (scale * HEIGHT) as i32,
        );
    }

    pub(crate) fn connect_ppu_events(self: &Rc<Self>) {
        self.emu.on_widget(self.clone(), |display, _: VBlankEvent| {
            display.vblank();
        });
        self.emu
            .on_widget(self.clone(), |display, evt: HBlankEvent| {
                display.hblank(evt);
            });
    }

    pub(crate) fn from_builder(
        builder: &gtk::Builder,
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<EmulatorDisplay> {
        let widget = EmulatorDisplayWidget::from_builder(builder).unwrap();
        EmulatorDisplay::from_widget(context, emu, widget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use olympia_engine::gameboy::Palette;

    fn bg_pixel(index: u8) -> GBPixel {
        GBPixel::new(Palette::Background, index)
    }

    fn pixel_data_at(
        surface: &mut cairo::ImageSurface,
        x: i32,
        y: i32,
    ) -> Result<Vec<u8>, cairo::BorrowError> {
        let stride = surface.get_stride();
        let bpp = surface.get_format().stride_for_width(1).unwrap() as usize;
        let idx = ((y * stride) + (x * (bpp as i32))) as usize;
        surface
            .get_data()
            .map(|data| Vec::from(&data[idx..idx + bpp]))
    }

    #[test]
    fn test_render_buffer() {
        let mut buffer = Buffer::new(4, 4, 2);
        buffer.hblank(HBlankEvent {
            pixels: vec![bg_pixel(2), bg_pixel(1), bg_pixel(1), bg_pixel(0)],
            current_line: 0,
        });
        buffer.hblank(HBlankEvent {
            pixels: vec![bg_pixel(3), bg_pixel(0), bg_pixel(3), bg_pixel(0)],
            current_line: 0,
        });
        buffer.vblank();

        let expected_colors: Vec<Vec<usize>> = vec![
            vec![2, 2, 1, 1, 1, 1, 0, 0],
            vec![2, 2, 1, 1, 1, 1, 0, 0],
            vec![3, 3, 0, 0, 3, 3, 0, 0],
            vec![3, 3, 0, 0, 3, 3, 0, 0],
        ];

        for (y, row) in expected_colors.iter().enumerate() {
            for (x, color_index) in row.iter().enumerate() {
                let surface = buffer.image_surface.as_mut().unwrap();
                let actual_subpixels = pixel_data_at(surface, x as i32, y as i32).unwrap();
                let (r, g, b) = COLORS[*color_index];
                let expected_subpixels = vec![r, g, b, 0];
                assert_eq!(
                    actual_subpixels, expected_subpixels,
                    "Unexpected pixels at ({}, {}). Found {:?}, expected {:?}",
                    x, y, actual_subpixels, expected_subpixels
                );
            }
        }
    }
}
