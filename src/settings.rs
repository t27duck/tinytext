use std::fs;
use std::path::PathBuf;

use gtk::glib;

fn file() -> PathBuf {
    glib::user_config_dir().join("tinytext/settings.conf")
}

pub fn word_wrap() -> bool {
    fs::read_to_string(file())
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("word_wrap=").map(|v| v.trim() == "true"))
        })
        .unwrap_or(true)
}

pub fn set_word_wrap(on: bool) {
    let file = file();
    if let Some(dir) = file.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(file, format!("word_wrap={on}\n"));
}
