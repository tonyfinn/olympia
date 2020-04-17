mod builder;
mod emulator;
mod screens;
mod utils;
mod widgets;

use gio::prelude::*;
use gtk::prelude::*;
use gtk::Application;

struct EmulatorApp {
    gtk_app: Application,
}

impl EmulatorApp {
    fn new() -> EmulatorApp {
        let gtk_app = Application::new(Some("com.tonyfinn.olympia_native"), Default::default())
            .expect("Failed to start GTK");

        let mut emu = EmulatorApp { gtk_app };
        emu.register_events();
        emu
    }

    fn register_events(&mut self) {
        self.gtk_app.connect_startup(|app| {
            let quit = gio::SimpleAction::new("quit", None);
            quit.connect_activate(|_, _| std::process::exit(0));
            app.add_action(&quit);

            let menu_builder = gtk::Builder::new_from_string(include_str!("../res/menu.ui"));
            let app_main_menu: gio::Menu = menu_builder.get_object("MainMenu").unwrap();
            app.set_menubar(Some(&app_main_menu));
        });

        self.gtk_app.connect_activate(|app| {
            let debugger_window = screens::Debugger::new(&app);
            debugger_window.show_all();
        });
    }

    fn start(self) {
        self.gtk_app.run(&[]);
    }
}

fn main() {
    let app = EmulatorApp::new();
    app.start();
}
