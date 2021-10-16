use crate::subclass_widget;
use crate::utils::{EmulatorHandle, GValueExt};
use crate::widgets::common::emu_param_spec;

use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{
    glib::{self, once_cell::sync::Lazy, subclass::InitializingObject},
    prelude::*,
};
use olympia_engine::gameboy::{GBPixel, Palette, VRAM};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use super::common::{EmulatorWidget, EMU_PROPERTY};
use super::emulator_display::GBDisplayBuffer;

pub const SPRITES_PER_LINE: usize = 16;
pub const TOTAL_SPRITES: usize = 384;
pub const SPRITE_ROW_COUNT: usize = TOTAL_SPRITES / SPRITES_PER_LINE;
pub const SPRITE_WIDTH: usize = 8;
pub const SPRITE_SCALE: usize = 2;

#[derive(CompositeTemplate)]
#[template(file = "../../res/tileset_viewer.ui")]
pub struct TilesetViewerInternal {
    #[template_child]
    large_sprites_check: TemplateChild<gtk::CheckButton>,
    #[template_child]
    drawing_area: TemplateChild<gtk::DrawingArea>,
    #[template_child]
    refresh_button: TemplateChild<gtk::Button>,
    buffer: RefCell<GBDisplayBuffer>,
    large_sprites_enabled: AtomicBool,
    emu: RefCell<Option<EmulatorHandle>>,
}

impl Default for TilesetViewerInternal {
    fn default() -> TilesetViewerInternal {
        TilesetViewerInternal {
            buffer: RefCell::new(GBDisplayBuffer::new(
                SPRITES_PER_LINE * SPRITE_WIDTH,
                (TOTAL_SPRITES / SPRITES_PER_LINE) * SPRITE_WIDTH,
                SPRITE_SCALE,
            )),
            large_sprites_check: Default::default(),
            drawing_area: Default::default(),
            emu: Default::default(),
            large_sprites_enabled: Default::default(),
            refresh_button: Default::default(),
        }
    }
}

subclass_widget!(TilesetViewerInternal, gtk::Box, TilesetViewer);

const LARGE_SPRITES_PROPERTY: &str = "large-sprites";

impl ObjectImpl for TilesetViewerInternal {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);

        self.drawing_area.connect_draw(
            glib::clone!(@weak obj => @default-return Inhibit(false), move |_drawing_area, cr| {
                let buffer = Self::from_instance(&obj).buffer.borrow();
                buffer.render_to_context(cr);
                Inhibit(false)
            }),
        );

        self.refresh_button
            .connect_clicked(glib::clone!(@strong obj => move |_| {
                obj.render();
            }));
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![
                emu_param_spec(),
                glib::ParamSpec::new_boolean(
                    LARGE_SPRITES_PROPERTY,
                    LARGE_SPRITES_PROPERTY,
                    LARGE_SPRITES_PROPERTY,
                    false,
                    glib::ParamFlags::READWRITE,
                ),
            ]
        });
        PROPERTIES.as_ref()
    }

    fn set_property(
        &self,
        _obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        match pspec.name() {
            EMU_PROPERTY => {
                self.emu.replace(Some(value.unwrap()));
            }
            LARGE_SPRITES_PROPERTY => {
                self.large_sprites_enabled
                    .store(value.unwrap(), Ordering::Relaxed);
            }
            _ => unimplemented!(),
        }
    }

    // Called whenever a property is retrieved from this instance. The id
    // is the same as the index of the property in the PROPERTIES array.
    fn property(&self, _obj: &Self::Type, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            EMU_PROPERTY => match self.emu.borrow().as_ref() {
                Some(emu) => emu.clone().to_value(),
                None => panic!("No connected emulator"),
            },
            LARGE_SPRITES_PROPERTY => self
                .large_sprites_enabled
                .load(Ordering::Relaxed)
                .to_value(),
            _ => unimplemented!(),
        }
    }
}

impl WidgetImpl for TilesetViewerInternal {}

impl ContainerImpl for TilesetViewerInternal {}

impl BoxImpl for TilesetViewerInternal {}

glib::wrapper! {
    pub struct TilesetViewer(ObjectSubclass<TilesetViewerInternal>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Buildable, gtk::Orientable;
}

impl TilesetViewer {
    pub fn render(&self) {
        glib::MainContext::ref_thread_default().spawn_local(self.clone().render_internal());
    }

    pub async fn render_internal(self) {
        let internal = TilesetViewerInternal::from_instance(&self);
        let emu = match internal.emu.borrow().clone() {
            Some(emu) => emu,
            None => return,
        };

        let mem = match emu.query_memory(VRAM.start, VRAM.last).await {
            Ok(mem) => mem,
            Err(_) => return,
        };
        for i in 0..TOTAL_SPRITES {
            self.render_sprite(i, &mem.data, &mut internal.buffer.borrow_mut());
        }
        internal.buffer.borrow_mut().swap_buffers();
        let width = SPRITE_SCALE * SPRITE_WIDTH * SPRITES_PER_LINE;
        let height = SPRITE_SCALE * SPRITE_WIDTH * SPRITE_ROW_COUNT;
        internal
            .drawing_area
            .queue_draw_area(0, 0, width as i32, height as i32);
    }

    pub(crate) fn render_sprite(
        &self,
        index: usize,
        data: &[Option<u8>],
        buffer: &mut GBDisplayBuffer,
    ) {
        let sprite_base_x = (index % SPRITES_PER_LINE) * SPRITE_WIDTH;
        let sprite_base_y = (index / SPRITES_PER_LINE) * SPRITE_WIDTH;
        for x in 0..SPRITE_WIDTH {
            for y in 0..SPRITE_WIDTH {
                let palette_index = Self::read_pixel_palette_index(index, data, x, y);
                let pixel = GBPixel::new(Palette::Background, palette_index);
                buffer.draw_pixel(sprite_base_x + x, sprite_base_y + y, &pixel);
            }
        }
    }

    pub fn read_pixel_palette_index(
        tile_index: usize,
        data: &[Option<u8>],
        x: usize,
        y: usize,
    ) -> u8 {
        let lower_addr = (tile_index * 0x10) + (y * 2);

        let lower_byte = data.get(lower_addr).copied().flatten().unwrap_or(0);
        let upper_byte = data.get(lower_addr + 1).copied().flatten().unwrap_or(0);

        let upper_byte_value = (upper_byte >> (7 - x)) & 1;
        let lower_byte_value = (lower_byte >> (7 - x)) & 1;

        lower_byte_value | (upper_byte_value << 1)
    }
}

impl EmulatorWidget for TilesetViewer {}
