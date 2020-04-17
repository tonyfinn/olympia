use crate::builder_struct;
use crate::emulator::{commands::UiBreakpoint, remote::RemoteEmulator};
use crate::utils;

use glib::clone;
use gtk::prelude::*;
use olympia_engine::debug::{Breakpoint, RWTarget};
use std::rc::Rc;

builder_struct!(
    pub(crate) struct BreakpointViewerWidget {
        #[ogtk(id = "DebuggerBreakpointAdd")]
        add_button: gtk::Button,
        #[ogtk(id = "DebuggerBreakpointMonitorEntry")]
        monitor_input: gtk::Entry,
        #[ogtk(id = "BreakpointListStore")]
        store: gtk::ListStore,
        #[ogtk(id = "DebuggerExpectedValueEntry")]
        value_input: gtk::Entry,
    }
);

pub(crate) struct BreakpointViewer {
    emu: Rc<RemoteEmulator>,
    widget: BreakpointViewerWidget,
}

impl BreakpointViewer {
    pub(crate) fn from_widget(
        widget: BreakpointViewerWidget,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<BreakpointViewer> {
        let bpv = Rc::new(BreakpointViewer { widget, emu });

        bpv.connect_ui_events();
        bpv
    }

    pub(crate) fn from_builder(
        builder: &gtk::Builder,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<BreakpointViewer> {
        let widget = BreakpointViewerWidget::from_builder(builder).unwrap();
        BreakpointViewer::from_widget(widget, emu)
    }

    pub fn connect_ui_events(self: &Rc<Self>) {
        let ctx = glib::MainContext::default();
        self.widget.add_button.connect_clicked(
            clone!(@strong self as bpv, @strong ctx => move |_| {
                ctx.spawn_local(bpv.clone().add_breakpoint())
            }),
        );
    }

    fn parse_breakpoint(&self) -> Option<UiBreakpoint> {
        let target: Option<RWTarget> = self
            .widget
            .monitor_input
            .get_text()
            .and_then(|s| s.as_str().parse().ok());
        let value = if let Some(RWTarget::Cycles) = target {
            self.widget.value_input.get_text().and_then(|text| {
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
            self.widget
                .value_input
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
            utils::run_infallible(self.emu.add_breakpoint(breakpoint.clone())).await;
            self.widget.store.insert_with_values(
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
