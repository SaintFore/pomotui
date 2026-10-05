use pomotui_protocol::{Command, Handler, PROTOCOL_VERSION, Request, Response};
use pomotui_service::{Service, SyncFileAdapter, SyncServiceHandler, SyncWorker};
use pomotui_sync::{
    Document, EntityId, MutationInstant, Record, RecordId, RecordPayload, TaskStatus,
};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

fn request(key: Option<&str>, command: Command) -> Request {
    Request {
        version: PROTOCOL_VERSION,
        idempotency_key: key.map(str::to_owned),
        command,
    }
}

fn test_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pomotui-worker-{label}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("test directory");
    root
}

fn configured_service(root: &std::path::Path, path: &std::path::Path) -> Arc<Mutex<Service>> {
    let mut service = Service::open(&root.join("service.sqlite3")).expect("service");
    service.enable_background_sync();
    service.handle(request(
        Some("enable"),
        Command::SyncEnable { path: path.into() },
    ));
    Arc::new(Mutex::new(service))
}

#[test]
fn worker_synchronizes_once_when_it_starts() {
    let root = test_root("startup");
    let path = root.join("pomotui.sync");
    let task_entity = EntityId::parse("00000000-0000-0000-0000-000000000002").expect("entity");
    let remote_task = Record::new(
        RecordId::parse("00000000-0000-0000-0000-000000000001").expect("record"),
        task_entity.clone(),
        MutationInstant::from_millis(1_000).expect("instant"),
        RecordPayload::TaskVersion {
            title: "Imported on startup".into(),
            status: TaskStatus::Open,
        },
    );
    let remote_session = Record::new(
        RecordId::parse("00000000-0000-0000-0000-000000000003").expect("record"),
        EntityId::parse("00000000-0000-0000-0000-000000000004").expect("entity"),
        MutationInstant::from_millis(2_000).expect("instant"),
        RecordPayload::SessionEnded {
            ended_at: 1,
            kind: pomotui_sync::SessionKind::Focus,
            outcome: pomotui_sync::SessionOutcome::Stopped,
            planned_seconds: 60,
            actual_seconds: 1,
            task_entity_id: Some(task_entity),
            task_title: Some("Imported on startup".into()),
        },
    );
    std::fs::write(
        &path,
        Document::new(&[remote_task, remote_session])
            .and_then(|document| document.to_json())
            .expect("document"),
    )
    .expect("sync file");
    let service = configured_service(&root, &path);

    let worker =
        SyncWorker::start(Arc::clone(&service), Duration::from_mins(1)).expect("sync worker");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let response = service
            .lock()
            .expect("service lock")
            .handle(request(None, Command::TaskList));
        let Response::Data { value } = response else {
            panic!("Task list response");
        };
        if value
            .as_array()
            .expect("tasks")
            .iter()
            .any(|task| task["title"] == "Imported on startup")
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "startup synchronization timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    let Response::Data { value: status } = service
        .lock()
        .expect("service")
        .handle(request(None, Command::SyncStatus))
    else {
        panic!("sync status");
    };
    assert!(
        status["warning"]
            .as_str()
            .is_some_and(|warning| warning.contains("timestamp more than one year"))
    );
    worker.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn invalid_first_document_does_not_durably_enable_synchronization() {
    let root = test_root("invalid-first-document");
    let path = root.join("pomotui.sync");
    std::fs::write(&path, "truncated").expect("invalid sync file");
    let service = configured_service(&root, &path);
    let worker =
        SyncWorker::start(Arc::clone(&service), Duration::from_mins(1)).expect("sync worker");

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let Response::Data { value: status } = service
            .lock()
            .expect("service")
            .handle(request(None, Command::SyncStatus))
        else {
            panic!("sync status");
        };
        if status["last_error_stage"] == "validate" {
            break;
        }
        assert!(Instant::now() < deadline, "validation attempt timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    worker.shutdown();
    drop(service);

    let mut restarted = Service::open(&root.join("service.sqlite3")).expect("restart service");
    let Response::Data { value: status } = restarted.handle(request(None, Command::SyncStatus))
    else {
        panic!("restarted sync status");
    };
    assert_eq!(status["enabled"], false);
    assert!(
        std::fs::read_dir(&root)
            .expect("test directory")
            .filter_map(Result::ok)
            .all(|entry| !entry
                .file_name()
                .to_string_lossy()
                .contains("initial-sync-export"))
    );

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn worker_synchronizes_again_on_the_fixed_interval() {
    let root = test_root("interval");
    let path = root.join("pomotui.sync");
    std::fs::write(
        &path,
        Document::new(&[])
            .and_then(|document| document.to_json())
            .expect("empty document"),
    )
    .expect("sync file");
    let service = configured_service(&root, &path);
    let worker =
        SyncWorker::start(Arc::clone(&service), Duration::from_millis(20)).expect("sync worker");
    std::thread::sleep(Duration::from_millis(40));

    let remote = Record::new(
        RecordId::parse("00000000-0000-0000-0000-000000000011").expect("record"),
        EntityId::parse("00000000-0000-0000-0000-000000000012").expect("entity"),
        MutationInstant::from_millis(2_000).expect("instant"),
        RecordPayload::TaskVersion {
            title: "Imported on interval".into(),
            status: TaskStatus::Open,
        },
    );
    std::fs::write(
        &path,
        Document::new(&[remote])
            .and_then(|document| document.to_json())
            .expect("remote document"),
    )
    .expect("replace sync file");

    wait_for_task(&service, "Imported on interval");
    worker.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

fn wait_for_task(service: &Arc<Mutex<Service>>, title: &str) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let response = service
            .lock()
            .expect("service lock")
            .handle(request(None, Command::TaskList));
        let Response::Data { value } = response else {
            panic!("Task list response");
        };
        if value
            .as_array()
            .expect("tasks")
            .iter()
            .any(|task| task["title"] == title)
        {
            return;
        }
        assert!(Instant::now() < deadline, "synchronization timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct AlwaysChangingFile {
    reads: Mutex<usize>,
}

impl SyncFileAdapter for AlwaysChangingFile {
    fn read(&self, _path: &std::path::Path) -> Result<Option<String>, String> {
        let mut reads = self.reads.lock().expect("reads");
        *reads += 1;
        let suffix = u128::try_from(*reads).expect("read count") + 100;
        let record = Record::new(
            RecordId::parse(&format!("00000000-0000-0000-0000-{suffix:012x}")).expect("record"),
            EntityId::parse(&format!("00000000-0000-0000-0000-{:012x}", suffix + 100))
                .expect("entity"),
            MutationInstant::from_millis(i64::try_from(*reads).expect("instant") * 1_000)
                .expect("instant"),
            RecordPayload::TaskVersion {
                title: format!("Remote {reads}"),
                status: TaskStatus::Open,
            },
        );
        Document::new(&[record])
            .and_then(|document| document.to_json())
            .map(Some)
    }

    fn replace(&self, _path: &std::path::Path, _document: &str) -> Result<(), String> {
        panic!("a continuously changing source must not be replaced")
    }
}

#[test]
fn worker_replans_a_changing_source_only_three_times() {
    let root = test_root("replan");
    let path = root.join("pomotui.sync");
    let service = configured_service(&root, &path);
    let file = Arc::new(AlwaysChangingFile {
        reads: Mutex::new(0),
    });

    let worker = SyncWorker::start_with_file_adapter(
        Arc::clone(&service),
        Duration::from_mins(1),
        file.clone(),
    )
    .expect("sync worker");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let response = service
            .lock()
            .expect("service")
            .handle(request(None, Command::SyncStatus));
        let Response::Data { value } = response else {
            panic!("sync status");
        };
        if value["last_error_stage"] == "compare" {
            break;
        }
        assert!(Instant::now() < deadline, "replan attempt timed out");
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(*file.reads.lock().expect("reads"), 6);
    worker.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[derive(Default)]
struct BlockingState {
    reads: usize,
    replacements: usize,
    released: bool,
}

struct BlockingFile {
    state: Mutex<BlockingState>,
    changed: Condvar,
    document: String,
}

impl SyncFileAdapter for BlockingFile {
    fn read(&self, _path: &std::path::Path) -> Result<Option<String>, String> {
        let mut state = self.state.lock().expect("state");
        state.reads += 1;
        self.changed.notify_all();
        while state.reads == 1 && !state.released {
            state = self.changed.wait(state).expect("state");
        }
        Ok(Some(self.document.clone()))
    }

    fn replace(&self, _path: &std::path::Path, _document: &str) -> Result<(), String> {
        let mut state = self.state.lock().expect("state");
        state.replacements += 1;
        self.changed.notify_all();
        Ok(())
    }
}

#[test]
fn overlapping_requests_coalesce_into_one_later_run() {
    let root = test_root("coalesce");
    let path = root.join("pomotui.sync");
    let service = configured_service(&root, &path);
    let file = Arc::new(BlockingFile {
        state: Mutex::new(BlockingState::default()),
        changed: Condvar::new(),
        document: Document::new(&[])
            .and_then(|document| document.to_json())
            .expect("document"),
    });
    let worker = SyncWorker::start_with_file_adapter(
        Arc::clone(&service),
        Duration::from_mins(1),
        file.clone(),
    )
    .expect("sync worker");
    let trigger = worker.trigger();

    let mut state = file.state.lock().expect("state");
    while state.reads == 0 {
        state = file.changed.wait(state).expect("state");
    }
    let Response::Data { value: status } = service
        .lock()
        .expect("service")
        .handle(request(None, Command::SyncStatus))
    else {
        panic!("sync status");
    };
    assert_eq!(status["in_progress"], true);
    for _ in 0..20 {
        trigger.request();
    }
    state.released = true;
    file.changed.notify_all();
    while state.reads < 4 {
        state = file.changed.wait(state).expect("state");
    }
    drop(state);
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(file.state.lock().expect("state").reads, 4);
    let Response::Data { value: status } = service
        .lock()
        .expect("service")
        .handle(request(None, Command::SyncStatus))
    else {
        panic!("sync status");
    };
    assert_eq!(status["in_progress"], false);
    assert!(status["last_success"].is_number());

    drop(trigger);
    let shutdown_started = Instant::now();
    worker.shutdown();
    assert!(shutdown_started.elapsed() < Duration::from_millis(250));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn durable_mutation_through_protocol_handler_triggers_synchronization() {
    let root = test_root("mutation");
    let path = root.join("pomotui.sync");
    std::fs::write(
        &path,
        Document::new(&[])
            .and_then(|document| document.to_json())
            .expect("document"),
    )
    .expect("sync file");
    let service = configured_service(&root, &path);
    let worker =
        SyncWorker::start(Arc::clone(&service), Duration::from_mins(1)).expect("sync worker");
    let mut handler = SyncServiceHandler::new(Arc::clone(&service), worker.trigger());

    let response = handler.handle(request(
        Some("create"),
        Command::TaskCreate {
            title: "Triggered mutation".into(),
        },
    ));
    assert!(matches!(response, Response::Data { .. }));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let records = std::fs::read_to_string(&path)
            .ok()
            .and_then(|source| Document::from_json(&source).ok())
            .map_or(0, |document| document.records().len());
        if records == 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "mutation synchronization timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    drop(handler);
    worker.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn asynchronous_rebuild_creates_a_missing_file_and_reports_its_limit() {
    let root = test_root("rebuild");
    let path = root.join("missing.sync");
    let service = configured_service(&root, &path);
    let mut guard = service.lock().expect("service");
    guard.handle(request(
        Some("create"),
        Command::TaskCreate {
            title: "Locally retained".into(),
        },
    ));
    drop(guard);
    let worker =
        SyncWorker::start(Arc::clone(&service), Duration::from_mins(1)).expect("sync worker");
    let mut handler = SyncServiceHandler::new(Arc::clone(&service), worker.trigger());

    let response = handler.handle(request(Some("rebuild"), Command::SyncRebuild));
    assert!(matches!(response, Response::Data { .. }));
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let response = handler.handle(request(None, Command::SyncStatus));
        let Response::Data { value } = response else {
            panic!("sync status");
        };
        if value["warning"]
            .as_str()
            .is_some_and(|warning| warning.contains("unseen remote"))
        {
            break;
        }
        assert!(Instant::now() < deadline, "rebuild timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
    let rebuilt = std::fs::read_to_string(&path).expect("rebuilt file");
    assert_eq!(
        Document::from_json(&rebuilt)
            .expect("valid rebuilt document")
            .records()
            .len(),
        1
    );

    drop(handler);
    worker.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sync_now_imports_a_replaced_document_through_the_protocol_handler() {
    let root = test_root("now");
    let path = root.join("pomotui.sync");
    std::fs::write(
        &path,
        Document::new(&[])
            .and_then(|document| document.to_json())
            .expect("document"),
    )
    .expect("sync file");
    let service = configured_service(&root, &path);
    let worker =
        SyncWorker::start(Arc::clone(&service), Duration::from_mins(1)).expect("sync worker");
    let mut handler = SyncServiceHandler::new(Arc::clone(&service), worker.trigger());
    std::thread::sleep(Duration::from_millis(20));
    let remote = Record::new(
        RecordId::parse("00000000-0000-0000-0000-000000000021").expect("record"),
        EntityId::parse("00000000-0000-0000-0000-000000000022").expect("entity"),
        MutationInstant::from_millis(3_000).expect("instant"),
        RecordPayload::TaskVersion {
            title: "Imported now".into(),
            status: TaskStatus::Open,
        },
    );
    std::fs::write(
        &path,
        Document::new(&[remote])
            .and_then(|document| document.to_json())
            .expect("remote document"),
    )
    .expect("replace sync file");

    assert!(matches!(
        handler.handle(request(Some("now"), Command::SyncNow)),
        Response::Data { .. }
    ));
    wait_for_task(&service, "Imported now");
    let Response::Data { value: status } = handler.handle(request(None, Command::SyncStatus))
    else {
        panic!("sync status");
    };
    assert_eq!(status["enabled"], true);
    assert_eq!(status["path"], path.to_string_lossy().as_ref());
    assert_eq!(status["format_version"], 5);
    assert!(
        status["local_record_count"]
            .as_u64()
            .is_some_and(|count| count >= 1)
    );

    drop(handler);
    worker.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sync_now_while_disabled_has_a_stable_rejection() {
    let service = Arc::new(Mutex::new(Service::new()));
    service.lock().expect("service").enable_background_sync();
    let worker =
        SyncWorker::start(Arc::clone(&service), Duration::from_mins(1)).expect("sync worker");
    let mut handler = SyncServiceHandler::new(Arc::clone(&service), worker.trigger());

    let Response::Error { error } = handler.handle(request(Some("now"), Command::SyncNow)) else {
        panic!("disabled sync must fail");
    };
    assert_eq!(
        error,
        pomotui_protocol::ProtocolError::Rejected {
            message: "synchronization is not enabled".into()
        }
    );

    drop(handler);
    worker.shutdown();
}

struct ReplacedDuringWriteState {
    document: String,
    replacements: usize,
}

struct ReplacedDuringWriteFile {
    state: Mutex<ReplacedDuringWriteState>,
    changed: Condvar,
}

impl SyncFileAdapter for ReplacedDuringWriteFile {
    fn read(&self, _path: &std::path::Path) -> Result<Option<String>, String> {
        Ok(Some(self.state.lock().expect("state").document.clone()))
    }

    fn replace(&self, _path: &std::path::Path, document: &str) -> Result<(), String> {
        let mut state = self.state.lock().expect("state");
        state.replacements += 1;
        if state.replacements > 1 {
            state.document = document.into();
        }
        self.changed.notify_all();
        Ok(())
    }
}

#[test]
fn later_trigger_restores_retained_union_after_external_replacement_wins_a_write_race() {
    let root = test_root("external-replacement");
    let path = root.join("pomotui.sync");
    let empty_document = Document::new(&[])
        .and_then(|document| document.to_json())
        .expect("document");
    let service = configured_service(&root, &path);
    service.lock().expect("service").handle(request(
        Some("create"),
        Command::TaskCreate {
            title: "Must survive replacement".into(),
        },
    ));
    let file = Arc::new(ReplacedDuringWriteFile {
        state: Mutex::new(ReplacedDuringWriteState {
            document: empty_document.clone(),
            replacements: 0,
        }),
        changed: Condvar::new(),
    });
    let worker = SyncWorker::start_with_file_adapter(
        Arc::clone(&service),
        Duration::from_mins(1),
        file.clone(),
    )
    .expect("sync worker");
    let trigger = worker.trigger();

    let mut state = file.state.lock().expect("state");
    while state.replacements < 1 {
        state = file.changed.wait(state).expect("state");
    }
    assert_eq!(state.document, empty_document);
    drop(state);
    trigger.request();
    let mut state = file.state.lock().expect("state");
    while state.replacements < 2 {
        state = file.changed.wait(state).expect("state");
    }
    let restored = Document::from_json(&state.document).expect("restored document");
    assert_eq!(restored.records().len(), 1);
    drop(state);

    drop(trigger);
    worker.shutdown();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn canonical_idle_sync_preserves_the_main_file_across_startup_and_manual_attempts() {
    let root = test_root("idle-main-file");
    let path = root.join("pomotui.sync");
    let canonical = Document::new(&[])
        .and_then(|document| document.to_json())
        .expect("document");
    std::fs::write(&path, &canonical).expect("main file");
    let service = configured_service(&root, &path);
    // Establish the complete local union before observing idle attempts.
    SyncWorker::start(Arc::clone(&service), Duration::from_mins(1))
        .expect("worker")
        .shutdown();
    let bytes = std::fs::read(&path).expect("main bytes");
    let metadata = std::fs::metadata(&path).expect("main metadata");
    // Keep the original inode alive so replacement cannot reuse its identity.
    let original = std::fs::File::open(&path).expect("original main file");
    for attempt in 0..3 {
        let worker =
            SyncWorker::start(Arc::clone(&service), Duration::from_mins(1)).expect("worker");
        let mut handler = SyncServiceHandler::new(Arc::clone(&service), worker.trigger());
        assert!(!matches!(
            handler.handle(request(Some(&format!("idle-{attempt}")), Command::SyncNow)),
            Response::Error { .. }
        ));
        drop(handler);
        worker.shutdown();
        let current = std::fs::metadata(&path).expect("current metadata");
        assert_eq!(std::fs::read(&path).expect("main bytes"), bytes);
        assert_eq!(
            current.modified().expect("mtime"),
            metadata.modified().expect("original mtime")
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                current.ino(),
                original.metadata().expect("open main metadata").ino()
            );
        }
        let Response::Data { value: status } = service
            .lock()
            .expect("service")
            .handle(request(None, Command::SyncStatus))
        else {
            panic!("sync status");
        };
        assert!(status["last_success"].is_number());
        assert!(status["last_error"].is_null());
        assert_eq!(status["in_progress"], false);
    }
    drop(original);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn interval_sync_imports_canonical_records_without_replacing_the_main_file() {
    struct ObservedFile(std::sync::mpsc::Sender<()>);
    impl SyncFileAdapter for ObservedFile {
        fn read(&self, path: &std::path::Path) -> Result<Option<String>, String> {
            let result = pomotui_platform::read_sync_file(path);
            let _ = self.0.send(());
            result
        }
        fn replace(&self, path: &std::path::Path, document: &str) -> Result<(), String> {
            pomotui_platform::replace_sync_file(path, document)
        }
    }
    let root = test_root("idle-interval");
    let path = root.join("pomotui.sync");
    let record = Record::new(
        RecordId::parse("00000000-0000-0000-0000-000000000001").expect("record"),
        EntityId::parse("00000000-0000-0000-0000-000000000002").expect("entity"),
        MutationInstant::from_millis(1_000).expect("instant"),
        RecordPayload::TaskVersion {
            title: "Canonical remote task".into(),
            status: TaskStatus::Open,
        },
    );
    let canonical = Document::new(&[record])
        .and_then(|document| document.to_json())
        .expect("document");
    std::fs::write(&path, &canonical).expect("main file");
    let original = std::fs::File::open(&path).expect("main file");
    let metadata = original.metadata().expect("main metadata");
    let service = configured_service(&root, &path);
    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = SyncWorker::start_with_file_adapter(
        Arc::clone(&service),
        Duration::from_millis(10),
        Arc::new(ObservedFile(sender)),
    )
    .expect("worker");
    // Three completed source/recheck pairs cover startup and interval attempts.
    for _ in 0..6 {
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("sync read");
    }
    worker.shutdown();
    let current = std::fs::metadata(&path).expect("current metadata");
    assert_eq!(
        std::fs::read_to_string(&path).expect("main bytes"),
        canonical
    );
    assert_eq!(
        current.modified().expect("mtime"),
        metadata.modified().expect("original mtime")
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(current.ino(), metadata.ino());
    }
    let Response::Data { value: tasks } = service
        .lock()
        .expect("service")
        .handle(request(None, Command::TaskList))
    else {
        panic!("tasks");
    };
    assert!(
        tasks
            .as_array()
            .expect("tasks")
            .iter()
            .any(|task| task["title"] == "Canonical remote task")
    );
    drop(original);
    let _ = std::fs::remove_dir_all(root);
}
