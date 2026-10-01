use std::io::Write;
use std::path::{Path, PathBuf};

/// Reads a UTF-8 sync document, returning `None` when it does not exist.
///
/// # Errors
///
/// Returns a diagnostic filesystem error for unreadable or non-UTF-8 files.
pub fn read_sync_file(path: &Path) -> Result<Option<String>, String> {
    const MAX_SYNC_FILE_BYTES: u64 = 8 * 1024 * 1024;
    match std::fs::metadata(path) {
        Ok(metadata) if metadata.len() > MAX_SYNC_FILE_BYTES => {
            return Err(format!("sync file exceeds {MAX_SYNC_FILE_BYTES} bytes"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "cannot inspect sync file {}: {error}",
                path.display()
            ));
        }
    }
    match std::fs::read_to_string(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) => Err(format!("cannot read sync file {}: {error}", path.display())),
    }
}

/// Durably writes and validates a temporary JSON document before replacing the target.
///
/// # Errors
///
/// Returns a diagnostic error when directory creation, writing, validation, or atomic
/// replacement fails.
pub fn replace_sync_file(path: &Path, document: &str) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create sync directory {}: {error}", parent.display()))?;
    let temporary = temporary_path(path);
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| format!("cannot create temporary sync file: {error}"))?;
        file.write_all(document.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("cannot write temporary sync file: {error}"))?;
        let validation = std::fs::read_to_string(&temporary)
            .map_err(|error| format!("cannot validate temporary sync file: {error}"))?;
        pomotui_sync::Document::from_json(&validation)
            .map_err(|error| format!("temporary sync document is invalid: {error}"))?;
        if validation != document {
            return Err("temporary sync document changed while being written".into());
        }
        std::fs::rename(&temporary, path)
            .map_err(|error| format!("cannot atomically replace {}: {error}", path.display()))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn temporary_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("pomotui.sync");
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_replacement_rejects_a_semantically_invalid_sync_document() {
        let root = std::env::temp_dir().join(format!(
            "pomotui-sync-replace-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("test directory");
        let destination = root.join("pomotui.sync");
        std::fs::write(&destination, "previous valid destination").expect("existing destination");

        let error = replace_sync_file(&destination, r#"{"not":"a sync document"}"#)
            .expect_err("invalid document must not replace destination");

        assert!(error.contains("temporary sync document is invalid"));
        assert_eq!(
            std::fs::read_to_string(&destination).expect("preserved destination"),
            "previous valid destination"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
