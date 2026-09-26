use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use crate::session::{self, Snapshot};
use crate::{app, fileio};

/// How long after the last edit an unsaved buffer is snapshotted to disk.
const SNAPSHOT_DELAY: Duration = Duration::from_millis(750);

type AfterSave = Box<dyn FnOnce(&Rc<Editor>)>;

/// One editor window with its buffer and the file it belongs to.
pub struct Editor {
    pub window: gtk::ApplicationWindow,
    view: gtk::TextView,
    buffer: gtk::TextBuffer,
    search_bar: gtk::SearchBar,
    search_entry: gtk::SearchEntry,
    path: RefCell<Option<PathBuf>>,
    snapshot_id: RefCell<String>,
    snapshot_timer: RefCell<Option<glib::SourceId>>,
    force_close: Cell<bool>,
    prompting: Cell<bool>,
    file_dialog: RefCell<Option<gtk::FileChooserNative>>,
}

impl Editor {
    pub fn new(application: &gtk::Application) -> Rc<Self> {
        let window = gtk::ApplicationWindow::builder()
            .application(application)
            .default_width(900)
            .default_height(650)
            .build();

        let menu_button = gtk::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&build_menu())
            .primary(true)
            .tooltip_text("Menu")
            .build();
        let header = gtk::HeaderBar::new();
        header.pack_end(&menu_button);
        window.set_titlebar(Some(&header));

        let buffer = gtk::TextBuffer::new(None);
        let view = gtk::TextView::builder()
            .buffer(&buffer)
            .monospace(true)
            .left_margin(10)
            .right_margin(10)
            .top_margin(8)
            .bottom_margin(8)
            .build();
        view.add_css_class("tinytext-editor");
        let scroller = gtk::ScrolledWindow::builder()
            .child(&view)
            .hexpand(true)
            .vexpand(true)
            .build();

        let search_entry = gtk::SearchEntry::builder()
            .placeholder_text("Find")
            .width_chars(30)
            .build();
        let search_bar = gtk::SearchBar::builder()
            .child(&search_entry)
            .show_close_button(true)
            .build();
        search_bar.connect_entry(&search_entry);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        content.append(&search_bar);
        content.append(&scroller);
        window.set_child(Some(&content));

