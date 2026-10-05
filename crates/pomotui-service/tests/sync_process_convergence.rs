use pomotui_protocol::{
    Client, Command, PROTOCOL_VERSION, Request, Response, SessionKind, Snapshot,
};
use pomotui_sync::Document;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static REQUEST_ID: AtomicU64 = AtomicU64::new(1);

struct TimerService {
    root: PathBuf,
    socket: PathBuf,
    child: Option<Child>,
}

impl TimerService {
    fn start(root: PathBuf, rounds_per_cycle: u8) -> Self {
        std::fs::create_dir_all(root.join("runtime")).expect("runtime directory");
        std::fs::create_dir_all(root.join("data")).expect("data directory");
        std::fs::create_dir_all(root.join("config/pomotui")).expect("config directory");
        std::fs::write(
            root.join("config/pomotui/config.toml"),
            format!(
                "focus_minutes = 1\nshort_break_minutes = 5\nlong_break_minutes = 15\nrounds_per_cycle = {rounds_per_cycle}\nreminder_enabled = false\nvolume = 100\n"
            ),
        )
        .expect("service configuration");
        let socket = root.join("runtime/pomotui.sock");
        let log = std::fs::File::create(root.join("service.log")).expect("service log");
        let child = ProcessCommand::new(env!("CARGO_BIN_EXE_pomotui-service"))
            .env("POMOTUI_SOCKET", &socket)
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .stdout(Stdio::from(log.try_clone().expect("clone service log")))
            .stderr(Stdio::from(log))
            .spawn()
            .expect("start Timer Service");
        let mut service = Self {
            root,
            socket,
            child: Some(child),
        };
        service.wait_until_ready();
        service
    }

    fn wait_until_ready(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.socket.exists() {
                let ready = Client::connect(&self.socket).and_then(|mut client| {
                    client.request(&Request {
                        version: PROTOCOL_VERSION,
                        idempotency_key: None,
                        command: Command::Status,
                    })
                });
                if matches!(ready, Ok(Response::Snapshot { .. })) {
                    return;
                }
            }
            if let Some(status) = self
                .child
                .as_mut()
                .expect("running child")
                .try_wait()
                .expect("child status")
            {
                panic!(
                    "Timer Service exited with {status}: {}",
                    std::fs::read_to_string(self.root.join("service.log")).unwrap_or_default()
                );
            }
            assert!(Instant::now() < deadline, "Timer Service startup timed out");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn request(&self, command: Command) -> Response {
        let mut client = Client::connect(&self.socket).expect("connect to Timer Service");
        let key = command.mutates().then(|| {
            format!(
                "process-test-{}",
                REQUEST_ID.fetch_add(1, Ordering::Relaxed)
            )
        });
        client
            .request(&Request {
                version: PROTOCOL_VERSION,
                idempotency_key: key,
                command,
            })
            .expect("Timer Service response")
    }

    fn data(&self, command: Command) -> serde_json::Value {
        let Response::Data { value } = self.request(command) else {
            panic!("expected data response");
        };
        value
    }

    fn snapshot(&self) -> Snapshot {
        let Response::Snapshot { snapshot } = self.request(Command::Status) else {
            panic!("expected snapshot");
        };
        snapshot
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.kill().expect("stop Timer Service");
            child.wait().expect("reap Timer Service");
            let _ = std::fs::remove_file(&self.socket);
        }
    }

    fn restart(&mut self, rounds_per_cycle: u8) {
        self.stop();
        let mut replacement = Self::start(self.root.clone(), rounds_per_cycle);
        self.socket.clone_from(&replacement.socket);
        self.child = replacement.child.take();
    }
}

impl Drop for TimerService {
    fn drop(&mut self) {
        self.stop();
    }
}

fn test_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "pomotui-process-convergence-{}-{}",
        std::process::id(),
        REQUEST_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("test root");
    root
}

