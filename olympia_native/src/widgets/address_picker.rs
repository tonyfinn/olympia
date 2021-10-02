use olympia_engine::{monitor::parse_number, registers::WordRegister};

use gtk::glib::{
    self, clone,
    once_cell::sync::Lazy,
    prelude::*,
    subclass::{prelude::*, InitializingObject, Signal},
    wrapper as glib_wrapper, ParamFlags, ParamSpec,
};

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use std::{
    cell::RefCell,
    sync::atomic::{AtomicU16, Ordering},
};

use crate::utils::EmulatorHandle;

#[derive(CompositeTemplate, Default)]
#[template(file = "../../res/address_picker.ui")]
pub struct AddressPickerInternal {
    #[template_child]
    pub(crate) address_entry: TemplateChild<gtk::Entry>,
    #[template_child]
    pub(crate) pc_button: TemplateChild<gtk::Button>,
    #[template_child]
    pub(crate) go_button: TemplateChild<gtk::Button>,
    emu: RefCell<Option<EmulatorHandle>>,
    address_selected: AtomicU16,
}

#[glib::object_subclass]
impl ObjectSubclass for AddressPickerInternal {
    const NAME: &'static str = "OlympiaAddressPicker";
    type ParentType = gtk::Box;
    type Type = AddressPicker;

    fn class_init(klass: &mut Self::Class) {
        Self::bind_template(klass);
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

glib_wrapper! {
    pub struct AddressPicker(ObjectSubclass<AddressPickerInternal>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Buildable, gtk::Orientable;
}

impl AddressPicker {
    async fn set_target_to_pc(self) {
        let emu: EmulatorHandle = self
            .property(EMU_PROPERTY)
            .expect("Invalid emulator property name")
            .get()
            .expect("No emulator adapter attached to address picker");
        let result = emu.query_registers().await;
        if let Ok(registers) = result {
            let pc_value = registers.read_u16(WordRegister::PC);
            self.set_property(ADDRESS_PROPERTY, format!("0x{:04X}", pc_value))
                .expect("Invalid address property name");
        }
    }

    pub fn connect_goto<F>(&self, f: F)
    where
        F: Fn(u16) -> () + 'static,
    {
        self.connect_local(GOTO_ADDRESS_SIGNAL, false, move |v| {
            let unwrapped: u32 = v[1].get().expect("Wrong type sent for goto signal");
            let right_sized = unwrapped as u16; // No GValue exists for u16
            f(right_sized);
            None
        })
        .unwrap();
    }
}

pub const ADDRESS_PROPERTY: &'static str = "address";
pub const EMU_PROPERTY: &'static str = "emu";
pub const PC_CLICKED_SIGNAL: &'static str = "pc-button-clicked";
pub const GOTO_ADDRESS_SIGNAL: &'static str = "goto-address";

impl ObjectImpl for AddressPickerInternal {
    fn properties() -> &'static [ParamSpec] {
        static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
            vec![
                ParamSpec::new_string(
                    ADDRESS_PROPERTY,
                    ADDRESS_PROPERTY,
                    ADDRESS_PROPERTY,
                    None,
                    ParamFlags::READWRITE,
                ),
                ParamSpec::new_boxed(
                    EMU_PROPERTY,
                    EMU_PROPERTY,
                    EMU_PROPERTY,
                    EmulatorHandle::static_type(),
                    ParamFlags::READWRITE,
                ),
            ]
        });
        PROPERTIES.as_ref()
    }

    fn signals() -> &'static [Signal] {
        static SIGNALS: Lazy<Vec<Signal>> = Lazy::new(|| {
            vec![
                Signal::builder(
                    // Signal name
                    PC_CLICKED_SIGNAL,
                    // Types of the values which will be sent to the signal handler
                    &[],
                    // Type of the value the signal handler sends back
                    <()>::static_type().into(),
                )
                .build(),
                Signal::builder(
                    // Signal name
                    GOTO_ADDRESS_SIGNAL,
                    // Types of the values which will be sent to the signal handler
                    &[u32::static_type().into()],
                    // Type of the value the signal handler sends back
                    <()>::static_type().into(),
                )
                .build(),
            ]
        });
        SIGNALS.as_ref()
    }

    fn constructed(&self, obj: &Self::Type) {
        // Call "constructed" on parent
        self.parent_constructed(obj);

        self.address_entry.set_text("0x100");
        self.address_selected.store(0x100, Ordering::Relaxed);

        self.address_entry
            .get()
            .connect_changed(clone!(@strong obj => move |_| {
                let widget = Self::from_instance(&obj);
                let text = widget.address_entry.text();

                if let Ok(addr) = parse_number(&text) {
                    widget.address_selected.store(addr, Ordering::Relaxed);
                    obj.notify(ADDRESS_PROPERTY);
                    widget.go_button.get().set_sensitive(true);
                } else {
                    widget.go_button.get().set_sensitive(false);
                }
            }));

        self.pc_button
            .connect_clicked(clone!(@strong obj => @default-return (), move |_| {
                glib::MainContext::ref_thread_default().spawn_local(obj.clone().set_target_to_pc());
            }));
        self.address_entry
            .connect_activate(clone!(@strong obj => @default-return (), move |_| {
                let address = Self::from_instance(&obj).address_selected.load(Ordering::Relaxed);
                obj.emit_by_name_with_values(GOTO_ADDRESS_SIGNAL, &[u32::from(address).to_value()]).unwrap();
            }));
        self.go_button
            .connect_clicked(clone!(@strong obj => @default-return (), move |_| {
                let address = Self::from_instance(&obj).address_selected.load(Ordering::Relaxed);
                obj.emit_by_name_with_values(GOTO_ADDRESS_SIGNAL, &[u32::from(address).to_value()]).unwrap();
            }));
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
            ADDRESS_PROPERTY => {
                let address: &str = value
                    .get()
                    .expect("type conformity checked by `Object::set_property`");
                if let Ok(numeric) = parse_number(address) {
                    self.address_selected.store(numeric, Ordering::Relaxed);
                    self.go_button.get().set_sensitive(true);
                } else {
                    self.go_button.get().set_sensitive(false);
                }
                self.address_entry.set_text(address);
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
            ADDRESS_PROPERTY => u32::from(self.address_selected.load(Ordering::Relaxed)).to_value(),
            _ => unimplemented!(),
        }
    }
}

impl WidgetImpl for AddressPickerInternal {}

impl ContainerImpl for AddressPickerInternal {}

impl BoxImpl for AddressPickerInternal {}
