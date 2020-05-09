use gtk::prelude::*;

pub(crate) fn show_error_dialog<E: std::error::Error>(
    err: E,
    window: Option<&gtk::ApplicationWindow>,
) {
    let dialog = gtk::MessageDialog::new(
        window,
        gtk::DialogFlags::all(),
        gtk::MessageType::Error,
        gtk::ButtonsType::Ok,
        &format!("{}", err),
    );
    dialog.show_all();
}

pub(crate) async fn run_infallible<T, F>(future: F) -> ()
where
    F: std::future::Future<Output = Result<T, ()>>,
{
    match future.await {
        Ok(_) => {}
        Err(_) => {}
    }
}

pub(crate) async fn run_fallible<T, E, F>(future: F, window: Option<&gtk::ApplicationWindow>) -> ()
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::error::Error,
{
    if let Err(e) = future.await {
        show_error_dialog(e, window);
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use crate::emulator::glib::glib_remote_emulator;
    use crate::emulator::remote::RemoteEmulator;
    use glib::error::BoolError;
    use std::{
        future::Future,
        path::{Path, PathBuf},
        rc::Rc,
        time::{Duration, Instant},
    };

    pub fn wait_for_task<T>(ctx: &glib::MainContext, t: impl Future<Output = T>) -> T {
        let start_time = Instant::now();
        let result = ctx.block_on(t);
        let timeout = Duration::from_millis(1000);
        while ctx.pending() {
            if start_time.elapsed() > timeout {
                panic!("Timeout of {:?} elapsed", timeout);
            }
            ctx.iteration(true);
        }
        result
    }

    pub fn digest_events(ctx: &glib::MainContext) {
        let start_time = Instant::now();
        let timeout = Duration::from_millis(1000);
        while ctx.pending() {
            if start_time.elapsed() > timeout {
                panic!("Timeout of {:?} elapsed", timeout);
            }
            ctx.iteration(true);
        }
    }

    pub(crate) fn fizzbuzz_path() -> PathBuf {
        let mut path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_owned();
        path.push("res");
        path.push("fizzbuzz.gb");
        path
    }

    /// Gets a sample remote emulator setup with no ROM loaded
    pub(crate) fn get_unloaded_remote_emu(
        context: glib::MainContext,
    ) -> Rc<crate::emulator::remote::RemoteEmulator> {
        let emu = glib_remote_emulator(context);
        emu
    }

    // Gets a sample remote emulator setup loaded with a fizzbuzz ROM
    pub(crate) fn get_loaded_remote_emu(
        context: glib::MainContext,
    ) -> Rc<crate::emulator::remote::RemoteEmulator> {
        let emu = get_unloaded_remote_emu(context.clone());

        let task = async {
            emu.load_rom(fizzbuzz_path()).await.unwrap();
        };
        wait_for_task(&context, task);
        emu
    }

    /// Initialises GTK with a default MainContext for test purposes
    pub(crate) fn setup_gtk() -> Result<(), BoolError> {
        if !gtk::is_initialized() {
            gtk::init().expect("Failed to init GTK");
        }

        Ok(())
    }

    pub(crate) fn next_tick(context: &glib::MainContext, emu: &RemoteEmulator) {
        wait_for_task(&context, emu.query_registers()).unwrap();
    }

    pub(crate) fn setup_context() -> glib::MainContext {
        let context = glib::MainContext::new();
        context.acquire();
        context
    }
}
