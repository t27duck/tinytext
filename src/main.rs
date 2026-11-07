mod app;
mod editor;
mod fileio;
mod font;
mod session;
mod settings;

use gtk::prelude::*;
use gtk::{gio, glib};

const APP_ID: &str = "dev.tinytext.TinyText";

/// When started from a terminal, fork so the shell gets its prompt back
/// (like `gvim`). Must run before GTK/GLib start any threads. `-f` /
/// `--foreground` keeps the process attached, e.g. for debugging.
fn detach_from_terminal(args: &mut Vec<String>) {
    let before = args.len();
    args.retain(|a| a != "-f" && a != "--foreground");
    let foreground = args.len() != before;
    // GApplication prints these and exits; keep them in the foreground.
    let info = args[1..]
        .iter()
        .any(|a| a.starts_with("--help") || a == "-h" || a == "--version");
    // SAFETY: plain libc calls; the process is still single-threaded here.
    unsafe {
        let on_tty = (0..=2).any(|fd| libc::isatty(fd) == 1);
        if foreground || info || !on_tty {
            return;
        }
        match libc::fork() {
            -1 => {} // fork failed: just keep running in the foreground
            0 => {
                // New session: no controlling terminal, so closing the
                // terminal doesn't SIGHUP the editor.
                libc::setsid();
                let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR);
                if devnull >= 0 {
                    libc::dup2(devnull, 0);
                    if devnull > 2 {
                        libc::close(devnull);
                    }
                }
            }
            _ => std::process::exit(0),
        }
    }
}

fn main() -> glib::ExitCode {
    let mut args: Vec<String> = std::env::args().collect();
    detach_from_terminal(&mut args);

    let application = gtk::Application::builder()
        .application_id(APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    application.connect_startup(app::startup);
    application.connect_activate(|a| app::launch(a, Vec::new()));
    application.connect_open(|a, files, _| {
        app::launch(a, files.iter().filter_map(|f| f.path()).collect());
    });
    application.run_with_args(&args)
}
