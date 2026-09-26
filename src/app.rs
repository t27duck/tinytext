use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{gio, glib};

use crate::editor::Editor;
use crate::session::{self, Snapshot};
use crate::{font, settings};

const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGTERM: i32 = 15;

thread_local! {
    static EDITORS: RefCell<Vec<Rc<Editor>>> = const { RefCell::new(Vec::new()) };
    static WORD_WRAP: Cell<bool> = const { Cell::new(true) };
    static FIRST_LAUNCH: Cell<bool> = const { Cell::new(true) };
}

pub fn register(editor: &Rc<Editor>) {
    EDITORS.with(|e| e.borrow_mut().push(editor.clone()));
}

pub fn unregister(editor: &Rc<Editor>) {
    EDITORS.with(|e| e.borrow_mut().retain(|x| !Rc::ptr_eq(x, editor)));
}

fn editors() -> Vec<Rc<Editor>> {
    EDITORS.with(|e| e.borrow().clone())
}

pub fn word_wrap() -> bool {
    WORD_WRAP.get()
}

/// Runs once in the primary instance.
pub fn startup(app: &gtk::Application) {
    font::install();
    WORD_WRAP.set(settings::word_wrap());

    let new = gio::SimpleAction::new("new", None);
    let weak = app.downgrade();
    new.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            Editor::new(&app).present();
        }
    });
    app.add_action(&new);

    let wrap = gio::SimpleAction::new_stateful("wrap", None, &word_wrap().to_variant());
    wrap.connect_activate(|action, _| {
        let on = !action.state().and_then(|v| v.get::<bool>()).unwrap_or(false);
        action.set_state(&on.to_variant());
        WORD_WRAP.set(on);
        settings::set_word_wrap(on);
        for editor in editors() {
            editor.set_word_wrap(on);
        }
    });
    app.add_action(&wrap);

    let about = gio::SimpleAction::new("about", None);
    let weak = app.downgrade();
    about.connect_activate(move |_, _| {
        if let Some(app) = weak.upgrade() {
            show_about(&app);
        }
    });
    app.add_action(&about);

    app.set_accels_for_action("app.new", &["<Control>n"]);
    app.set_accels_for_action("win.open", &["<Control>o"]);
    app.set_accels_for_action("win.save", &["<Control>s"]);
    app.set_accels_for_action("win.save-as", &["<Control><Shift>s"]);
    app.set_accels_for_action("win.find", &["<Control>f"]);
    app.set_accels_for_action("win.close", &["<Control>w"]);

    // On logout/shutdown, persist unsaved buffers instead of prompting.
    for signal in [SIGHUP, SIGINT, SIGTERM] {
        let weak = app.downgrade();
        glib_unix::unix_signal_add_local(signal, move || {
            for editor in editors() {
                editor.flush_snapshot();
            }
            if let Some(app) = weak.upgrade() {
                app.quit();
            }
            glib::ControlFlow::Break
        });
    }
}

/// Handles an activation of the application, either at startup or from
/// another `tinytext` invocation.
pub fn launch(app: &gtk::Application, paths: Vec<PathBuf>) {
    if FIRST_LAUNCH.replace(false) {
        let snapshots = session::list();
        if !snapshots.is_empty() {
            prompt_restore(app, snapshots, paths);
            return;
        }
    }
    open_paths_or_new(app, &paths);
}

fn open_paths_or_new(app: &gtk::Application, paths: &[PathBuf]) {
    if paths.is_empty() {
        Editor::new(app).present();
    }
    for path in paths {
        open_path(app, path, None);
    }
}

/// Opens `path` in a window. An already-open file is focused instead;
/// `reuse` is used if it is an empty untitled window.
pub fn open_path(app: &gtk::Application, path: &Path, reuse: Option<&Rc<Editor>>) {
    let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());

    if let Some(existing) = editors().into_iter().find(|e| e.path().as_deref() == Some(&*path)) {
        existing.present();
        return;
    }

    let parent = reuse.map(|e| e.window.clone().upcast::<gtk::Window>());
    let text = match std::fs::read(&path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => Some(text),
            Err(_) => {
                show_error(
                    app,
                    parent.as_ref(),
                    &format!("Could not open “{}”", path.display()),
                    "The file is not valid UTF-8 text.",
                );
                return;
            }
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            show_error(
                app,
                parent.as_ref(),
                &format!("Could not open “{}”", path.display()),
                &err.to_string(),
            );
            return;
        }
    };

    let editor = match reuse {
        Some(e) if e.is_pristine() => e.clone(),
        _ => Editor::new(app),
    };
    editor.load(path, text.as_deref());
    editor.present();
}