fn task_id(service: &TimerService, title: &str) -> u64 {
    service
        .data(Command::TaskList)
        .as_array()
        .expect("Task list")
        .iter()
        .find(|task| task["title"] == title)
        .and_then(|task| task["id"].as_u64())
        .unwrap_or_else(|| panic!("missing Task {title}"))
}

fn create_task(service: &TimerService, title: &str) -> u64 {
    assert!(!matches!(
        service.request(Command::TaskCreate {
            title: title.into()
        }),
        Response::Error { .. }
    ));
    task_id(service, title)
}

fn stopped_review(service: &TimerService, task: u64, reflection: &str) {
    assert!(matches!(
        service.request(Command::Start {
            kind: SessionKind::Focus,
            task_id: Some(task),
        }),
        Response::Snapshot { .. }
    ));
    std::thread::sleep(Duration::from_millis(1_100));
    assert!(matches!(
        service.request(Command::StopReview),
        Response::Snapshot { .. }
    ));
    assert!(matches!(
        service.request(Command::ReviewSuccess {
            reflection: Some(reflection.into()),
        }),
        Response::Snapshot { .. }
    ));
}

fn enable(service: &TimerService, path: &Path) {
    assert!(matches!(
        service.request(Command::SyncEnable { path: path.into() }),
        Response::Data { .. }
    ));
}

fn sync_now(service: &TimerService) {
    assert!(matches!(
        service.request(Command::SyncNow),
        Response::Data { .. }
    ));
    std::thread::sleep(Duration::from_millis(100));
    wait_until("synchronization", || {
        let status = service.data(Command::SyncStatus);
        !status["in_progress"].as_bool().unwrap_or(false)
            && status["last_error"].is_null()
            && status["last_success"].is_number()
    });
}

fn rebuild(service: &TimerService) {
    assert!(matches!(
        service.request(Command::SyncRebuild),
        Response::Data { .. }
    ));
    wait_until("synchronization rebuild", || {
        let status = service.data(Command::SyncStatus);
        !status["in_progress"].as_bool().unwrap_or(false)
            && status["last_error"].is_null()
            && status["warning"]
                .as_str()
                .is_some_and(|warning| warning.contains("unseen remote"))
    });
}

fn wait_until(label: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {label}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn normalized_shared_state(service: &TimerService) -> serde_json::Value {
    let snapshot = service.snapshot();
    let mut tasks = service
        .data(Command::TaskList)
        .as_array()
        .expect("tasks")
        .clone();
    tasks.sort_by_key(|task| task["title"].as_str().unwrap_or_default().to_owned());
    let mut history = service
        .data(Command::History)
        .as_array()
        .expect("history")
        .clone();
    for record in &mut history {
        strip_ids(record);
    }
    history.sort_by_key(|record| {
        (
            record["task_title"].as_str().unwrap_or_default().to_owned(),
            record["outcome"].as_str().unwrap_or_default().to_owned(),
        )
    });
    let mut rewards = service.data(Command::Rewards);
    strip_ids(&mut rewards);
    let mut chain = service.data(Command::ActionChainCurrent);
    strip_ids(&mut chain);
    serde_json::json!({
        "tasks": tasks.into_iter().map(|mut task| {
            task.as_object_mut().expect("Task").remove("id");
            task
        }).collect::<Vec<_>>(),
        "history": history,
        "today_focus_seconds": snapshot.today.focus_seconds,
        "chain": chain,
        "rewards": rewards,
    })
}

fn strip_ids(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => items.iter_mut().for_each(strip_ids),
        serde_json::Value::Object(fields) => {
            fields.remove("id");
            fields.remove("session_id");
            fields.remove("task_id");
            fields.remove("milestone_id");
            fields.remove("chain_id");
            fields.values_mut().for_each(strip_ids);
        }
        _ => {}
    }
}

fn assert_timer_commands_still_work(service: &TimerService) {
    assert!(!matches!(
        service.request(Command::Resume),
        Response::Error { .. }
    ));
    assert!(!matches!(
        service.request(Command::Pause),
        Response::Error { .. }
    ));
}

