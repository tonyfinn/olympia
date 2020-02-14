use gtk::prelude::*;
use crate::emulator::{EmulatorAdapter, ExecMode};
use crate::utils;
use glib::clone;
use std::rc::Rc;

pub(crate) struct PlaybackControls {
    play: gtk::ToggleButton,
    fast: gtk::ToggleButton,
    step: gtk::Button,
    adapter: Rc<EmulatorAdapter>,
}

impl PlaybackControls {
    pub(crate) fn from_builder(builder: &gtk::Builder, adapter: Rc<EmulatorAdapter>) -> Rc<PlaybackControls> {
        let controls = Rc::new(PlaybackControls {
            play: builder.get_object("PlayButton").unwrap(),
            fast: builder.get_object("FastButton").unwrap(),
            step: builder.get_object("StepButton").unwrap(),
            adapter
        });

        controls.connect_ui_events();
        controls.connect_adapter_events();

        controls
    }

    async fn step(self: Rc<Self>) -> () {
        utils::run_fallible(self.adapter.step(), None).await;
    }

    async fn set_mode(self: Rc<Self>, mode: ExecMode) -> () {
        utils::run_fallible(self.adapter.set_mode(mode), None).await;
    }

    fn connect_adapter_events(self: &Rc<Self>) {
        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.adapter.on_mode_change(tx);
        rx.attach(Some(&glib::MainContext::default()), clone!(@strong self as controls => move |mode| {
            controls.apply_mode(mode);
            glib::Continue(true)
        }));
    }

    fn connect_ui_events(self: &Rc<Self>) {
        let ctx = glib::MainContext::default();
        self.step.connect_clicked(clone!(@strong self as controls, @strong ctx => move |_| {
            ctx.spawn_local(controls.clone().step());
        }));
        self.play.connect_toggled(clone!(@strong self as controls, @strong ctx => move |_| {
            let new_mode = if controls.play.get_active() {
                ExecMode::Standard
            } else {
                ExecMode::Paused
            };
            ctx.spawn_local(controls.clone().set_mode(new_mode));
        }));
        self.fast.connect_toggled(clone!(@strong self as controls, @strong ctx => move |_| {
            let new_mode = if controls.fast.get_active() {
                ExecMode::Uncapped
            } else {
                ExecMode::Paused
            };
            ctx.spawn_local(controls.clone().set_mode(new_mode));
        }));
    }

    pub(crate) fn apply_mode(&self, mode: ExecMode) {
        self.play.set_sensitive(mode != ExecMode::Unloaded);
        self.play.set_active(mode == ExecMode::Standard);
        self.step.set_sensitive(mode == ExecMode::Paused);
        self.fast.set_sensitive(mode != ExecMode::Unloaded);
        self.fast.set_active(mode == ExecMode::Uncapped);
    }
}