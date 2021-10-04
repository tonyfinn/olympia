use super::common::EMU_PROPERTY;
use crate::subclass_widget;
use crate::utils::{EmulatorHandle, GValueExt};
use crate::widgets::common::{emu_param_spec, EmulatorWidget};
use crate::widgets::AddressPicker;

use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::{
    glib::{self, once_cell::sync::Lazy, subclass::InitializingObject},
    prelude::*,
    TextBufferBuilder,
};
use olympia_engine::disassembler::{DisassemblyFormat, DisassemblyIterator};
use std::cell::RefCell;

#[derive(CompositeTemplate, Default)]
#[template(file = "../../res/disassembly.ui")]
pub struct DisassemblerInternal {
    #[template_child(id = "DisassemblyTextView")]
    text_view: TemplateChild<gtk::TextView>,
    #[template_child(id = "DisassemblyAddressPicker")]
    address_picker: TemplateChild<AddressPicker>,
    emu: RefCell<Option<EmulatorHandle>>,
}

subclass_widget!(DisassemblerInternal, gtk::Box, Disassembler);

impl ObjectImpl for DisassemblerInternal {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.bind_property(EMU_PROPERTY, &*self.address_picker, EMU_PROPERTY)
            .build();

        self.text_view.set_monospace(true);

        let obj = obj.clone();
        self.address_picker
            .connect_goto(move |addr| obj.goto_address(addr));
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| vec![emu_param_spec()]);
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
            _ => unimplemented!(),
        }
    }
}

impl WidgetImpl for DisassemblerInternal {}

impl ContainerImpl for DisassemblerInternal {}

impl BoxImpl for DisassemblerInternal {}

glib::wrapper! {
    pub struct Disassembler(ObjectSubclass<DisassemblerInternal>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Buildable, gtk::Orientable;
}

impl Disassembler {
    pub fn goto_address(&self, address: u16) {
        glib::MainContext::ref_thread_default()
            .spawn_local(self.clone().goto_address_internal(address));
    }

    async fn goto_address_internal(self, address: u16) {
        let emu = self.emu_handle();

        let query_response = emu.query_memory(address, address.saturating_add(600)).await;

        if let Ok(memory_region) = query_response {
            let data = memory_region.data.iter().map(|b| b.unwrap_or(0));
            let disasm_iter =
                DisassemblyIterator::new(data, DisassemblyFormat::Columnar, address.into());
            let lines: Vec<String> = disasm_iter.take(200).collect();
            let disasm: String = lines.join("\n");
            let buffer = TextBufferBuilder::new().text(&disasm).build();
            let tv: gtk::TextView = DisassemblerInternal::from_instance(&self).text_view.get();
            tv.set_buffer(Some(&buffer));
        }
    }
}

impl EmulatorWidget for Disassembler {}