pub fn show_error(app: &gtk::Application, parent: Option<&gtk::Window>, primary: &str, secondary: &str) {
    let dialog = gtk::MessageDialog::builder()
        .application(app)
        .modal(true)
        .message_type(gtk::MessageType::Error)
        .buttons(gtk::ButtonsType::Ok)
        .text(primary)
        .secondary_text(secondary)
        .build();
    dialog.set_transient_for(parent);
    dialog.connect_response(|dialog, _| dialog.destroy());
    dialog.present();
}

fn show_about(app: &gtk::Application) {
    let window = gtk::Window::builder()
        .application(app)
        .title("About tinytext")
        .modal(true)
        .resizable(false)
        .build();
    window.set_transient_for(app.active_window().as_ref());

    let icon = gtk::Image::builder()
        .icon_name("accessories-text-editor")
        .pixel_size(64)
        .build();
    let name = gtk::Label::new(None);
    name.set_markup("<span size='x-large' weight='bold'>tinytext</span>");
    let version = gtk::Label::new(Some(&format!("Version {}", env!("CARGO_PKG_VERSION"))));
    version.add_css_class("dim-label");
    let comments = gtk::Label::new(Some("A tiny plaintext editor"));

    let close = gtk::Button::with_label("Close");
    close.set_halign(gtk::Align::Center);
    close.set_margin_top(8);
    let window_ref = window.clone();
    close.connect_clicked(move |_| window_ref.close());

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(6)
        .margin_top(24)
        .margin_bottom(18)
        .margin_start(36)
        .margin_end(36)
        .build();
    content.append(&icon);
    content.append(&name);
    content.append(&version);
    content.append(&comments);
    content.append(&close);
    window.set_child(Some(&content));
    window.set_default_widget(Some(&close));

    // Escape closes the window, like a stock dialog.
    let keys = gtk::EventControllerKey::new();
    let window_ref = window.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            window_ref.close();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(keys);

    window.present();
    close.grab_focus();
}

/// Asks whether to restore unsaved windows from a previous session.
/// Closing the prompt without choosing keeps the data for next time.
fn prompt_restore(app: &gtk::Application, snapshots: Vec<Snapshot>, paths: Vec<PathBuf>) {
    let window = gtk::Window::builder()
        .application(app)
        .title("tinytext")
        .resizable(false)
        .build();

    let count = snapshots.len();
    let heading = gtk::Label::builder()
        .label(if count == 1 {
            "Restore 1 unsaved window?".to_string()
        } else {
            format!("Restore {count} unsaved windows?")
        })
        .halign(gtk::Align::Start)
        .build();
    heading.set_markup(&format!("<b>{}</b>", glib::markup_escape_text(&heading.label())));

    let names: Vec<String> = snapshots
        .iter()
        .map(|s| {
            s.path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "Untitled".to_string())
        })
        .collect();
    let details = gtk::Label::builder()
        .label(format!(
            "tinytext has unsaved work from a previous session:\n\n{}\n\nDiscarding deletes it permanently.",
            names.iter().map(|n| format!("  • {n}")).collect::<Vec<_>>().join("\n")
        ))
        .halign(gtk::Align::Start)
        .wrap(true)
        .max_width_chars(60)
        .build();

    let discard = gtk::Button::with_label("Discard");
    discard.add_css_class("destructive-action");
    let restore = gtk::Button::with_label("Restore");
    restore.add_css_class("suggested-action");
    let buttons = gtk::Box::builder()
        .spacing(8)
        .halign(gtk::Align::End)
        .margin_top(6)
        .build();
    buttons.append(&discard);
    buttons.append(&restore);

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&heading);
    content.append(&details);
    content.append(&buttons);
    window.set_child(Some(&content));
    window.set_default_widget(Some(&restore));

    let pending = Rc::new(RefCell::new(Some((snapshots, paths))));

    let (app_ref, pending_ref, window_ref) = (app.clone(), pending.clone(), window.clone());
    restore.connect_clicked(move |_| {
        let Some((snapshots, paths)) = pending_ref.take() else { return };
        for snapshot in snapshots {
            let editor = Editor::new(&app_ref);
            editor.restore(snapshot);
            editor.present();
        }
        for path in &paths {
            open_path(&app_ref, path, None);
        }
        window_ref.destroy();
    });

    let (app_ref, pending_ref, window_ref) = (app.clone(), pending.clone(), window.clone());
    discard.connect_clicked(move |_| {
        let Some((snapshots, paths)) = pending_ref.take() else { return };
        for snapshot in &snapshots {
            session::remove(&snapshot.id);
        }
        open_paths_or_new(&app_ref, &paths);
        window_ref.destroy();
    });

    let app_ref = app.clone();
    window.connect_close_request(move |_| {
        if let Some((_, paths)) = pending.take() {
            open_paths_or_new(&app_ref, &paths);
        }
        glib::Propagation::Proceed
    });

    window.present();
    restore.grab_focus();
}
