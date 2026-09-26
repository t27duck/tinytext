mod app;
mod editor;
mod fileio;
mod font;
mod session;
mod settings;

use gtk::prelude::*;
use gtk::{gio, glib};

const APP_ID: &str = "dev.tinytext.TinyText";

fn main() -> glib::ExitCode {
    let application = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    application.connect_startup(app::startup);
    application.connect_activate(|a| app::launch(a, Vec::new()));
    application.connect_open(|a, files, _| {
        app::launch(a, files.iter().filter_map(|f| f.path()).collect());
    });
    application.run()
}
