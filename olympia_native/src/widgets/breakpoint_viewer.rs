use crate::emulator::UiBreakpoint;

use glib::clone;
use gtk::prelude::*;
use olympia_engine::debug::{Breakpoint, RWTarget};
use std::rc::Rc;

use crate::emulator::EmulatorAdapter;
use crate::utils;

pub(crate) struct BreakpointViewer {
    adapter: Rc<EmulatorAdapter>,
    add_button: gtk::Button,
    monitor_input: gtk::Entry,
    store: gtk::ListStore,
    value_input: gtk::Entry,
}

impl BreakpointViewer {

    pub(crate) fn from_builder(builder: &gtk::Builder, adapter: Rc<EmulatorAdapter>) -> Rc<BreakpointViewer> {
        let store: gtk::ListStore = builder.get_object("BreakpointListStore").unwrap();
        let monitor_input: gtk::Entry = builder
            .get_object("DebuggerBreakpointMonitorEntry")
            .unwrap();
        let value_input: gtk::Entry = builder.get_object("DebuggerExpectedValueEntry").unwrap();
        let add_button: gtk::Button = builder.get_object("DebuggerBreakpointAdd").unwrap();
        let bpv = Rc::new(BreakpointViewer {
            adapter,
            add_button,
            monitor_input,
            store,
            value_input,
        });
        bpv.connect_ui_events();
        bpv
    }

    pub fn connect_ui_events(self: &Rc<Self>) {
        let ctx = glib::MainContext::default();
        self.add_button.connect_clicked(clone!(@strong self as bpv, @strong ctx => move |_| {
            ctx.spawn_local(bpv.clone().add_breakpoint())
        }));
    }

    fn parse_breakpoint(&self) -> Option<UiBreakpoint> {
        let target: Option<RWTarget> = self
            .monitor_input
            .get_text()
            .and_then(|s| s.as_str().parse().ok());
        let value = if let Some(RWTarget::Cycles) = target {
            self.value_input.get_text().and_then(|text| {
                let s = text.as_str();
                let (num, multiplier) = if s.ends_with("s") {
                    let s = s.replace("s", "");
                    (String::from(s.as_str()), 1024 * 1024)
                } else {
                    (s.into(), 1)
                };
                u64::from_str_radix(&num, 16).ok().map(|x| x * multiplier)
            })
        } else {
            self.value_input
                .get_text()
                .and_then(|s| u64::from_str_radix(s.as_str(), 16).ok())
        };
        match (target, value) {
            (Some(t), Some(v)) => Some(Breakpoint::new(t, v).into()),
            _ => None,
        }
    }

    async fn add_breakpoint(self: Rc<Self>) {
        if let Some(ref breakpoint) = self.parse_breakpoint() {
            utils::run_infallible(self.adapter.add_breakpoint(breakpoint.clone())).await;
            self.store.insert_with_values(
                None,
                &[0, 1, 2],
                &[
                    &breakpoint.active,
                    &format!("{}", breakpoint.breakpoint.monitor),
                    &format!("== {:X}", breakpoint.breakpoint.value),
                ],
            );
        }
    }
}
