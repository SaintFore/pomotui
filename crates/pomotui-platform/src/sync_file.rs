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
            .map_err(|error| format!("cannot atomically replace {}: {error}", path.display()))?;
        sync_directory(parent)
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
        if !is_conflict_name(filename, &prefix, &suffix) {
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
    // Managed quarantine copies remain candidates after interruption or a restore collision.
    let quarantine = quarantine_path(path)?;
    if quarantine.exists() {
        let _directory = private_directory(&quarantine)?;
        for (index, entry) in std::fs::read_dir(&quarantine)
            .map_err(|e| e.to_string())?
            .enumerate()
        {
            if index >= 4096 {
                return Err("cleanup recovery exceeds 4096 entries".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let filename = entry.file_name();
            let Some(filename) = filename.to_str() else {
                continue;
            };
            if is_conflict_name(filename, &prefix, &suffix) {
                candidates.push(entry.path());
            }
        }
    }
    candidates.sort();
    if candidates.len() > 32 {
        let offset = NEXT.fetch_add(32, Ordering::Relaxed) % candidates.len();
        candidates.rotate_left(offset);
        candidates.truncate(32);
    }
    Ok(candidates)
}

fn is_conflict_name(filename: &str, prefix: &str, suffix: &str) -> bool {
    let Some(rest) = filename
        .strip_prefix(prefix)
        .and_then(|v| v.strip_suffix(suffix))
    else {
        return false;
    };
    let parts: Vec<_> = rest.split('-').collect();
    parts.len() == 3
        && parts[0].len() == 8
        && parts[1].len() == 6
        && parts[0]
            .bytes()
            .chain(parts[1].bytes())
            .all(|b| b.is_ascii_digit())
        && !parts[2].is_empty()
        && parts[2].bytes().all(|b| b.is_ascii_alphanumeric())
}

fn private_directory(path: &Path) -> Result<std::fs::File, String> {
    use std::os::unix::fs::MetadataExt;
    let directory = open_directory(path)?;
    let metadata = directory.metadata().map_err(|e| e.to_string())?;
    if metadata.uid() != rustix::process::geteuid().as_raw() || metadata.mode() & 0o077 != 0 {
        return Err("unsafe cleanup quarantine ownership or permissions".into());
    }
    Ok(directory)
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

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    #[test]
    fn changed_and_replaced_candidates_survive_cleanup() {
        let root = std::env::temp_dir().join(format!("pomotui-cleanup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let main = root.join("pomotui.sync");
        let sibling = root.join("pomotui.sync-conflict-20261005-120000-DEVICE.sync");
        std::fs::write(&sibling, "observed").unwrap();
        let observed = observe_sync_candidate(&sibling).unwrap().unwrap();
        std::fs::write(&sibling, "changed").unwrap();
        assert!(cleanup_sync_candidate(&main, &observed).is_err());
        assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "changed");
        std::fs::write(&sibling, "observed").unwrap();
        let observed = observe_sync_candidate(&sibling).unwrap().unwrap();
        cleanup_sync_candidate(&main, &observed).unwrap();
        assert!(!sibling.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn replacement_at_move_and_restore_collision_leave_every_inode_named() {
        let root =
            std::env::temp_dir().join(format!("pomotui-cleanup-race-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let main = root.join("pomotui.sync");
        let sibling = root.join("pomotui.sync-conflict-20261005-120000-DEVICE.sync");
        std::fs::write(&sibling, "observed").unwrap();
        let observed = observe_sync_candidate(&sibling).unwrap().unwrap();
        let result = cleanup_with_probe(&main, &observed, &mut |stage| match stage {
            CleanupStage::Prechecked => {
                let replacement = root.join("replacement");
                std::fs::write(&replacement, "replacement facts").unwrap();
                std::fs::rename(replacement, &sibling).unwrap();
            }
            CleanupStage::Quarantined => {
                std::fs::write(&sibling, "later original").unwrap();
            }
            CleanupStage::Verified => panic!("replacement must not reach disposal"),
        });
        assert!(result.unwrap_err().contains("retained changed artifact"));
        assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "later original");
        let recovered = discover_sync_conflicts(&main).unwrap();
        assert_eq!(recovered.len(), 2);
        assert!(
            recovered
                .iter()
                .any(|path| std::fs::read_to_string(path).unwrap() == "replacement facts")
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_writer_before_verification_is_retained_and_atomic_new_original_survives() {
        use std::io::{Seek, SeekFrom};
        let root =
            std::env::temp_dir().join(format!("pomotui-cleanup-writer-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let main = root.join("pomotui.sync");
        let sibling = root.join("pomotui.sync-conflict-20261005-120000-DEVICE.sync");
        std::fs::write(&sibling, "observed").unwrap();
        let observed = observe_sync_candidate(&sibling).unwrap().unwrap();
        let mut writer = std::fs::OpenOptions::new()
            .write(true)
            .open(&sibling)
            .unwrap();
        let result = cleanup_with_probe(&main, &observed, &mut |stage| {
            if matches!(stage, CleanupStage::Quarantined) {
                writer.seek(SeekFrom::Start(0)).unwrap();
                writer.write_all(b"new facts").unwrap();
                writer.sync_all().unwrap();
            }
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read_to_string(&sibling).unwrap(), "new facts");
        let observed = observe_sync_candidate(&sibling).unwrap().unwrap();
        cleanup_with_probe(&main, &observed, &mut |stage| {
            if matches!(stage, CleanupStage::Quarantined) {
                std::fs::write(&sibling, "new pathname facts").unwrap();
            }
        })
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(&sibling).unwrap(),
            "new pathname facts"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_after_final_verification_demonstrates_the_transport_contract_limit() {
        // Rename cannot revoke an already-open writable descriptor. This deliberately
        // demonstrates why arbitrary in-place writers are outside the cleanup guarantee.
        let root =
            std::env::temp_dir().join(format!("pomotui-cleanup-limit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let main = root.join("pomotui.sync");
        let sibling = root.join("pomotui.sync-conflict-20261005-120000-DEVICE.sync");
        std::fs::write(&sibling, "observed").unwrap();
        let observed = observe_sync_candidate(&sibling).unwrap().unwrap();
        let mut writer = std::fs::OpenOptions::new()
            .write(true)
            .open(&sibling)
            .unwrap();
        cleanup_with_probe(&main, &observed, &mut |stage| {
            if matches!(stage, CleanupStage::Verified) {
                writer.write_all(b"unobserved").unwrap();
            }
        })
        .unwrap();
        assert!(!sibling.exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interrupted_quarantine_is_discovered_and_retired_after_restart() {
        let root =
            std::env::temp_dir().join(format!("pomotui-cleanup-crash-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let main = root.join("pomotui.sync");
        let sibling = root.join("pomotui.sync-conflict-20261005-120000-DEVICE.sync");
        std::fs::write(&sibling, "absorbed facts").unwrap();
        let observed = observe_sync_candidate(&sibling).unwrap().unwrap();
        let crash = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = cleanup_with_probe(&main, &observed, &mut |stage| {
                if matches!(stage, CleanupStage::Quarantined) {
                    panic!("simulated stop after durable quarantine");
                }
            });
        }));
        assert!(crash.is_err());
        assert!(!sibling.exists());
        let recovery = discover_sync_conflicts(&main).unwrap();
        assert_eq!(recovery.len(), 1);
        let recovered = observe_sync_candidate(&recovery[0]).unwrap().unwrap();
        assert_eq!(recovered.source, "absorbed facts");
        cleanup_sync_candidate(&main, &recovered).unwrap();
        assert!(discover_sync_conflicts(&main).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}

/// Exact bounded observation used only after durable import and publication.
#[derive(Clone, Debug)]
pub struct SyncCandidate {
    pub path: PathBuf,
    pub source: String,
    identity: Option<(u64, u64, u64, i64, i64, i64, i64)>,
}

impl SyncCandidate {
    /// Adapters without filesystem identity may retain, but cannot safely remove, a candidate.
    #[must_use]
    pub fn retained(path: PathBuf, source: String) -> Self {
        Self {
            path,
            source,
            identity: None,
        }
    }
}

fn signature(metadata: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    use std::os::unix::fs::MetadataExt;
    (
        metadata.dev(),
        metadata.ino(),
        metadata.nlink(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

/// Opens a bounded regular file without following a final symlink and pins its identity.
/// # Errors
/// Returns an error when a candidate is unsafe or changes during observation.
pub fn observe_sync_candidate(path: &Path) -> Result<Option<SyncCandidate>, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot observe {}: {error}", path.display())),
    };
    observe_open_file(path, file).map(Some)
}

fn observe_open_file(path: &Path, mut file: std::fs::File) -> Result<SyncCandidate, String> {
    use std::io::Read;
    let before = file.metadata().map_err(|e| e.to_string())?;
    if !before.is_file() || before.len() > 8 * 1024 * 1024 {
        return Err(format!("unsafe or oversized candidate {}", path.display()));
    }
    let mut source = String::new();
    (&mut file)
        .take(8 * 1024 * 1024 + 1)
        .read_to_string(&mut source)
        .map_err(|e| e.to_string())?;
    let after = file.metadata().map_err(|e| e.to_string())?;
    if signature(&before) != signature(&after)
        || before.len() != after.len()
        || source.len() > 8 * 1024 * 1024
    {
        return Err(format!(
            "candidate changed during observation: {}",
            path.display()
        ));
    }
    Ok(SyncCandidate {
        path: path.into(),
        source,
        identity: Some(signature(&after)),
    })
}

fn sync_directory(path: &Path) -> Result<(), String> {
    open_directory(path)?
        .sync_all()
        .map_err(|e| format!("directory fsync {}: {e}", path.display()))
}
fn open_directory(path: &Path) -> Result<std::fs::File, String> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| format!("unsafe directory {}: {e}", path.display()))
}
fn quarantine_path(main: &Path) -> Result<PathBuf, String> {
    let name = main
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid main filename")?;
    Ok(main.with_file_name(format!(".pomotui-cleanup-{name}")))
}

fn matches_observation(observed: &SyncCandidate, current: &SyncCandidate, moved: bool) -> bool {
    match (observed.identity, current.identity) {
        (Some(a), Some(b)) => {
            a.0 == b.0
                && a.1 == b.1
                && a.2 == 1
                && b.2 == 1
                && a.3 == b.3
                && a.4 == b.4
                && (moved || (a.5 == b.5 && a.6 == b.6))
                && observed.source == current.source
        }
        _ => false,
    }
}

/// Moves without overwriting another pathname. Unsupported platforms retain evidence.
fn rename_no_replace(from: &Path, to: &Path) -> Result<(), String> {
    let source_dir = open_directory(from.parent().ok_or("missing source parent")?)?;
    let target_dir = open_directory(to.parent().ok_or("missing target parent")?)?;
    rustix::fs::renameat_with(
        &source_dir,
        from.file_name().ok_or("missing source name")?,
        &target_dir,
        to.file_name().ok_or("missing target name")?,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|e| {
        format!(
            "no-replace rename {} to {}: {e}",
            from.display(),
            to.display()
        )
    })?;
    source_dir
        .sync_all()
        .and_then(|()| target_dir.sync_all())
        .map_err(|e| format!("rename directory fsync: {e}"))
}

/// Conditionally retires an absorbed conflict copy under the atomic-replacement transport contract.
/// Existing writable descriptors are not revoked by rename; arbitrary future in-place writes
/// are outside this contract. Changed copies are restored without replacing a new original.
/// # Errors
/// Returns a diagnostic and retains recoverable evidence on uncertainty.
pub fn cleanup_sync_candidate(main: &Path, observed: &SyncCandidate) -> Result<(), String> {
    cleanup_with_probe(main, observed, &mut |_| {})
}

#[derive(Clone, Copy)]
enum CleanupStage {
    Prechecked,
    Quarantined,
    Verified,
}

fn cleanup_with_probe(
    main: &Path,
    observed: &SyncCandidate,
    probe: &mut dyn FnMut(CleanupStage),
) -> Result<(), String> {
    let name = main
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid main filename")?;
    let (stem, extension) = name
        .rsplit_once('.')
        .map_or((name, ""), |(stem, ext)| (stem, ext));
    let suffix = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    let quarantine = quarantine_path(main)?;
    if !observed
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| is_conflict_name(n, &format!("{stem}.sync-conflict-"), &suffix))
        || (observed.path.parent() != main.parent()
            && observed.path.parent() != Some(quarantine.as_path()))
    {
        return Err("cleanup refused unrelated candidate path".into());
    }
    if observed.path.parent() == Some(quarantine.as_path()) {
        private_directory(&quarantine)?;
    }
    let Some(current) = observe_sync_candidate(&observed.path)? else {
        return Ok(());
    };
    if !matches_observation(observed, &current, false) {
        return Err(format!(
            "cleanup retained changed candidate {}",
            observed.path.display()
        ));
    }
    probe(CleanupStage::Prechecked);
    let quarantine = quarantine_path(main)?;
    let recovered = observed.path.parent() == Some(quarantine.as_path());
    let moved = if recovered {
        observed.path.clone()
    } else {
        use std::os::unix::fs::DirBuilderExt;
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&quarantine) {
            Ok(()) => sync_directory(main.parent().unwrap_or(Path::new(".")))?,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(e) => return Err(format!("create cleanup quarantine: {e}")),
        }
        private_directory(&quarantine)?;
        let destination = quarantine.join(
            observed
                .path
                .file_name()
                .ok_or("missing candidate filename")?,
        );
        rename_no_replace(&observed.path, &destination)?;
        destination
    };
    let directory = private_directory(&quarantine)?;
    probe(CleanupStage::Quarantined);
    let filename = moved.file_name().ok_or("missing artifact filename")?;
    let descriptor = rustix::fs::openat(
        &directory,
        filename,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|e| e.to_string())?;
    let checked = observe_open_file(&moved, std::fs::File::from(descriptor))?;
    if !matches_observation(observed, &checked, !recovered) {
        let restore = if recovered {
            Err("already in recovery quarantine".into())
        } else {
            rename_no_replace(&moved, &observed.path)
        };
        return Err(format!(
            "cleanup retained changed artifact {} (restore: {restore:?})",
            moved.display()
        ));
    }
    probe(CleanupStage::Verified);
    rustix::fs::unlinkat(&directory, filename, rustix::fs::AtFlags::empty())
        .map_err(|e| format!("cleanup unlink {}: {e}", moved.display()))?;
    directory
        .sync_all()
        .map_err(|e| format!("cleanup directory fsync: {e}"))
}

/// Confirms existing identical publication is durable before retiring conflict evidence.
/// # Errors
/// Returns a diagnostic if bytes changed or file/directory synchronization fails.
pub fn confirm_sync_publication(path: &Path, expected: &str) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    let observed = observe_open_file(path, file.try_clone().map_err(|e| e.to_string())?)?;
    if observed.source != expected {
        return Err("publication changed before durability confirmation".into());
    }
    file.sync_all()
        .map_err(|e| format!("publication fsync: {e}"))?;
    sync_directory(path.parent().unwrap_or(Path::new(".")))
}