#[test]
#[allow(clippy::too_many_lines)]
fn two_timer_services_converge_and_recover_without_sharing_live_state() {
    let root = test_root();
    let sync_path = root.join("exchange/pomotui.sync");
    std::fs::create_dir_all(sync_path.parent().expect("exchange directory"))
        .expect("exchange directory");
    let mut first = TimerService::start(root.join("first"), 4);
    let mut second = TimerService::start(root.join("second"), 7);

    let first_task = create_task(&first, "Written on first");
    stopped_review(&first, first_task, "first reflection");
    let first_link = first.data(Command::ActionChainCurrent)["links"][0]["id"]
        .as_u64()
        .expect("first chain link");
    first.request(Command::ChainEntryEdit {
        id: first_link,
        reflection: Some("edited on first".into()),
        chain_entry_title: Some("First entry".into()),
    });
    first.request(Command::RewardCreate {
        name: "Tea break".into(),
        threshold: 1,
        budget: Some(12),
    });
    let unlock = first.data(Command::Rewards)["unlocks"][0]["id"]
        .as_u64()
        .expect("reward unlock");
    first.request(Command::RewardClaim { unlock_id: unlock });

    let second_task = create_task(&second, "Written on second");
    stopped_review(&second, second_task, "second reflection");
    assert_eq!(first.snapshot().completed_rounds, 0);
    assert_eq!(second.snapshot().completed_rounds, 0);

    enable(&first, &sync_path);
    rebuild(&first);
    let old_file = std::fs::read(&sync_path).expect("first Device document");
    enable(&second, &sync_path);
    sync_now(&second);
    sync_now(&first);
    sync_now(&second);

    assert_eq!(
        normalized_shared_state(&first),
        normalized_shared_state(&second)
    );
    assert!(first.snapshot().today.focus_seconds >= 2);
    assert_eq!(first.snapshot().completed_rounds, 0);
    assert_eq!(second.snapshot().completed_rounds, 0);
    assert_eq!(first.snapshot().rounds_per_cycle, 4);
    assert_eq!(second.snapshot().rounds_per_cycle, 7);

    let first_local_kind = first.snapshot().kind;
    assert!(!matches!(
        first.request(Command::Start {
            kind: first_local_kind,
            task_id: None,
        }),
        Response::Error { .. }
    ));
    assert!(!matches!(
        first.request(Command::Pause),
        Response::Error { .. }
    ));
    let pending_task = create_task(&second, "Pending only on second");
    second.request(Command::Start {
        kind: SessionKind::Focus,
        task_id: Some(pending_task),
    });
    std::thread::sleep(Duration::from_millis(20));
    second.request(Command::StopReview);
    wait_until("second Pending Review", || {
        second.snapshot().pending_review.is_some()
    });
    assert_eq!(first.snapshot().state, "paused");
    assert!(first.snapshot().pending_review.is_none());
    assert!(second.snapshot().pending_review.is_some());

    let before_replay = normalized_shared_state(&first);
    std::fs::write(&sync_path, &old_file).expect("overwrite with old document");
    sync_now(&first);
    sync_now(&second);
    sync_now(&first);
    let after_old_file_recovery = normalized_shared_state(&first);
    sync_now(&second);
    assert!(task_id(&first, "Written on second") > 0);
    assert_eq!(
        normalized_shared_state(&first),
        normalized_shared_state(&second)
    );
    assert_eq!(normalized_shared_state(&first), after_old_file_recovery);
    assert_ne!(normalized_shared_state(&first), before_replay);
    assert_eq!(first.snapshot().state, "paused");
    assert!(first.snapshot().pending_review.is_none());
    assert!(second.snapshot().pending_review.is_some());
    assert_eq!(first.snapshot().rounds_per_cycle, 4);
    assert_eq!(second.snapshot().rounds_per_cycle, 7);

    first.restart(4);
    assert!(task_id(&first, "Written on second") > 0);
    sync_now(&first);

    first.request(Command::SyncDisable);
    let retained_only_by_first = create_task(&first, "Restored by retained replica");
    assert!(retained_only_by_first > 0);
    std::fs::remove_file(&sync_path).expect("delete exchange file");
    rebuild(&second);
    enable(&first, &sync_path);
    sync_now(&first);
    sync_now(&second);
    assert!(task_id(&second, "Restored by retained replica") > 0);

    let valid_file = std::fs::read(&sync_path).expect("valid exchange file");
    let before_invalid_import = normalized_shared_state(&first);
    std::fs::write(&sync_path, b"not json").expect("invalid exchange file");
    first.request(Command::SyncNow);
    wait_until("invalid file error", || {
        first.data(Command::SyncStatus)["last_error_stage"] == "read"
            || first.data(Command::SyncStatus)["last_error_stage"] == "validate"
    });
    assert_eq!(normalized_shared_state(&first), before_invalid_import);
    assert_timer_commands_still_work(&first);
    let after_failure = create_task(&first, "Local command after invalid file");
    assert!(after_failure > 0);
    assert!(first.snapshot().durable_health.error.is_none());
    std::fs::write(&sync_path, valid_file).expect("restore valid exchange file");
    sync_now(&first);
    sync_now(&second);

    let inaccessible = root.join("exchange/directory-not-file");
    std::fs::create_dir_all(&inaccessible).expect("inaccessible sync target");
    enable(&first, &inaccessible);
    first.request(Command::SyncNow);
    wait_until("inaccessible file error", || {
        first.data(Command::SyncStatus)["last_error"].is_string()
    });
    assert_timer_commands_still_work(&first);
    assert!(create_task(&first, "Local command after inaccessible file") > 0);
    enable(&first, &sync_path);
    sync_now(&first);

    let document =
        Document::from_json(&std::fs::read_to_string(&sync_path).expect("final sync file"))
            .expect("valid final exchange document");
    assert!(!document.records().is_empty());

    first.stop();
    second.stop();
    std::fs::remove_dir_all(root).expect("remove test root");
}

