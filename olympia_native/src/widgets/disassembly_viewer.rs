use super::address_picker;
use crate::utils::EmulatorHandle;
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

#[glib::object_subclass]
impl ObjectSubclass for DisassemblerInternal {
    const NAME: &'static str = "OlympiaDisassembler";
    type ParentType = gtk::Box;
    type Type = Disassembler;

    fn class_init(klass: &mut Self::Class) {
        Self::bind_template(klass);
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

const EMU_PROPERTY: &'static str = "emu";

impl ObjectImpl for DisassemblerInternal {
    fn constructed(&self, obj: &Self::Type) {
        // Call "constructed" on parent
        self.parent_constructed(obj);
        obj.bind_property(
            EMU_PROPERTY,
            &*self.address_picker,
            address_picker::EMU_PROPERTY,
        )
        .build();

        self.text_view.set_monospace(true);

        let obj = obj.clone();
        self.address_picker
            .connect_goto(move |addr| obj.goto_address(addr));
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| {
            vec![glib::ParamSpec::new_boxed(
                EMU_PROPERTY,
                EMU_PROPERTY,
                EMU_PROPERTY,
                EmulatorHandle::static_type(),
                glib::ParamFlags::READWRITE,
            )]
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
                let emu = value
                    .get()
                    .expect("type conformity checked by `Object::set_property`");
                self.emu.replace(Some(emu));
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
    pub fn attach_emu(&self, emu: EmulatorHandle) {
        self.set_property(EMU_PROPERTY, emu).unwrap();
    }

    pub fn goto_address(&self, address: u16) {
        glib::MainContext::ref_thread_default()
            .spawn_local(self.clone().goto_address_internal(address));
    }

    async fn goto_address_internal(self, address: u16) {
        let emu: EmulatorHandle = self
            .property(EMU_PROPERTY)
            .expect("Invalid emulator property name")
            .get()
            .expect("No emulator adapter attached to disassembler");

        let query_response = emu.query_memory(address, address + 600).await;

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
