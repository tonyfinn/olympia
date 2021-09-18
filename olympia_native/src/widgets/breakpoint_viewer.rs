use crate::builder_struct;
use crate::utils;

use derive_more::{From, Into};
use glib::clone;
use glib::subclass::prelude::*;
use glib::GBoxed;
use gtk::prelude::*;
use olympia_engine::monitor::BreakpointCondition;
use olympia_engine::monitor::BreakpointIdentifier;
use olympia_engine::monitor::Comparison;
use olympia_engine::{
    monitor::{Breakpoint, RWTarget},
    remote::RemoteEmulator,
};
use std::rc::Rc;

const ACTIVE_COLUMN_INDEX: i32 = 0;
const ID_COLUMN_INDEX: i32 = 3;

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

#[derive(Clone, Debug, PartialEq, Eq, GBoxed, From, Into)]
#[gboxed(type_name = "boxed_identifier")]
struct BoxedIdentifier(BreakpointIdentifier);

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

    fn add_clicked(self: Rc<Self>) {
        self.context.spawn_local(self.clone().add_breakpoint());
    }

    fn get_tree_value(
        self: &Rc<Self>,
        path: &gtk::TreePath,
        column_index: i32,
    ) -> Option<glib::Value> {
        match self.widget.store.iter(path) {
            Some(x) => Some(self.widget.store.value(&x, column_index)),
            _ => None,
        }
    }

    fn set_tree_value<T: ToValue>(
        self: &Rc<Self>,
        path: &gtk::TreePath,
        column_index: i32,
        value: T,
    ) {
        let iter = self.widget.store.iter(path);

        let iter = match iter {
            Some(x) => x,
            _ => return,
        };
        self.widget
            .store
            .set_value(&iter, column_index as u32, &value.to_value());
    }

    fn bp_active_toggled(self: &Rc<Self>, path: gtk::TreePath) {
        self.context
            .spawn_local(self.clone().toggle_breakpoint(path));
    }

    async fn toggle_breakpoint(self: Rc<Self>, path: gtk::TreePath) {
        let previous_state: bool = self
            .get_tree_value(&path, ACTIVE_COLUMN_INDEX)
            .and_then(|v| v.get().ok())
            .unwrap_or_default();
        let id: bool = self
            .get_tree_value(&path, ACTIVE_COLUMN_INDEX)
            .and_then(|v| v.get().ok())
            .unwrap_or_default();
        let new_state = !previous_state;
        /*let result = self.emu.set_breakpoint_state(id, new_state).await;
        if let Ok(resp) = result {
            log::debug!(
                "Toggled breakpoint from {} to {}",
                previous_state,
                new_state
            );
            self.set_tree_value(&path, ACTIVE_COLUMN_INDEX, resp.new_state);
        }*/
    }

    fn condition_changed(self: &Rc<Self>) {
        let id = self.widget.condition_picker.active_id();

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

    fn parse_breakpoint(&self) -> Option<Breakpoint> {
        let target: Option<RWTarget> = self.widget.monitor_input.text().parse().ok();
        let value = if let Some(RWTarget::Cycles) = target {
            let text = self.widget.value_input.text();
            let s = text.as_str();
            let (num, multiplier) = if s.ends_with("s") {
                let s = s.replace("s", "");
                (String::from(s.as_str()), 1024 * 1024)
            } else {
                (s.into(), 1)
            };
            u64::from_str_radix(&num, 16).ok().map(|x| x * multiplier)
        } else {
            let s = self.widget.value_input.text();
            u64::from_str_radix(s.as_str(), 16).ok()
        };
        let picker = &self.widget.condition_picker;
        let condition = picker.active_text().and_then(|s| {
            let comparison: Result<Comparison, _> = s.parse();
            if let Ok(comp) = comparison {
                let expected_value = value?;
                Some(BreakpointCondition::Test(comp, expected_value))
            } else {
                picker.active_id().and_then(|id| {
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
            (Some(t), Some(c)) => Some(Breakpoint::new(t, c)),
            _ => None,
        }
    }

    async fn add_breakpoint(self: Rc<Self>) {
        if let Some(ref breakpoint) = self.parse_breakpoint() {
            utils::run_infallible(self.emu.add_breakpoint(breakpoint.clone())).await;
            self.widget.store.insert_with_values(
                None,
                &[
                    (0, &breakpoint.active),
                    (1, &format!("{}", breakpoint.monitor)),
                    (2, &format!("{}", breakpoint.condition)),
                ],
            );
        }
    }
}
