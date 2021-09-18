use crate::{builder_struct, utils};
use glib::clone;
use gtk::prelude::*;
use olympia_engine::{
    events::ModeChangeEvent,
    remote::{ExecMode, RemoteEmulator},
};
use std::rc::Rc;

builder_struct!(
    pub(crate) struct PlaybackControlsWidget {
        #[ogtk(id = "PlayButton")]
        play: gtk::ToggleButton,
        #[ogtk(id = "FastButton")]
        fast: gtk::ToggleButton,
        #[ogtk(id = "StepButton")]
        step: gtk::Button,
    }
);

pub(crate) struct PlaybackControls {
    context: glib::MainContext,
    emu: Rc<RemoteEmulator>,
    widget: PlaybackControlsWidget,
}

impl PlaybackControls {
    pub(crate) fn from_widget(
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
        widget: PlaybackControlsWidget,
    ) -> Rc<PlaybackControls> {
        let controls = Rc::new(PlaybackControls {
            context,
            emu,
            widget,
        });

        controls.connect_ui_events();
        controls.connect_adapter_events();

        controls
    }

    pub(crate) fn from_builder(
        builder: &gtk::Builder,
        context: glib::MainContext,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<PlaybackControls> {
        let widget = PlaybackControlsWidget::from_builder(builder).unwrap();
        PlaybackControls::from_widget(context, emu, widget)
    }

    async fn step(self: Rc<Self>) -> () {
        match utils::run_fallible(self.emu.step(), None).await {
            Ok(_) => {}
            Err(e) => {
                log::warn!(target: "playback_controls", "Failed to step on manual click: {}", e)
            }
        };
    }

    async fn set_mode(self: Rc<Self>, mode: ExecMode) -> () {
        utils::run_infallible(self.emu.set_mode(mode)).await;
    }

    fn connect_adapter_events(self: &Rc<Self>) {
        self.emu
            .on_widget(self.clone(), |controls, mode: ModeChangeEvent| {
                controls.apply_mode(mode.new_mode);
            });
    }

    fn step_clicked(self: &Rc<Self>) {
        self.context.spawn_local(self.clone().step());
    }

    fn play_clicked(self: &Rc<Self>) {
        let new_mode = if self.widget.play.is_active() {
            ExecMode::Standard
        } else {
            ExecMode::Paused
        };
        self.context.spawn_local(self.clone().set_mode(new_mode));
    }

    fn fast_clicked(self: &Rc<Self>) {
        let new_mode = if self.widget.fast.is_active() {
            ExecMode::Uncapped
        } else {
            ExecMode::Paused
        };
        self.context.spawn_local(self.clone().set_mode(new_mode));
    }

    fn connect_ui_events(self: &Rc<Self>) {
        self.widget
            .step
            .connect_clicked(clone!(@weak self as controls => move |_| {
                controls.step_clicked();
            }));
        self.widget
            .play
            .connect_toggled(clone!(@weak self as controls => move |_| {
                controls.play_clicked();
            }));
        self.widget
            .fast
            .connect_toggled(clone!(@weak self as controls => move |_| {
                controls.fast_clicked();
            }));
    }

    pub(crate) fn apply_mode(&self, mode: ExecMode) {
        self.widget.play.set_sensitive(mode != ExecMode::Unloaded);
        self.widget.play.set_active(mode == ExecMode::Standard);
        self.widget.step.set_sensitive(mode == ExecMode::Paused);
        self.widget.fast.set_sensitive(mode != ExecMode::Unloaded);
        self.widget.fast.set_active(mode == ExecMode::Uncapped);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::test_utils;

    #[test]
    fn gtk_test_from_builder_config() {
        test_utils::with_unloaded_emu(|context, emu| {
            let builder = gtk::Builder::from_string(include_str!("../../res/debugger.ui"));
            let component = PlaybackControls::from_builder(&builder, context.clone(), emu.clone());

            assert_eq!(false, component.widget.play.get_sensitive());
            assert_eq!(false, component.widget.play.is_active());
            assert_eq!(false, component.widget.step.get_sensitive());
            assert_eq!(false, component.widget.fast.get_sensitive());
            assert_eq!(false, component.widget.fast.is_active());
        });
    }

    #[test]
    fn gtk_test_activate_buttons_on_rom_load() {
        test_utils::with_unloaded_emu(|context, emu| {
            let builder = gtk::Builder::from_string(include_str!("../../res/debugger.ui"));
            let component = PlaybackControls::from_builder(&builder, context.clone(), emu.clone());

            let task = async {
                emu.load_rom(test_utils::fizzbuzz_rom()).await.unwrap();
            };
            test_utils::wait_for_task(&context, task);

            assert_eq!(true, component.widget.play.get_sensitive());
            assert_eq!(false, component.widget.play.is_active());
            assert_eq!(true, component.widget.step.get_sensitive());
            assert_eq!(true, component.widget.fast.get_sensitive());
            assert_eq!(false, component.widget.fast.is_active());
        });
    }

    #[test]
    fn gtk_test_play_toggle_button() {
        test_utils::with_unloaded_emu(|context, emu| {
            let builder = gtk::Builder::from_string(include_str!("../../res/debugger.ui"));
            let component = PlaybackControls::from_builder(&builder, context.clone(), emu.clone());

            let task = async {
                emu.load_rom(test_utils::fizzbuzz_rom()).await.unwrap();
            };
            test_utils::wait_for_task(&context, task);

            component.widget.play.set_active(true);

            test_utils::next_tick(&context, &emu);

            assert_eq!(true, component.widget.play.get_sensitive());
            assert_eq!(true, component.widget.play.is_active());
            assert_eq!(true, component.widget.fast.get_sensitive());
            assert_eq!(false, component.widget.fast.is_active());
            assert_eq!(false, component.widget.step.get_sensitive());

            component.widget.play.set_active(false);

            test_utils::next_tick(&context, &emu);

            assert_eq!(true, component.widget.play.get_sensitive());
            assert_eq!(false, component.widget.play.is_active());
            assert_eq!(true, component.widget.fast.get_sensitive());
            assert_eq!(false, component.widget.fast.is_active());
            assert_eq!(true, component.widget.step.get_sensitive());
        });
    }

    #[test]
    fn gtk_test_fast_toggle_button() {
        test_utils::with_unloaded_emu(|context, emu| {
            let builder = gtk::Builder::from_string(include_str!("../../res/debugger.ui"));
            let component = PlaybackControls::from_builder(&builder, context.clone(), emu.clone());

            let task = async {
                emu.load_rom(test_utils::fizzbuzz_rom()).await.unwrap();
            };
            test_utils::wait_for_task(&context, task);

            component.widget.fast.set_active(true);

            test_utils::next_tick(&context, &emu);

            assert_eq!(true, component.widget.play.get_sensitive());
            assert_eq!(false, component.widget.play.is_active());
            assert_eq!(true, component.widget.fast.get_sensitive());
            assert_eq!(true, component.widget.fast.is_active());
            assert_eq!(false, component.widget.step.get_sensitive());

            component.widget.fast.set_active(false);
            test_utils::next_tick(&context, &emu);

            assert_eq!(true, component.widget.play.get_sensitive());
            assert_eq!(false, component.widget.play.is_active());
            assert_eq!(true, component.widget.fast.get_sensitive());
            assert_eq!(false, component.widget.fast.is_active());
            assert_eq!(true, component.widget.step.get_sensitive());
        });
    }

    #[test]
    fn gtk_test_step_toggle_button() {
        test_utils::with_unloaded_emu(|context, emu| {
            let builder = gtk::Builder::from_string(include_str!("../../res/debugger.ui"));
            let component = PlaybackControls::from_builder(&builder, context.clone(), emu.clone());

            let task = async {
                emu.load_rom(test_utils::fizzbuzz_rom()).await.unwrap();
            };
            test_utils::wait_for_task(&context, task);

            component.widget.step.clicked();

            let registers = test_utils::wait_for_task(&context, emu.query_registers()).unwrap();
            assert_eq!(registers.pc, 0x101);
        });
    }
}