#[test]
fn separate_replica_directories_recover_conflict_records_and_replay() {
    let root = test_root().with_extension("conflict");
    let mut first = TimerService::start(root.join("first"), 4);
    let second = TimerService::start(root.join("second"), 7);
    let first_path = root.join("first/transport/custom.sync");
    let second_path = root.join("second/transport/custom.sync");
    let task = create_task(&first, "Offline first");
    stopped_review(&first, task, "first review");
    let task = create_task(&second, "Offline second");
    stopped_review(&second, task, "second review");
    enable(&first, &first_path);
    rebuild(&first);
    enable(&second, &second_path);
    rebuild(&second);
    let first_bytes = std::fs::read(&first_path).expect("first document");
    let second_bytes = std::fs::read(&second_path).expect("second document");
    let sibling = first_path.with_file_name("custom.sync-conflict-20261005-120000-ABCDEFG.sync");
    std::fs::write(&sibling, &second_bytes).expect("deliver conflict copy");
    sync_now(&first);
    assert_eq!(
        first.data(Command::ActionChainCurrent)["links"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(sibling.exists(), "ticket 3 preserves candidates");
    let merged = std::fs::read(&first_path).expect("merged document");
    std::fs::write(&second_path, &merged).expect("deliver union");
    sync_now(&second);
    assert_eq!(
        normalized_shared_state(&first),
        normalized_shared_state(&second)
    );
    first.stop();
    std::fs::write(&first_path, first_bytes).expect("replay stale main");
    first = TimerService::start(root.join("first"), 4);
    sync_now(&first);
    assert_eq!(
        normalized_shared_state(&first),
        normalized_shared_state(&second)
    );
    first.stop();
    drop(second);
    let _ = std::fs::remove_dir_all(root);
}
