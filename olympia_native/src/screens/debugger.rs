use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use std::path::PathBuf;
use std::rc::Rc;

use crate::emulator::glib::glib_remote_emulator;
use crate::utils;
use crate::widgets::{
    BreakpointViewer, EmulatorDisplay, MemoryViewer, PlaybackControls, RegisterLabels,
};

use olympia_engine::remote::{LoadRomError, RemoteEmulator};

#[allow(dead_code)]
pub(crate) struct Debugger {
    emu: Rc<RemoteEmulator>,
    breakpoint_viewer: Rc<BreakpointViewer>,
    emulator_display: Rc<EmulatorDisplay>,
    memory_viewer: Rc<MemoryViewer>,
    register_labels: Rc<RegisterLabels>,
    playback_controls: Rc<PlaybackControls>,
    window: ApplicationWindow,
}

impl Debugger {
    pub(crate) fn new(app: &Application) -> Rc<Debugger> {
        let ctx = glib::MainContext::default();
        let emu = glib_remote_emulator(ctx.clone());

        let builder = gtk::Builder::new_from_string(include_str!("../../res/debugger.ui"));
        let playback_controls = PlaybackControls::from_builder(&builder, ctx.clone(), emu.clone());
        let window: ApplicationWindow = builder.get_object("MainWindow").unwrap();
        let open_action = gio::SimpleAction::new("open", None);
        let register_labels = RegisterLabels::from_builder(&builder, ctx.clone(), emu.clone());
        let emulator_display = EmulatorDisplay::from_builder(&builder, ctx.clone(), emu.clone());
        let memory_viewer = MemoryViewer::from_builder(&builder, ctx.clone(), emu.clone(), 17);
        let breakpoint_viewer = BreakpointViewer::from_builder(&builder, ctx.clone(), emu.clone());

        window.set_application(Some(app));
        window.add_action(&open_action);

        let debugger = Rc::new(Debugger {
            emu,
            breakpoint_viewer,
            emulator_display,
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

    async fn load_rom_fs(&self, path: PathBuf) -> Result<(), LoadRomError> {
        let data =
            std::fs::read(path).map_err(|err| LoadRomError::Io(format!("{}", err).into()))?;
        self.emu.load_rom(data).await
    }

    async fn load_rom(self: Rc<Self>, path: PathBuf) -> () {
        utils::run_fallible(self.load_rom_fs(path), Some(&self.window)).await;
    }

    pub(crate) fn show_all(&self) {
        self.window.show_all();
    }
}
