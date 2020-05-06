use crate::builder_struct;
use crate::emulator::commands::QueryRegistersResponse;
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

impl Default for RegisterLabelsWidget {
    fn default() -> Self {
        RegisterLabelsWidget {
            af_input: Default::default(),
            bc_input: Default::default(),
            de_input: Default::default(),
            hl_input: Default::default(),
            pc_input: Default::default(),
            sp_input: Default::default(),
        }
    }
}

pub(crate) struct RegisterLabels {
    context: glib::MainContext,
    emu: Rc<RemoteEmulator>,
    widget: RegisterLabelsWidget,
}

impl RegisterLabels {
    pub(crate) fn from_widget(
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
        widget: RegisterLabelsWidget,
    ) -> Rc<RegisterLabels> {
        let labels = Rc::new(RegisterLabels {
            context,
            emu,
            widget,
        });

        labels.connect_adapter_events();
        labels
    }

    pub(crate) fn from_builder(
        builder: &gtk::Builder,
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<RegisterLabels> {
        let widget = RegisterLabelsWidget::from_builder(builder).unwrap();
        RegisterLabels::from_widget(context, emu, widget)
    }

    fn connect_adapter_events(self: &Rc<Self>) {
        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.emu.on_step(tx);
        rx.attach(
            Some(&self.context),
            clone!(@weak self as labels, @strong self.context as context => @default-return glib::Continue(false), move |_| {
                context.spawn_local(labels.clone().update());
                glib::Continue(true)
            }),
        );

        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.emu.on_register_write(tx);
        rx.attach(
            Some(&self.context),
            clone!(@weak self as labels => @default-return glib::Continue(false), move |rw| {
                labels.handle_register_write(rw.reg, rw.value);
                glib::Continue(true)
            }),
        );
    }

    fn handle_register_write(&self, reg: WordRegister, value: u16) {
        self.label_for_register(reg)
            .set_text(&format!("{:04X}", value));
    }

    fn register_inputs(&self) -> Vec<(&gtk::Entry, WordRegister)> {
        WordRegister::all()
            .iter()
            .map(|reg| (self.label_for_register(*reg), *reg))
            .collect()
    }

    fn label_for_register(&self, reg: WordRegister) -> &gtk::Entry {
        match reg {
            WordRegister::AF => &self.widget.af_input,
            WordRegister::BC => &self.widget.bc_input,
            WordRegister::DE => &self.widget.de_input,
            WordRegister::HL => &self.widget.hl_input,
            WordRegister::SP => &self.widget.sp_input,
            WordRegister::PC => &self.widget.pc_input,
        }
    }

    fn set_editable(&self, editable: bool) {
        for (input, _) in self.register_inputs().iter_mut() {
            input.set_editable(editable);
        }
    }

    fn render(&self, registers: QueryRegistersResponse) {
        self.set_editable(true);
        for (input, register) in self.register_inputs().iter_mut() {
            let value = registers.read_u16(*register);
            input.set_text(&format!("{:04X}", value));
        }
    }

    async fn update(self: Rc<Self>) -> () {
        let register_result = self.emu.query_registers().await;
        match register_result {
            Ok(registers) => self.render(registers),
            Err(_) => self.set_editable(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils;

    #[test]
    fn gtk_render_text() {
        test_utils::setup_gtk().unwrap();
        let context = test_utils::setup_context();
        let emu = Rc::new(test_utils::get_loaded_remote_emu(context.clone()));
        let widget = RegisterLabelsWidget::default();
        let component = RegisterLabels::from_widget(context.clone(), emu, widget);
        test_utils::wait_for_task(&context, component.clone().update());

        component.render(QueryRegistersResponse {
            af: 0x6666,
            bc: 0x5555,
            de: 0x4444,
            hl: 0x3333,
            pc: 0x2222,
            sp: 0x1111,
        });

        let af_text: Option<String> = component.widget.af_input.get_text().map(|txt| txt.into());
        let bc_text: Option<String> = component.widget.bc_input.get_text().map(|txt| txt.into());
        let de_text: Option<String> = component.widget.de_input.get_text().map(|txt| txt.into());
        let hl_text: Option<String> = component.widget.hl_input.get_text().map(|txt| txt.into());
        let pc_text: Option<String> = component.widget.pc_input.get_text().map(|txt| txt.into());
        let sp_text: Option<String> = component.widget.sp_input.get_text().map(|txt| txt.into());

        assert_eq!(af_text, Some(String::from("6666")));
        assert_eq!(bc_text, Some(String::from("5555")));
        assert_eq!(de_text, Some(String::from("4444")));
        assert_eq!(hl_text, Some(String::from("3333")));
        assert_eq!(pc_text, Some(String::from("2222")));
        assert_eq!(sp_text, Some(String::from("1111")));
        context.release();
    }

    #[test]
    fn gtk_integration() {
        test_utils::setup_gtk().unwrap();
        let context = test_utils::setup_context();
        let emu = Rc::new(test_utils::get_loaded_remote_emu(context.clone()));
        let builder = gtk::Builder::new_from_string(include_str!("../../res/debugger.ui"));
        let component = RegisterLabels::from_builder(&builder, context.clone(), emu.clone());

        let task = async {
            emu.step().await.unwrap();
            emu.step().await.unwrap();
            emu.query_registers().await
        };
        let actual_registers = test_utils::wait_for_task(&context, task).unwrap();
        test_utils::digest_events(&context);

        let af_text: Option<String> = component.widget.af_input.get_text().map(|txt| txt.into());
        let bc_text: Option<String> = component.widget.bc_input.get_text().map(|txt| txt.into());
        let de_text: Option<String> = component.widget.de_input.get_text().map(|txt| txt.into());
        let hl_text: Option<String> = component.widget.hl_input.get_text().map(|txt| txt.into());
        let pc_text: Option<String> = component.widget.pc_input.get_text().map(|txt| txt.into());
        let sp_text: Option<String> = component.widget.sp_input.get_text().map(|txt| txt.into());

        assert_eq!(af_text, Some(format!("{:04X}", actual_registers.af)));
        assert_eq!(bc_text, Some(format!("{:04X}", actual_registers.bc)));
        assert_eq!(de_text, Some(format!("{:04X}", actual_registers.de)));
        assert_eq!(hl_text, Some(format!("{:04X}", actual_registers.hl)));
        assert_eq!(pc_text, Some(format!("{:04X}", actual_registers.pc)));
        assert_eq!(sp_text, Some(format!("{:04X}", actual_registers.sp)));
        context.release();
    }

    #[test]
    fn gtk_handle_write() {
        test_utils::setup_gtk().unwrap();
        let context = test_utils::setup_context();
        let emu = Rc::new(test_utils::get_loaded_remote_emu(context.clone()));
        let builder = gtk::Builder::new_from_string(include_str!("../../res/debugger.ui"));
        let component = RegisterLabels::from_builder(&builder, context.clone(), emu.clone());

        component.handle_register_write(WordRegister::BC, 0x8080);

        let bc_text: Option<String> = component.widget.bc_input.get_text().map(|txt| txt.into());

        assert_eq!(bc_text, Some("8080".into()));
        context.release();
    }
}
