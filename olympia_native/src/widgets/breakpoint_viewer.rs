use crate::builder_struct;
use crate::utils;

use derive_more::{Display, Error, From, Into};
use gtk::glib;
use gtk::glib::{clone, GBoxed};
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
const MONITOR_COLUMN_INDEX: i32 = 1;
const CONDITION_COLUMN_INDEX: i32 = 2;
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

#[derive(PartialEq, Eq, Clone, Debug, Display, Error)]
pub enum BreakpointParseError {
    #[display(fmt = "Invalid Target {0:?}", _0)]
    InvalidTarget(#[error(not(source))] String),
    #[display(fmt = "Invalid value {0:?}", _0)]
    InvalidValue(#[error(not(source))] String),
    #[display(fmt = "Invalid target {0:?} and invalid value {1:?}", _0, _1)]
    InvalidTargetAndValue(String, String),
}

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
        let id: u32 = self
            .get_tree_value(&path, ID_COLUMN_INDEX)
            .and_then(|v| v.get().ok())
            .unwrap_or_default();
        let new_state = !previous_state;
        let result = self.emu.set_breakpoint_state(id.into(), new_state).await;
        if let Ok(resp) = result {
            log::debug!(
                "Toggled breakpoint from {} to {}",
                previous_state,
                new_state
            );
            self.set_tree_value(&path, ACTIVE_COLUMN_INDEX, resp.new_state);
        }
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

    fn parse_breakpoint(&self) -> Result<Breakpoint, BreakpointParseError> {
        let target_text: String = self.widget.monitor_input.text().into();
        let target: Option<RWTarget> = target_text.parse().ok();
        let value_text: String = self.widget.value_input.text().into();
        let value = if let Some(RWTarget::Cycles) = target {
            let (num, multiplier) = if value_text.ends_with("s") {
                let s = value_text.replace("s", "");
                (String::from(s.as_str()), 1024 * 1024)
            } else {
                (value_text.clone(), 1)
            };
            u64::from_str_radix(&num, 16).ok().map(|x| x * multiplier)
        } else {
            u64::from_str_radix(&value_text, 16).ok()
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
            (Some(t), Some(c)) => Ok(Breakpoint::new(t, c)),
            (None, Some(_cond)) => Err(BreakpointParseError::InvalidTarget(target_text.clone())),
            (Some(_target), None) => Err(BreakpointParseError::InvalidValue(value_text.clone())),
            (None, None) => Err(BreakpointParseError::InvalidTargetAndValue(
                target_text.clone(),
                value_text.clone(),
            )),
        }
    }

    async fn add_parsed_breakpoint(&self, breakpoint: &Breakpoint) {
        let resp = utils::run_infallible(self.emu.add_breakpoint(breakpoint.clone())).await;
        self.widget.store.insert_with_values(
            None,
            &[
                (ACTIVE_COLUMN_INDEX as u32, &breakpoint.active),
                (
                    MONITOR_COLUMN_INDEX as u32,
                    &format!("{}", breakpoint.monitor),
                ),
                (
                    CONDITION_COLUMN_INDEX as u32,
                    &format!("{}", breakpoint.condition),
                ),
                (ID_COLUMN_INDEX as u32, &u32::from(resp.id)),
            ],
        );
    }

    async fn add_breakpoint(self: Rc<Self>) {
        match self.parse_breakpoint() {
            Ok(bp) => self.add_parsed_breakpoint(&bp).await,
            Err(err) => {
                utils::show_error_dialog(err, None).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use gtk::TreePath;

    use super::*;
    use crate::utils::test_utils;

    fn count_tree_items(component: Rc<BreakpointViewer>) -> u32 {
        let iter = component
            .widget
            .store
            .iter_first()
            .expect("No entries in tree");
        let mut count = 1;
        while component.widget.store.iter_next(&iter) {
            count += 1;
        }
        count
    }

    fn add_breakpoint(
        component: Rc<BreakpointViewer>,
        context: &glib::MainContext,
        emu: Rc<RemoteEmulator>,
        monitor: &str,
        condition: &str,
        value: Option<&str>,
    ) {
        component.widget.monitor_input.set_text(monitor);
        if let Some(v) = value {
            component.widget.value_input.set_text(v);
        }
        component
            .widget
            .condition_picker
            .set_active_id(Some(condition));
        component.widget.add_button.clicked();
        test_utils::next_tick(&context, &emu)
    }

    #[test]
    fn test_add_breakpoint() {
        test_utils::with_loaded_emu(|context, emu| {
            let builder = gtk::Builder::from_string(include_str!("../../res/breakpoints.ui"));
            let component = BreakpointViewer::from_builder(&builder, context.clone(), emu.clone());
            let store = &component.widget.store;

            add_breakpoint(
                component.clone(),
                &context,
                emu.clone(),
                "PC",
                "Equal",
                Some("20"),
            );

            add_breakpoint(
                component.clone(),
                &context,
                emu.clone(),
                "AF",
                "NotEqual",
                Some("40"),
            );

            let count = count_tree_items(component.clone());
            assert_eq!(count, 2);
            let iter = store.iter_first().unwrap();
            let active: bool = store.value(&iter, ACTIVE_COLUMN_INDEX).get().unwrap();
            assert_eq!(active, true);
            let monitor: String = store.value(&iter, MONITOR_COLUMN_INDEX).get().unwrap();
            assert_eq!(&monitor, "register PC");
            let condition: String = store.value(&iter, CONDITION_COLUMN_INDEX).get().unwrap();
            assert_eq!(&condition, "== 20");
        });
    }

    #[test]
    fn test_toggle_breakpoint() {
        test_utils::with_loaded_emu(|context, emu| {
            let builder = gtk::Builder::from_string(include_str!("../../res/breakpoints.ui"));
            let component = BreakpointViewer::from_builder(&builder, context.clone(), emu.clone());
            let store = &component.widget.store;
            add_breakpoint(
                component.clone(),
                &context,
                emu.clone(),
                "PC",
                "Equal",
                Some("20"),
            );
            add_breakpoint(
                component.clone(),
                &context,
                emu.clone(),
                "AF",
                "NotEqual",
                Some("40"),
            );
            let mut tree_path = TreePath::new_first();
            tree_path.next();
            component
                .widget
                .active_column_renderer
                .emit_by_name("toggled", &[&tree_path.to_str()])
                .unwrap();
            test_utils::next_tick(&context, &emu);

            let count = count_tree_items(component.clone());
            assert_eq!(count, 2);
            let iter = store.iter_first().unwrap();
            store.iter_next(&iter);
            let active: bool = store.value(&iter, ACTIVE_COLUMN_INDEX).get().unwrap();
            assert_eq!(active, false);
        });
    }
}
