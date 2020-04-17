use crate::builder_struct;
use crate::emulator::{commands::ExecMode, remote::RemoteEmulator};
use crate::utils;
use glib::clone;
use gtk::prelude::*;
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
    widget: PlaybackControlsWidget,
    emu: Rc<RemoteEmulator>,
}

impl PlaybackControls {
    pub(crate) fn from_widget(
        widget: PlaybackControlsWidget,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<PlaybackControls> {
        let controls = Rc::new(PlaybackControls { widget, emu });

        controls.connect_ui_events();
        controls.connect_adapter_events();

        controls
    }

    pub(crate) fn from_builder(
        builder: &gtk::Builder,
        emu: Rc<RemoteEmulator>,
    ) -> Rc<PlaybackControls> {
        let widget = PlaybackControlsWidget::from_builder(builder).unwrap();
        PlaybackControls::from_widget(widget, emu)
    }

    async fn step(self: Rc<Self>) -> () {
        utils::run_fallible(self.emu.step(), None).await;
    }

    async fn set_mode(self: Rc<Self>, mode: ExecMode) -> () {
        utils::run_infallible(self.emu.set_mode(mode)).await;
    }

    fn connect_adapter_events(self: &Rc<Self>) {
        let (tx, rx) = glib::MainContext::channel(glib::source::PRIORITY_DEFAULT);
        self.emu.on_mode_change(tx);
        rx.attach(
            Some(&glib::MainContext::default()),
            clone!(@strong self as controls => move |mode| {
                controls.apply_mode(mode);
                glib::Continue(true)
            }),
        );
    }

    fn connect_ui_events(self: &Rc<Self>) {
        let ctx = glib::MainContext::default();
        self.widget.step.connect_clicked(
            clone!(@strong self as controls, @strong ctx => move |_| {
                ctx.spawn_local(controls.clone().step());
            }),
        );
        self.widget.play.connect_toggled(
            clone!(@strong self as controls, @strong ctx => move |_| {
                let new_mode = if controls.widget.play.get_active() {
                    ExecMode::Standard
                } else {
                    ExecMode::Paused
                };
                ctx.spawn_local(controls.clone().set_mode(new_mode));
            }),
        );
        self.widget.fast.connect_toggled(
            clone!(@strong self as controls, @strong ctx => move |_| {
                let new_mode = if controls.widget.fast.get_active() {
                    ExecMode::Uncapped
                } else {
                    ExecMode::Paused
                };
                ctx.spawn_local(controls.clone().set_mode(new_mode));
            }),
        );
    }

    pub(crate) fn apply_mode(&self, mode: ExecMode) {
        self.widget.play.set_sensitive(mode != ExecMode::Unloaded);
        self.widget.play.set_active(mode == ExecMode::Standard);
        self.widget.step.set_sensitive(mode == ExecMode::Paused);
        self.widget.fast.set_sensitive(mode != ExecMode::Unloaded);
        self.widget.fast.set_active(mode == ExecMode::Uncapped);
    }
}
