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

fn create_child<C: IsA<gtk::Widget> + IsA<glib::Object>>(
    parent_builder: &gtk::Builder,
    builder_xml: &str,
    container_id: &str,
    content_id: &str,
) -> gtk::Builder {
    let child_builder = gtk::Builder::from_string(builder_xml);
    let container: gtk::Box = parent_builder.object(container_id).unwrap();
    let content: C = child_builder.object(content_id).unwrap();
    container.pack_start(&content, true, true, 0);
    child_builder
}

impl Debugger {
    pub(crate) fn new(app: &Application) -> Rc<Debugger> {
        let ctx = glib::MainContext::default();
        let emu = glib_remote_emulator(ctx.clone());

        let root_builder = gtk::Builder::from_string(include_str!("../../res/debugger.ui"));

        let playback_controls =
            PlaybackControls::from_builder(&root_builder, ctx.clone(), emu.clone());
        let window: ApplicationWindow = root_builder.object("MainWindow").unwrap();
        let open_action = gio::SimpleAction::new("open", None);
        let emulator_display =
            EmulatorDisplay::from_builder(&root_builder, ctx.clone(), emu.clone());

        let register_builder = create_child::<gtk::Box>(
            &root_builder,
            include_str!("../../res/registers.ui"),
            "RegistersContainer",
            "Registers",
        );
        let register_labels =
            RegisterLabels::from_builder(&register_builder, ctx.clone(), emu.clone());

        let memv_builder = create_child::<gtk::Box>(
            &root_builder,
            include_str!("../../res/memory.ui"),
            "MemoryContainer",
            "Memory",
        );
        let memory_viewer = MemoryViewer::from_builder(&memv_builder, ctx.clone(), emu.clone(), 17);

        create_child::<gtk::Label>(
            &root_builder,
            include_str!("../../res/disassembly.ui"),
            "DisassemblyContainer",
            "Disassembly",
        );

        let bpv_builder = create_child::<gtk::Box>(
            &root_builder,
            include_str!("../../res/breakpoints.ui"),
            "BreakpointsContainer",
            "Breakpoints",
        );
        let breakpoint_viewer =
            BreakpointViewer::from_builder(&bpv_builder, ctx.clone(), emu.clone());

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
                if let Some(filename) = file_chooser.filename() {
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
