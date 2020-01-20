use gdk::prelude::*;
use gtk::prelude::*;
use gio::prelude::*;

use gtk::{Application,ApplicationWindow,Button};

fn main() {
    let app = Application::new(
        Some("com.tonyfinn.olympia_native"),
        Default::default()
    ).expect("Failed to start GTK");

    app.connect_activate(|app| {
        let builder = gtk::Builder::new_from_string(include_str!("../res/debugger.glade"));
        let window = build_window(app);
        let grid: gtk::Grid = builder.get_object("DebuggerPanel").unwrap();
        let step_button: Button = builder.get_object("StepButton").unwrap();
        step_button.connect_clicked(|_| {
            println!("Stepped");
        });
        let reset_button: Button = builder.get_object("ResetButton").unwrap();
        reset_button.connect_clicked(|_| {
            println!("Reset");
        });

        window.add(&grid);

        window.show_all();
    });

    app.run(&[]);
}

fn build_window(app: &Application) -> ApplicationWindow {
    let window = ApplicationWindow::new(app);

    let geometry = gdk::Geometry {
        height_inc: 1,
        width_inc: 1,
        base_height: 300,
        base_width: 300,
        min_width: 300,
        min_height: 300,
        max_width: -1,
        max_height: -1,
        min_aspect: 0.0,
        max_aspect: 100.0,
        win_gravity: gdk::Gravity::Static,
    };

    let mut hints = gdk::WindowHints::MIN_SIZE;
    hints.insert(gdk::WindowHints::BASE_SIZE);

    window.set_geometry_hints(None::<&gtk::Widget>, Some(&geometry), hints);
    window.set_title("Olympia");

    window
}