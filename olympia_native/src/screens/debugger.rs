use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use std::boxed::Box;
use std::path::PathBuf;
use std::rc::Rc;

use crate::emulator::remote::{GlibEmulatorChannel, RemoteEmulator};
use crate::utils;
use crate::widgets::{BreakpointViewer, MemoryViewer, PlaybackControls, RegisterLabels};

#[allow(dead_code)]
pub(crate) struct Debugger {
    emu: Rc<RemoteEmulator>,
    breakpoint_viewer: Rc<BreakpointViewer>,
    memory_viewer: Rc<MemoryViewer>,
    register_labels: Rc<RegisterLabels>,
    playback_controls: Rc<PlaybackControls>,
    window: ApplicationWindow,
}

impl Debugger {
    pub(crate) fn new(app: &Application) -> Rc<Debugger> {
        let ctx = glib::MainContext::default();
        let glib_emu = Box::new(GlibEmulatorChannel::new());
        let emu = Rc::new(RemoteEmulator::new(glib_emu));

        let builder = gtk::Builder::new_from_string(include_str!("../../res/debugger.ui"));
        let playback_controls = PlaybackControls::from_builder(&builder, ctx.clone(), emu.clone());
        let window: ApplicationWindow = builder.get_object("MainWindow").unwrap();
        let open_action = gio::SimpleAction::new("open", None);
        let register_labels = RegisterLabels::from_builder(&builder, ctx.clone(), emu.clone());
        let memory_viewer = MemoryViewer::from_builder(&builder, ctx.clone(), emu.clone(), 17);
        let breakpoint_viewer = BreakpointViewer::from_builder(&builder, ctx.clone(), emu.clone());

        window.set_application(Some(app));
        window.add_action(&open_action);

        let debugger = Rc::new(Debugger {
            emu,
            breakpoint_viewer,
            memory_viewer,
            playback_controls,
            register_labels,
            window: window.clone(),
        });

        open_action.connect_activate(
            clone!(@strong debugger, @strong window, @strong ctx => move |_, _| {
                let file_chooser = gtk::FileChooserNative::new(
                    Some("Load ROM"),
                    Some(&window),
                    gtk::FileChooserAction::Open,
                    None,
                    None,
                );
                file_chooser.run();
                if let Some(filename) = file_chooser.get_filename() {
                    ctx.spawn_local(debugger.clone().load_rom(filename.into()));
                }
            }),
        );

        debugger
    }

    async fn load_rom(self: Rc<Self>, filename: PathBuf) -> () {
        utils::run_fallible(self.emu.load_rom(filename), Some(&self.window)).await;
    }

    pub(crate) fn show_all(&self) {
        self.window.show_all();
    }
}
