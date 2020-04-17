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
