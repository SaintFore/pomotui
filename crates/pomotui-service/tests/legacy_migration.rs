use pomotui_protocol::{Command, Handler, PROTOCOL_VERSION, Request, Response};
use pomotui_service::Service;
use pomotui_sync::Document as SyncDocument;
use std::path::{Path, PathBuf};

const RELEASED_FIXTURES: [(&str, &[u8]); 2] = [
    (
        "v0.1.0",
        include_bytes!("fixtures/v0.1.0-production.sqlite3"),
    ),
    (
        "v0.2.0",
        include_bytes!("fixtures/v0.2.0-production.sqlite3"),
    ),
];

fn request(key: Option<&str>, command: Command) -> Request {
    Request {
        version: PROTOCOL_VERSION,
        idempotency_key: key.map(str::to_owned),
        command,
    }
}

fn test_root(version: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "pomotui-released-migration-{version}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ))
}

fn backups(root: &Path, stage: &str) -> Vec<PathBuf> {
    let mut paths = std::fs::read_dir(root)
        .expect("fixture test directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(&format!("backup-{stage}-")))
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

#[test]
#[allow(clippy::too_many_lines)]
fn released_databases_migrate_export_once_and_restore_independently() {
    for (version, fixture) in RELEASED_FIXTURES {
        let root = test_root(version);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture test directory");
        let database = root.join("pomotui.sqlite3");
        let sync_path = root.join("pomotui.sync");
        std::fs::write(&database, fixture).expect("copy released fixture");
        std::fs::write(
            &sync_path,
            SyncDocument::new(&[])
                .and_then(|document| document.to_json())
                .expect("empty sync document"),
        )
        .expect("create sync document");

        let mut migrated = Service::open(&database).expect("open released database");
        assert_eq!(backups(&root, "schema-migration").len(), 1);
        let schema_backup = backups(&root, "schema-migration").remove(0);
        let restore_root = root.join("isolated-restore");
        std::fs::create_dir(&restore_root).expect("isolated restore directory");
        let restored_database = restore_root.join("pomotui.sqlite3");
        std::fs::copy(schema_backup, &restored_database).expect("restore migration backup");
        let mut restored =
            Service::open(&restored_database).expect("open migration backup independently");
        let Response::Data {
            value: restored_tasks,
        } = restored.handle(request(None, Command::TaskList))
        else {
            panic!("restored Task list");
        };
        assert!(
            restored_tasks
                .as_array()
                .expect("restored Tasks")
                .iter()
                .any(|task| task["title"] == "Released open task")
        );
        drop(restored);

        let enabled = migrated.handle(request(
            Some("enable"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        assert!(matches!(enabled, Response::Data { .. }), "{enabled:?}");
        let first_document = std::fs::read_to_string(&sync_path).expect("exported sync document");
        let first_records = SyncDocument::from_json(&first_document)
            .expect("valid exported document")
            .records()
            .to_vec();
        assert!(!first_records.is_empty());
        assert_eq!(backups(&root, "initial-sync-export").len(), 1);

        drop(migrated);
        let mut restarted = Service::open(&database).expect("restart migrated database");
        let retried = restarted.handle(request(
            Some("retry"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        assert!(matches!(retried, Response::Data { .. }), "{retried:?}");
        let retry_records = SyncDocument::from_json(
            &std::fs::read_to_string(&sync_path).expect("retried sync document"),
        )
        .expect("valid retried document")
        .records()
        .to_vec();
        assert_eq!(retry_records, first_records);
        assert_eq!(backups(&root, "schema-migration").len(), 1);
        assert_eq!(backups(&root, "initial-sync-export").len(), 1);

        let fresh_database = root.join("fresh.sqlite3");
        let mut fresh = Service::open(&fresh_database).expect("fresh service");
        let imported = fresh.handle(request(
            Some("import"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        assert!(matches!(imported, Response::Data { .. }), "{imported:?}");
        let Response::Snapshot { snapshot } = fresh.handle(request(None, Command::Status)) else {
            panic!("fresh status");
        };
        assert!(
            snapshot
                .tasks
                .iter()
                .any(|task| task.title == "Released open task")
        );
        assert!(
            snapshot
                .tasks
                .iter()
                .any(|task| task.title == "Released completed task" && task.completed)
        );
        assert!(
            snapshot
                .recent_history
                .iter()
                .any(|session| { session.task_title.as_deref() == Some("Released open task") })
        );
        let ended = snapshot
            .recent_ended_chains
            .first()
            .expect("released Ended Chain");
        assert_eq!(ended.links.len(), 1);
        assert_eq!(
            ended.links[0].reflection.as_deref(),
            Some("Released success reflection")
        );
        assert_eq!(ended.break_reflection, "Released failure reflection");
        assert_eq!(ended.rewards.len(), 1);
        assert_eq!(ended.rewards[0].name, "Released reward");
        assert_eq!(ended.rewards[0].state, "claimed");
        assert!(
            snapshot.pending_review.is_none(),
            "Pending Review stays local"
        );
        assert_eq!(snapshot.completed_rounds, 0, "Focus Cycle stays local");

        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn identical_released_databases_export_as_independent_histories() {
    let root = test_root("independent");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("fixture test directory");
    let mut exported = Vec::new();

    for device in ["first", "second"] {
        let database = root.join(format!("{device}.sqlite3"));
        let sync_path = root.join(format!("{device}.sync"));
        std::fs::write(&database, RELEASED_FIXTURES[0].1).expect("copy released fixture");
        std::fs::write(
            &sync_path,
            SyncDocument::new(&[])
                .and_then(|document| document.to_json())
                .expect("empty sync document"),
        )
        .expect("create sync document");
        let mut service = Service::open(&database).expect("open released database");
        let response = service.handle(request(
            Some("enable"),
            Command::SyncEnable {
                path: sync_path.clone(),
            },
        ));
        assert!(matches!(response, Response::Data { .. }), "{response:?}");
        exported.push(
            SyncDocument::from_json(
                &std::fs::read_to_string(sync_path).expect("exported sync document"),
            )
            .expect("valid exported document")
            .into_records(),
        );
    }

    let first_ids = exported[0]
        .iter()
        .map(|record| record.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(
        exported[1]
            .iter()
            .all(|record| !first_ids.contains(record.id.as_str())),
        "independent migrations must not derive identities from coincidentally equal legacy rows"
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn released_first_export_rolls_back_and_restarts_safely() {
    let root = test_root("rollback");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("fixture test directory");
    let database = root.join("pomotui.sqlite3");
    let sync_path = root.join("pomotui.sync");
    std::fs::write(&database, RELEASED_FIXTURES[0].1).expect("copy released fixture");
    let empty_document = SyncDocument::new(&[])
        .and_then(|document| document.to_json())
        .expect("empty sync document");
    std::fs::write(&sync_path, &empty_document).expect("create sync document");

    let mut service = Service::open(&database).expect("open released database");
    pomotui_platform::install_persistent_save_state_failure_trigger(&database)
        .expect("install persistence failure");
    let failed = service.handle(request(
        Some("enable"),
        Command::SyncEnable {
            path: sync_path.clone(),
        },
    ));
    assert!(matches!(failed, Response::Error { .. }), "{failed:?}");
    assert_eq!(
        std::fs::read_to_string(&sync_path).expect("unchanged sync document"),
        empty_document
    );
    drop(service);

    let connection = rusqlite::Connection::open(&database).expect("open failed migration");
    connection
        .execute("DROP TRIGGER reject_save_state", [])
        .expect("remove persistence failure");
    drop(connection);
    let mut restarted = Service::open(&database).expect("restart after rollback");
    let Response::Data { value: status } = restarted.handle(request(None, Command::SyncStatus))
    else {
        panic!("sync status after rollback");
    };
    assert_eq!(status["enabled"], false);
    let retried = restarted.handle(request(
        Some("retry"),
        Command::SyncEnable {
            path: sync_path.clone(),
        },
    ));
    assert!(matches!(retried, Response::Data { .. }), "{retried:?}");
    assert!(
        !SyncDocument::from_json(
            &std::fs::read_to_string(sync_path).expect("published sync document")
        )
        .expect("valid published document")
        .records()
        .is_empty()
    );

    let _ = std::fs::remove_dir_all(root);
}
