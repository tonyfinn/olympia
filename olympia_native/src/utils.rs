use derive_more::{Deref, From, Into};
use gtk::glib::value::FromValue;
use gtk::glib::{self, GBoxed};
use gtk::prelude::*;
use std::rc::Rc;

use olympia_engine::remote::RemoteEmulator;

pub(crate) async fn show_error_dialog<E: std::error::Error>(
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
    dialog.run_future().await;
    dialog.close();
}

pub(crate) async fn run_infallible<T, F>(future: F) -> T
where
    F: std::future::Future<Output = Result<T, ()>>,
{
    future
        .await
        .expect("called run_infallible with a fallible method")
}

pub(crate) async fn run_fallible<T, E, F>(
    future: F,
    window: Option<&gtk::ApplicationWindow>,
) -> Result<T, E>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::error::Error,
{
    let res = future.await;
    if let Err(ref e) = res {
        show_error_dialog(e, window).await;
    }
    res
}

pub trait GValueExt {
    fn unwrap<'a, T: FromValue<'a>>(&'a self) -> T;
}

impl GValueExt for glib::Value {
    fn unwrap<'a, T: FromValue<'a>>(&'a self) -> T {
        self.get().expect("Invalid type in GValue")
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use crate::{emulator::glib::glib_remote_emulator, widgets};
    use gtk::glib;
    use olympia_engine::remote::RemoteEmulator;
    use std::{
        any::Any,
        future::Future,
        panic::UnwindSafe,
        path::{Path, PathBuf},
        rc::Rc,
        sync::{Arc, Mutex, Once},
        time::{Duration, Instant},
    };

    pub struct GtkThread {
        sender: std::sync::mpsc::SyncSender<
            Box<dyn FnOnce() -> Box<dyn Any + Send + 'static> + Send + UnwindSafe>,
        >,
        receiver: std::sync::mpsc::Receiver<std::thread::Result<Box<dyn Any + Send>>>,
    }

    impl GtkThread {
        fn new() -> GtkThread {
            let (result_tx, result_rx) = std::sync::mpsc::sync_channel(0);
            let (task_tx, task_rx) = std::sync::mpsc::sync_channel(0);

            std::thread::spawn(move || {
                gtk::init().expect("Failed to init GTK");
                widgets::register();
                loop {
                    let task = task_rx.recv().expect("Failed to recieve task");
                    // Actually not guaranteed safe, but the worst that can happen is blowing up the test run
                    let result = std::panic::catch_unwind(task);
                    result_tx.send(result).expect("Failed to send result");
                }
            });

            GtkThread {
                sender: task_tx,
                receiver: result_rx,
            }
        }

        fn run<R: Any + Send + 'static, F: FnOnce() -> R + Send + UnwindSafe + 'static>(
            &self,
            f: F,
        ) -> std::thread::Result<R> {
            let inner_task = Box::new(move || {
                let result = f();
                Box::new(result) as Box<dyn Any + Send>
            });
            self.sender.send(inner_task).expect("Failed to send task");
            let result = self.receiver.recv().expect("Failed to recieve");
            result.and_then(|r| match r.downcast::<R>() {
                Ok(r) => Ok(*r),
                Err(e) => Err(e),
            })
        }
    }

    static mut GTK_MUTEX: Option<Arc<Mutex<GtkThread>>> = None;
    static SETUP_GTK_MUTEX: Once = Once::new();

    pub(crate) fn gtk_mutex() -> Arc<Mutex<GtkThread>> {
        unsafe {
            SETUP_GTK_MUTEX.call_once(|| {
                GTK_MUTEX = Some(Arc::new(Mutex::new(GtkThread::new())));
            });
            Arc::clone(GTK_MUTEX.as_ref().unwrap())
        }
    }

    pub(crate) fn with_gtk_lock<F, R>(f: F) -> R
    where
        R: Send + 'static,
        F: FnOnce() -> R + Send + UnwindSafe + 'static,
    {
        let gtk_mutex = gtk_mutex();
        let gtk_lock = gtk_mutex.lock().expect("Failed to acquire gtk lock");
        let result = gtk_lock.run(f);
        drop(gtk_lock);
        match result {
            Ok(r) => r,
            Err(e) => std::panic::resume_unwind(e),
        }
    }

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

    pub(crate) fn fizzbuzz_rom() -> Vec<u8> {
        let path = fizzbuzz_path();
        std::fs::read(path).unwrap()
    }

    /// Gets a sample remote emulator setup with no ROM loaded
    pub(crate) fn get_unloaded_remote_emu(context: glib::MainContext) -> Rc<RemoteEmulator> {
        glib_remote_emulator(context)
    }

    // Gets a sample remote emulator setup loaded with a fizzbuzz ROM
    pub(crate) fn get_loaded_remote_emu(context: glib::MainContext) -> Rc<RemoteEmulator> {
        let emu = get_unloaded_remote_emu(context.clone());

        let task = async {
            emu.load_rom(fizzbuzz_rom()).await.unwrap();
        };
        wait_for_task(&context, task);
        emu
    }

    pub(crate) fn next_tick(context: &glib::MainContext, emu: &RemoteEmulator) {
        wait_for_task(&context, emu.query_registers()).unwrap();
    }

    pub(crate) fn with_context<F: Fn(&glib::MainContext) -> ()>(f: F) {
        let context = glib::MainContext::new();
        let cguard = context.acquire();
        f(&context);
        drop(cguard);
    }

    pub(crate) fn with_unloaded_emu<F, R>(f: F)
    where
        F: FnOnce(glib::MainContext, Rc<RemoteEmulator>) -> R + Send + Sync + UnwindSafe + 'static,
        R: Send + 'static,
    {
        with_gtk_lock(|| {
            let context = glib::MainContext::new();
            let cguard = context.acquire();
            let emu = get_unloaded_remote_emu(context.clone());
            f(context.clone(), emu);
            drop(cguard);
        })
    }

    pub(crate) fn with_loaded_emu<F, R>(f: F)
    where
        F: FnOnce(glib::MainContext, Rc<RemoteEmulator>) -> R + Send + Sync + UnwindSafe + 'static,
        R: Send + 'static,
    {
        with_gtk_lock(|| {
            let context = glib::MainContext::new();
            let cguard = context.acquire();
            let emu = get_loaded_remote_emu(context.clone());
            f(context.clone(), emu);
            drop(cguard);
        })
    }
}
#[derive(Clone, Deref, From, Into, GBoxed)]
#[gboxed(type_name = "EmulatorHandle")]
pub struct EmulatorHandle(Rc<RemoteEmulator>);
