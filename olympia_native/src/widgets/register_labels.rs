use crate::emulator::EmulatorAdapter;

use glib::clone;
use gtk::prelude::*;
use olympia_engine::registers::WordRegister;
use std::rc::Rc;

pub(crate) struct RegisterLabels {
    af_input: gtk::Entry,
    bc_input: gtk::Entry,
    de_input: gtk::Entry,
    sp_input: gtk::Entry,
    hl_input: gtk::Entry,
    pc_input: gtk::Entry,
    adapter: Rc<EmulatorAdapter>,
}

impl RegisterLabels {
    pub(crate) fn from_builder(builder: &gtk::Builder, adapter: Rc<EmulatorAdapter>) -> Rc<RegisterLabels> {
        let labels = Rc::new(RegisterLabels {
            af_input: builder.get_object("AFInput").unwrap(),
            bc_input: builder.get_object("BCInput").unwrap(),
            de_input: builder.get_object("DEInput").unwrap(),
            sp_input: builder.get_object("SPInput").unwrap(),
            hl_input: builder.get_object("HLInput").unwrap(),
            pc_input: builder.get_object("PCInput").unwrap(),
            adapter,
        });
        
        labels.connect_adapter_events();
        labels
    }

    fn connect_adapter_events(self: &Rc<Self>) {
        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.adapter.on_step(tx);
        let context = glib::MainContext::default();
        rx.attach(Some(&context), clone!(@strong self as labels, @strong context => move |_| {
            context.spawn_local(labels.clone().update());
            glib::Continue(true)
        }));
    }

    fn inputs(&self) -> [&gtk::Entry; 6] {
        [
            &self.af_input,
            &self.bc_input,
            &self.de_input,
            &self.sp_input,
            &self.hl_input,
            &self.pc_input,
        ]
    }

    fn register_inputs(&self) -> [(&gtk::Entry, WordRegister); 6] {
        [
            (&self.af_input, WordRegister::AF),
            (&self.bc_input, WordRegister::BC),
            (&self.de_input, WordRegister::DE),
            (&self.sp_input, WordRegister::SP),
            (&self.hl_input, WordRegister::HL),
            (&self.pc_input, WordRegister::PC),
        ]
    }

    fn set_editable(&self, editable: bool) {
        for input in self.inputs().iter_mut() {
            input.set_editable(editable);
        }
    }

    async fn update(self: Rc<Self>) -> () {
        let register_result = self.adapter.query_registers().await;
        match register_result {
            Ok(registers) => {
                self.set_editable(true);
                for (input, register) in self.register_inputs().iter_mut() {
                    let value = registers.read_u16(*register);
                    input.set_text(&format!("{:04X}", value));
                }
            }
            Err(_) => self.set_editable(false),
        }
    }
}