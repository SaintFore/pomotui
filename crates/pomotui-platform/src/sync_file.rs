use std::io::Write;
use std::path::{Path, PathBuf};

/// Reads a UTF-8 sync document, returning `None` when it does not exist.
///
/// # Errors
///
/// Returns a diagnostic filesystem error for unreadable or non-UTF-8 files.
pub fn read_sync_file(path: &Path) -> Result<Option<String>, String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    const MAX_SYNC_FILE_BYTES: u64 = 8 * 1024 * 1024;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot open sync file {}: {error}", path.display())),
    };
    let metadata = file
        .metadata()
        .map_err(|error| format!("cannot inspect sync file {}: {error}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "unsafe sync file {}: expected a regular file",
            path.display()
        ));
    }
    if metadata.len() > MAX_SYNC_FILE_BYTES {
        return Err(format!(
            "sync file {} exceeds {MAX_SYNC_FILE_BYTES} bytes",
            path.display()
        ));
    }
    let mut source = String::new();
    file.take(MAX_SYNC_FILE_BYTES + 1)
        .read_to_string(&mut source)
        .map_err(|error| format!("cannot read sync file {}: {error}", path.display()))?;
    if source.len() as u64 > MAX_SYNC_FILE_BYTES {
        return Err(format!(
            "sync file {} grew beyond {MAX_SYNC_FILE_BYTES} bytes",
            path.display()
        ));
    }
    Ok(Some(source))
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

/// Discovers only Syncthing conflict siblings of the selected exchange filename.
/// Each attempt handles at most 32 candidates and scans at most 4096 directory entries.
///
/// # Errors
/// Returns a diagnostic for unsafe candidates or an excessive directory.
pub fn discover_sync_conflicts(path: &Path) -> Result<Vec<PathBuf>, String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "sync filename is not UTF-8".to_owned())?;
    let (stem, extension) = name
        .rsplit_once('.')
        .map_or((name, ""), |(stem, ext)| (stem, ext));
    let prefix = format!("{stem}.sync-conflict-");
    let suffix = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("cannot discover conflict siblings: {error}")),
    };
    let mut candidates = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= 4096 {
            return Err("conflict discovery exceeds 4096 directory entries".into());
        }
        let entry = entry.map_err(|error| format!("cannot inspect conflict sibling: {error}"))?;
        let filename = entry.file_name();
        let Some(filename) = filename.to_str() else {
            continue;
        };
        let Some(rest) = filename
            .strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix(&suffix))
        else {
            continue;
        };
        let parts: Vec<_> = rest.split('-').collect();
        if parts.len() != 3
            || parts[0].len() != 8
            || parts[1].len() != 6
            || !parts[0]
                .bytes()
                .chain(parts[1].bytes())
                .all(|b| b.is_ascii_digit())
            || parts[2].is_empty()
            || !parts[2].bytes().all(|b| b.is_ascii_alphanumeric())
        {
            continue;
        }
        let metadata = std::fs::symlink_metadata(entry.path())
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(format!(
                "unsafe conflict sibling {}: expected a regular file",
                entry.path().display()
            ));
        }
        candidates.push(entry.path());
    }
    candidates.sort();
    if candidates.len() > 32 {
        let offset = NEXT.fetch_add(32, Ordering::Relaxed) % candidates.len();
        candidates.rotate_left(offset);
        candidates.truncate(32);
    }
    Ok(candidates)
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

#[cfg(test)]
mod conflict_tests {
    use super::*;

    #[test]
    fn discovery_ignores_unrelated_names_bounds_batches_and_rejects_symlinks() {
        let root = std::env::temp_dir().join(format!("pomotui-discovery-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let main = root.join("custom.sync");
        std::fs::write(
            root.join("other.sync-conflict-20261005-120000-DEVICE.sync"),
            "unrelated",
        )
        .unwrap();
        std::fs::write(root.join("custom.sync-conflict-garbage.sync"), "unrelated").unwrap();
        for index in 0..40 {
            std::fs::write(
                root.join(format!(
                    "custom.sync-conflict-20261005-120000-D{index:02}.sync"
                )),
                "candidate",
            )
            .unwrap();
        }
        let first = discover_sync_conflicts(&main).unwrap();
        let second = discover_sync_conflicts(&main).unwrap();
        assert_eq!(first.len(), 32);
        assert_eq!(second.len(), 32);
        let mut union = first;
        union.extend(second);
        union.sort();
        union.dedup();
        assert_eq!(union.len(), 40);
        let unsafe_path = root.join("custom.sync-conflict-20261005-120000-SYMLINK.sync");
        std::os::unix::fs::symlink("/dev/null", &unsafe_path).unwrap();
        assert!(
            discover_sync_conflicts(&main)
                .unwrap_err()
                .contains("unsafe conflict sibling")
        );
        assert!(read_sync_file(&unsafe_path).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