        let editor = Rc::new(Editor {
            window,
            view,
            buffer,
            search_bar,
            search_entry,
            path: RefCell::new(None),
            snapshot_id: RefCell::new(session::new_id()),
            snapshot_timer: RefCell::new(None),
            force_close: Cell::new(false),
            prompting: Cell::new(false),
            file_dialog: RefCell::new(None),
        });
        editor.setup_actions();
        editor.setup_signals();
        editor.set_word_wrap(app::word_wrap());
        editor.update_title();
        app::register(&editor);
        editor
    }

    pub fn present(&self) {
        self.window.present();
        self.view.grab_focus();
    }

    pub fn path(&self) -> Option<PathBuf> {
        self.path.borrow().clone()
    }

    /// True for an untitled, empty window that can be reused to open a file.
    pub fn is_pristine(&self) -> bool {
        self.path.borrow().is_none() && self.buffer.char_count() == 0
    }

    /// True when closing the window would lose data.
    fn is_dirty(&self) -> bool {
        self.buffer.is_modified() && (self.path.borrow().is_some() || self.buffer.char_count() > 0)
    }

    pub fn set_word_wrap(&self, on: bool) {
        self.view.set_wrap_mode(if on {
            gtk::WrapMode::WordChar
        } else {
            gtk::WrapMode::None
        });
    }

    /// Loads file contents into this window. `text` is `None` for a file that
    /// does not exist yet.
    pub fn load(&self, path: PathBuf, text: Option<&str>) {
        self.set_contents(text.unwrap_or(""), Some(path), false);
    }

    /// Restores an unsaved buffer from a previous session.
    pub fn restore(&self, snapshot: Snapshot) {
        *self.snapshot_id.borrow_mut() = snapshot.id;
        self.set_contents(&snapshot.text, snapshot.path, true);
    }

    /// Writes any pending snapshot to disk immediately.
    pub fn flush_snapshot(&self) {
        self.cancel_snapshot_timer();
        self.write_snapshot();
    }

    fn set_contents(&self, text: &str, path: Option<PathBuf>, modified: bool) {
        *self.path.borrow_mut() = path;
        self.buffer.begin_irreversible_action();
        self.buffer.set_text(text);
        self.buffer.end_irreversible_action();
        self.buffer.place_cursor(&self.buffer.start_iter());
        self.buffer.set_modified(modified);
        self.update_title();
    }

    fn text(&self) -> String {
        let (start, end) = self.buffer.bounds();
        self.buffer.text(&start, &end, true).to_string()
    }

    fn display_name(&self) -> String {
        self.path
            .borrow()
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    fn update_title(&self) {
        let marker = if self.is_dirty() { "• " } else { "" };
        self.window
            .set_title(Some(&format!("{marker}{} — tinytext", self.display_name())));
    }

    fn setup_actions(self: &Rc<Self>) {
        self.add_action("open", |ed| ed.open_dialog());
        self.add_action("save", |ed| ed.save(None));
        self.add_action("save-as", |ed| ed.save_as(None));
        self.add_action("find", |ed| ed.show_find());
        self.add_action("close", |ed| ed.window.close());
    }

    fn add_action(self: &Rc<Self>, name: &str, f: impl Fn(&Rc<Editor>) + 'static) {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(ed) = weak.upgrade() {
                f(&ed);
            }
        });
        self.window.add_action(&action);
    }

    fn setup_signals(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.buffer.connect_changed(move |_| {
            if let Some(ed) = weak.upgrade() {
                ed.update_title();
                ed.schedule_snapshot();
            }
        });

        let weak = Rc::downgrade(self);
        self.buffer.connect_modified_changed(move |buffer| {
            if let Some(ed) = weak.upgrade() {
                ed.update_title();
                if !buffer.is_modified() {
                    ed.discard_snapshot();
                }
            }
        });

        let weak = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            let Some(ed) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if ed.prompting.get() {
                return glib::Propagation::Stop;
            }
            if ed.force_close.get() || !ed.is_dirty() {
                ed.discard_snapshot();
                return glib::Propagation::Proceed;
            }
            ed.confirm_close();
            glib::Propagation::Stop
        });

        let weak = Rc::downgrade(self);
        self.window.connect_destroy(move |_| {
            if let Some(ed) = weak.upgrade() {
                ed.cancel_snapshot_timer();
                app::unregister(&ed);
            }
        });

        self.setup_search_signals();
    }

    fn setup_search_signals(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.search_entry.connect_search_changed(move |_| {
            if let Some(ed) = weak.upgrade() {
                ed.find(true, true);
            }
        });

        let weak = Rc::downgrade(self);
        self.search_entry.connect_activate(move |_| {
            if let Some(ed) = weak.upgrade() {
                ed.find(true, false);
            }
        });

        let weak = Rc::downgrade(self);
        self.search_entry.connect_next_match(move |_| {
            if let Some(ed) = weak.upgrade() {
                ed.find(true, false);
            }
        });

        let weak = Rc::downgrade(self);
        self.search_entry.connect_previous_match(move |_| {
            if let Some(ed) = weak.upgrade() {
                ed.find(false, false);
            }
        });

        // Shift+Enter searches backwards.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, state| {
            let enter = key == gdk::Key::Return || key == gdk::Key::KP_Enter;
            if enter && state.contains(gdk::ModifierType::SHIFT_MASK) {
                if let Some(ed) = weak.upgrade() {
                    ed.find(false, false);
                }
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        self.search_entry.add_controller(keys);

        let weak = Rc::downgrade(self);
        self.search_bar.connect_search_mode_enabled_notify(move |bar| {
            if let Some(ed) = weak.upgrade() {
                if !bar.is_search_mode() {
                    ed.search_entry.remove_css_class("error");
                    ed.view.grab_focus();
                }
            }
        });
    }

    fn show_find(&self) {
        if let Some((start, end)) = self.buffer.selection_bounds() {
            let selected = self.buffer.text(&start, &end, false);
            if !selected.contains('\n') {
                self.search_entry.set_text(&selected);
            }
        }
        self.search_bar.set_search_mode(true);
        self.search_entry.grab_focus();
        self.search_entry.select_region(0, -1);
    }

    /// Selects the next (or previous) match, wrapping around the buffer.
    /// `include_current` lets the match start at the current selection, so
    /// refining the search text keeps the same match selected.
    fn find(&self, forward: bool, include_current: bool) {
        let needle = self.search_entry.text();
        if needle.is_empty() {
            self.search_entry.remove_css_class("error");
            return;
        }

        let flags = gtk::TextSearchFlags::CASE_INSENSITIVE;
        let (sel_start, sel_end) = self.buffer.selection_bounds().unwrap_or_else(|| {
            let cursor = self.buffer.iter_at_mark(&self.buffer.get_insert());
            (cursor.clone(), cursor)
        });

        let found = if forward {
            let from = if include_current { sel_start } else { sel_end };
            from.forward_search(&needle, flags, None)
                .or_else(|| self.buffer.start_iter().forward_search(&needle, flags, None))
        } else {
            sel_start
                .backward_search(&needle, flags, None)
                .or_else(|| self.buffer.end_iter().backward_search(&needle, flags, None))
        };

        match found {
            Some((start, end)) => {
                self.search_entry.remove_css_class("error");
                self.buffer.select_range(&start, &end);
                self.view
                    .scroll_to_mark(&self.buffer.get_insert(), 0.1, false, 0.0, 0.0);
            }
            None => self.search_entry.add_css_class("error"),
        }
    }

    fn open_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileChooserNative::new(
            Some("Open File"),
            Some(&self.window),
            gtk::FileChooserAction::Open,
            Some("_Open"),
            Some("_Cancel"),
        );
        dialog.set_modal(true);
        if let Some(dir) = self.path().as_deref().and_then(Path::parent) {
            let _ = dialog.set_current_folder(Some(&gio::File::for_path(dir)));
        }

        let weak = Rc::downgrade(self);
        dialog.connect_response(move |dialog, response| {
            let Some(ed) = weak.upgrade() else { return };
            ed.file_dialog.take();
            if response != gtk::ResponseType::Accept {
                return;
            }
            if let (Some(path), Some(application)) =
                (dialog.file().and_then(|f| f.path()), ed.window.application())
            {
                app::open_path(&application, &path, Some(&ed));
            }
        });
        dialog.show();
        self.file_dialog.replace(Some(dialog));
    }

    /// Saves to the current path, asking for one if the buffer is untitled.
    /// `then` runs only if the save succeeded.
    fn save(self: &Rc<Self>, then: Option<AfterSave>) {
        match self.path() {
            Some(path) => self.write_to(&path, then),
            None => self.save_as(then),
        }
    }

    fn save_as(self: &Rc<Self>, then: Option<AfterSave>) {
        let dialog = gtk::FileChooserNative::new(
            Some("Save As"),
            Some(&self.window),
            gtk::FileChooserAction::Save,
            Some("_Save"),
            Some("_Cancel"),
        );
        dialog.set_modal(true);
        match self.path() {
            Some(path) => {
                let _ = dialog.set_file(&gio::File::for_path(path));
            }
            None => dialog.set_current_name("Untitled.txt"),
        }

        let weak = Rc::downgrade(self);
        let then = RefCell::new(then);
        dialog.connect_response(move |dialog, response| {
            let Some(ed) = weak.upgrade() else { return };
            ed.file_dialog.take();
            if response != gtk::ResponseType::Accept {
                return;
            }
            if let Some(path) = dialog.file().and_then(|f| f.path()) {
                ed.write_to(&path, then.take());
            }
        });
        dialog.show();
        self.file_dialog.replace(Some(dialog));
    }

    fn write_to(self: &Rc<Self>, path: &Path, then: Option<AfterSave>) {
        if let Err(err) = fileio::write_file(path, &self.text()) {
            if let Some(application) = self.window.application() {
                app::show_error(
                    &application,
                    Some(self.window.upcast_ref()),
                    &format!("Could not save “{}”", path.display()),
                    &err.to_string(),
                );
            }
            return;
        }
        *self.path.borrow_mut() = Some(path.to_path_buf());
        self.buffer.set_modified(false);
        self.discard_snapshot();
        self.update_title();
        if let Some(then) = then {
            then(self);
        }
    }

    fn confirm_close(self: &Rc<Self>) {
        self.prompting.set(true);
        let dialog = gtk::MessageDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .message_type(gtk::MessageType::Question)
            .text(format!("Save changes to “{}” before closing?", self.display_name()))
            .secondary_text("If you don't save, your changes will be lost.")
            .build();
        dialog.add_buttons(&[
            ("Don't Save", gtk::ResponseType::Reject),
            ("Cancel", gtk::ResponseType::Cancel),
            ("Save", gtk::ResponseType::Accept),
        ]);
        dialog.set_default_response(gtk::ResponseType::Accept);
        if let Some(button) = dialog.widget_for_response(gtk::ResponseType::Accept) {
            button.add_css_class("suggested-action");
        }
        if let Some(button) = dialog.widget_for_response(gtk::ResponseType::Reject) {
            button.add_css_class("destructive-action");
        }

        let weak = Rc::downgrade(self);
        dialog.connect_response(move |dialog, response| {
            dialog.destroy();
            let Some(ed) = weak.upgrade() else { return };
            ed.prompting.set(false);
            match response {
                gtk::ResponseType::Accept => ed.save(Some(Box::new(|ed| ed.close_now()))),
                gtk::ResponseType::Reject => ed.close_now(),
                _ => {}
            }
        });
        dialog.present();
    }

    fn close_now(&self) {
        self.force_close.set(true);
        self.window.close();
    }

    fn schedule_snapshot(self: &Rc<Self>) {
        if self.snapshot_timer.borrow().is_some() {
            return;
        }
        let weak = Rc::downgrade(self);
        let source = glib::timeout_add_local_once(SNAPSHOT_DELAY, move || {
            if let Some(ed) = weak.upgrade() {
                ed.snapshot_timer.take();
                ed.write_snapshot();
            }
        });
        self.snapshot_timer.replace(Some(source));
    }

    fn cancel_snapshot_timer(&self) {
        if let Some(source) = self.snapshot_timer.take() {
            source.remove();
        }
    }

    fn write_snapshot(&self) {
        let id = self.snapshot_id.borrow().clone();
        if !self.is_dirty() {
            session::remove(&id);
            return;
        }
        let path = self.path();
        if let Err(err) = session::save(&id, path.as_deref(), &self.text()) {
            eprintln!("tinytext: could not write session snapshot: {err}");
        }
    }

    fn discard_snapshot(&self) {
        self.cancel_snapshot_timer();
        session::remove(&self.snapshot_id.borrow());
    }
}

fn build_menu() -> gio::Menu {
    let file = gio::Menu::new();
    file.append(Some("New Window"), Some("app.new"));
    file.append(Some("Open…"), Some("win.open"));
    file.append(Some("Save"), Some("win.save"));
    file.append(Some("Save As…"), Some("win.save-as"));

    let view = gio::Menu::new();
    view.append(Some("Find…"), Some("win.find"));
    view.append(Some("Word Wrap"), Some("app.wrap"));

    let about = gio::Menu::new();
    about.append(Some("About tinytext"), Some("app.about"));

    let menu = gio::Menu::new();
    menu.append_section(None, &file);
    menu.append_section(None, &view);
    menu.append_section(None, &about);
    menu
}
