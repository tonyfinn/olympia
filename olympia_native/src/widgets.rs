mod address_picker;
mod breakpoint_viewer;
mod disassembly_viewer;
mod emulator_display;
mod memory_viewer;
mod playback_controls;
mod register_labels;
mod tileset_viewer;

pub(crate) use address_picker::AddressPicker;
pub(crate) use breakpoint_viewer::BreakpointViewer;
pub(crate) use disassembly_viewer::Disassembler;
pub(crate) use emulator_display::EmulatorDisplay;
pub(crate) use memory_viewer::MemoryViewer;
pub(crate) use playback_controls::PlaybackControls;
pub(crate) use register_labels::RegisterLabels;
pub(crate) use tileset_viewer::TilesetViewer;

use gtk::prelude::StaticType;

pub fn register() {
    AddressPicker::static_type();
    Disassembler::static_type();
    TilesetViewer::static_type();
}
