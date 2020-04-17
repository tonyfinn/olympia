use crate::builder_struct;
use crate::emulator::remote::RemoteEmulator;

use glib::clone;
use gtk::prelude::*;
use olympia_engine::registers::WordRegister;
use std::rc::Rc;

builder_struct!(
    pub struct RegisterLabelsWidget {
        #[ogtk(id = "AFInput")]
        af_input: gtk::Entry,
        #[ogtk(id = "BCInput")]
        bc_input: gtk::Entry,
        #[ogtk(id = "DEInput")]
        de_input: gtk::Entry,
        #[ogtk(id = "SPInput")]
        sp_input: gtk::Entry,
        #[ogtk(id = "HLInput")]
        hl_input: gtk::Entry,
        #[ogtk(id = "PCInput")]
        pc_input: gtk::Entry,
    }
);

pub(crate) struct RegisterLabels {
    widget: RegisterLabelsWidget,
    emu: Rc<RemoteEmulator>,
}

impl RegisterLabels {
    pub(crate) fn from_widget(
        widget: RegisterLabelsWidget,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<RegisterLabels> {
        let labels = Rc::new(RegisterLabels { widget, emu });

        labels.connect_adapter_events();
        labels
    }

    pub(crate) fn from_builder(
        builder: &gtk::Builder,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<RegisterLabels> {
        let widget = RegisterLabelsWidget::from_builder(builder).unwrap();
        RegisterLabels::from_widget(widget, emu)
    }

    fn connect_adapter_events(self: &Rc<Self>) {
        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.emu.on_step(tx);
        let context = glib::MainContext::default();
        rx.attach(
            Some(&context),
            clone!(@strong self as labels, @strong context => move |_| {
                context.spawn_local(labels.clone().update());
                glib::Continue(true)
            }),
        );
    }

    fn inputs(&self) -> [&gtk::Entry; 6] {
        [
            &self.widget.af_input,
            &self.widget.bc_input,
            &self.widget.de_input,
            &self.widget.sp_input,
            &self.widget.hl_input,
            &self.widget.pc_input,
        ]
    }

    fn register_inputs(&self) -> [(&gtk::Entry, WordRegister); 6] {
        [
            (&self.widget.af_input, WordRegister::AF),
            (&self.widget.bc_input, WordRegister::BC),
            (&self.widget.de_input, WordRegister::DE),
            (&self.widget.sp_input, WordRegister::SP),
            (&self.widget.hl_input, WordRegister::HL),
            (&self.widget.pc_input, WordRegister::PC),
        ]
    }

    fn set_editable(&self, editable: bool) {
        for input in self.inputs().iter_mut() {
            input.set_editable(editable);
        }
    }

    async fn update(self: Rc<Self>) -> () {
        let register_result = self.emu.query_registers().await;
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

#[cfg(test)]
mod tests {
    #[test]
    fn query_adapters() {}
}
