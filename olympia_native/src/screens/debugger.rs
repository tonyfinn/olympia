use gio::prelude::*;
use glib::clone;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow};
use std::path::PathBuf;
use std::rc::Rc;

use crate::emulator::EmulatorAdapter;
use crate::utils;
use crate::widgets::{
    BreakpointViewer,
    MemoryViewer,
    PlaybackControls,
    RegisterLabels,
};

#[allow(dead_code)]
pub(crate) struct Debugger {
    adapter: Rc<EmulatorAdapter>,
    breakpoint_viewer: Rc<BreakpointViewer>,
    memory_viewer: Rc<MemoryViewer>,
    register_labels: Rc<RegisterLabels>,
    playback_controls: Rc<PlaybackControls>,
    window: ApplicationWindow,
}

impl Debugger {

    pub(crate) fn new(app: &Application) -> Rc<Debugger> {
        let ctx = glib::MainContext::default();
        let adapter = Rc::new(EmulatorAdapter::new(&ctx));

        let builder = gtk::Builder::new_from_string(include_str!("../../res/debugger.ui"));
        let playback_controls = PlaybackControls::from_builder(&builder, adapter.clone());
        let window: ApplicationWindow = builder.get_object("MainWindow").unwrap();
        let open_action = gio::SimpleAction::new("open", None);
        let register_labels = RegisterLabels::from_builder(&builder, adapter.clone());
        let memory_viewer = MemoryViewer::from_builder(&builder, 17, adapter.clone());
        let breakpoint_viewer = BreakpointViewer::from_builder(&builder, adapter.clone());

        window.set_application(Some(app));
        window.add_action(&open_action);

        let debugger = Rc::new(Debugger {
            adapter,
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
        utils::run_fallible(self.adapter.load_rom(filename), Some(&self.window)).await;
    }

    pub(crate) fn show_all(&self) {
        self.window.show_all();
    }
}