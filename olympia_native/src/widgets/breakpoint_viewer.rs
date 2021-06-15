use crate::builder_struct;
use crate::utils;

use glib::clone;
use glib::value::FromValue;
use gtk::prelude::*;
use olympia_engine::gbdebug::BreakpointCondition;
use olympia_engine::gbdebug::Comparison;
use olympia_engine::{
    gbdebug::{Breakpoint, RWTarget},
    remote::{RemoteEmulator, UiBreakpoint},
};
use std::rc::Rc;

const ACTIVE_COLUMN_INDEX: i32 = 0;

builder_struct!(
    pub(crate) struct BreakpointViewerWidget {
        #[ogtk(id = "DebuggerBreakpointAdd")]
        add_button: gtk::Button,
        #[ogtk(id = "DebuggerBreakpointMonitorEntry")]
        monitor_input: gtk::Entry,
        #[ogtk(id = "BreakpointListStore")]
        store: gtk::ListStore,
        #[ogtk(id = "DebuggerConditionPicker")]
        condition_picker: gtk::ComboBoxText,
        #[ogtk(id = "DebuggerExpectedValueEntry")]
        value_input: gtk::Entry,
        #[ogtk(id = "BreakpointActiveToggle")]
        active_column_renderer: gtk::CellRendererToggle,
    }
);

pub(crate) struct BreakpointViewer {
    context: glib::MainContext,
    emu: Rc<RemoteEmulator>,
    widget: BreakpointViewerWidget,
}

impl BreakpointViewer {
    pub(crate) fn from_widget(
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
        widget: BreakpointViewerWidget,
    ) -> Rc<BreakpointViewer> {
        let bpv = Rc::new(BreakpointViewer {
            context,
            emu,
            widget,
        });

        bpv.connect_ui_events();
        bpv
    }

    pub(crate) fn from_builder(
        builder: &gtk::Builder,
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<BreakpointViewer> {
        let widget = BreakpointViewerWidget::from_builder(builder).unwrap();
        BreakpointViewer::from_widget(context, emu, widget)
    }

    fn add_clicked(self: &Rc<Self>) {
        self.context.spawn_local(self.clone().add_breakpoint());
    }

    fn get_tree_value(
        self: &Rc<Self>,
        path: &gtk::TreePath,
        column_index: i32,
    ) -> Option<glib::Value> {
        match self.widget.store.get_iter(path) {
            Some(x) => Some(self.widget.store.get_value(&x, column_index)),
            _ => None,
        }
    }

    fn set_tree_value<T: ToValue>(
        self: &Rc<Self>,
        path: &gtk::TreePath,
        column_index: i32,
        value: T,
    ) {
        let iter = self.widget.store.get_iter(path);

        let iter = match iter {
            Some(x) => x,
            _ => return,
        };
        self.widget
            .store
            .set_value(&iter, column_index as u32, &value.to_value());
    }

    fn bp_active_toggled(self: &Rc<Self>, path: gtk::TreePath) {
        let previous_state: bool = self
            .get_tree_value(&path, ACTIVE_COLUMN_INDEX)
            .and_then(|v| v.get_some().ok())
            .unwrap_or_default();
        let new_state = !previous_state;
        self.set_tree_value(&path, ACTIVE_COLUMN_INDEX, new_state);
        log::debug!(
            "Toggled breakpoint from {} to {}",
            previous_state,
            new_state
        );
    }

    fn condition_changed(self: &Rc<Self>) {
        let id = self.widget.condition_picker.get_active_id();

        if let Some(active_id) = id {
            log::debug!("Condition changed: {}", active_id);
            let has_value = !(active_id == "Read" || active_id == "Write");
            self.widget.value_input.set_visible(has_value);
        }
    }

    pub fn connect_ui_events(self: &Rc<Self>) {
        self.widget
            .add_button
            .connect_clicked(clone!(@weak self as bpv => move |_| {
                bpv.add_clicked();
            }));

        self.widget
            .condition_picker
            .connect_changed(clone!(@weak self as bpv => move |_| {
                bpv.condition_changed();
            }));

        self.widget.active_column_renderer.connect_toggled(
            clone!(@weak self as bpv => move |_, path| {
                bpv.bp_active_toggled(path);
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
        let picker = &self.widget.condition_picker;
        let condition = picker.get_active_text().and_then(|s| {
            let comparison: Result<Comparison, _> = s.parse();
            if let Ok(comp) = comparison {
                let expected_value = value?;
                Some(BreakpointCondition::Test(comp, expected_value))
            } else {
                picker.get_active_id().and_then(|id| {
                    println!("{}", id);
                    if id == "Read" {
                        Some(BreakpointCondition::Read)
                    } else if id == "Write" {
                        Some(BreakpointCondition::Write)
                    } else {
                        None
                    }
                })
            }
        });
        match (target, condition) {
            (Some(t), Some(c)) => Some(Breakpoint::new(t, c).into()),
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
                    &format!("{}", breakpoint.breakpoint.condition),
                ],
            );
        }
    }
}
