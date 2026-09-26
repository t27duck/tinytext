//! Snapshots of unsaved buffers, kept on disk so they survive crashes and
//! reboots. Each window owns one snapshot id; a snapshot consists of
//! `<id>.txt` (buffer contents) and an optional `<id>.path` (the file the
//! buffer belongs to).

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gtk::glib;

use crate::fileio;

pub struct Snapshot {
    pub id: String,
    pub path: Option<PathBuf>,
    pub text: String,
}

fn dir() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| glib::home_dir().join(".local/state"))
        .join("tinytext/session")
}

pub fn new_id() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{nanos:024x}-{:x}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn save(id: &str, path: Option<&Path>, text: &str) -> std::io::Result<()> {
    let dir = dir();
    fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir)?;
    fileio::write_atomic(&dir.join(format!("{id}.txt")), text.as_bytes())?;
    let meta = dir.join(format!("{id}.path"));
    match path {
        Some(p) => fileio::write_atomic(&meta, p.as_os_str().as_bytes()),
        None => {
            let _ = fs::remove_file(meta);
            Ok(())
        }
    }
}

pub fn remove(id: &str) {
    let dir = dir();
    let _ = fs::remove_file(dir.join(format!("{id}.txt")));
    let _ = fs::remove_file(dir.join(format!("{id}.path")));
}

/// Returns all stored snapshots, oldest first.
pub fn list() -> Vec<Snapshot> {
    let dir = dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut snapshots = Vec::new();
    for entry in entries.flatten() {
        let file = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with('~') {
            // Leftover temporary file from an interrupted write.
            let _ = fs::remove_file(&file);
            continue;
        }
        let Some(id) = name.strip_suffix(".txt") else {
            continue;
        };
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        let path = fs::read(dir.join(format!("{id}.path")))
            .ok()
            .filter(|b| !b.is_empty())
            .map(|b| PathBuf::from(OsString::from_vec(b)));
        snapshots.push(Snapshot {
            id: id.to_string(),
            path,
            text,
        });
    }
    snapshots.sort_by(|a, b| a.id.cmp(&b.id));
    snapshots
}
