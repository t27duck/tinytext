use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Writes `text` to `path`, following symlinks. Tries an atomic
/// write-then-rename first and falls back to writing in place (e.g. when the
/// containing directory is not writable).
pub fn write_file(path: &Path, text: &str) -> io::Result<()> {
    let target = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    write_atomic(&target, text.as_bytes()).or_else(|_| fs::write(&target, text))
}

/// Writes `data` to a temporary sibling of `target`, syncs it to disk and
/// renames it over `target`, preserving the existing file's permissions.
pub fn write_atomic(target: &Path, data: &[u8]) -> io::Result<()> {
    let name = target
        .file_name()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let tmp = target.with_file_name(format!(".{}.tinytext~", name.to_string_lossy()));

    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        if let Ok(meta) = fs::metadata(target) {
            fs::set_permissions(&tmp, meta.permissions())?;
        }
        fs::rename(&tmp, target)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}
