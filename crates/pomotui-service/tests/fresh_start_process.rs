use pomotui_protocol::{
    Client, Command, PROTOCOL_VERSION, Request, Response, SessionKind, Snapshot,
};
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
        "pomotui-fresh-start-process-{}-{}",
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

fn wait_until(label: &str, condition: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting for {label}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn assert_clean(service: &TimerService, rounds: u8) {
    let snapshot = service.snapshot();
    assert_eq!(snapshot.state, "pending");
    assert_eq!(snapshot.planned_seconds, 60);
    assert_eq!(snapshot.rounds_per_cycle, rounds);
    assert_eq!(snapshot.completed_rounds, 0);
    assert!(snapshot.pending_review.is_none());
    assert_eq!(snapshot.action_chain.length, 0);
    assert!(snapshot.reward_debt.is_empty());
    assert!(
        service
            .data(Command::History)
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        service
            .data(Command::TaskList)
            .as_array()
            .unwrap()
            .iter()
            .all(|task| task["title"] == "Void")
    );
}

#[test]
fn fresh_start_clears_live_business_state_and_rejects_delayed_offline_work() {
    let root = test_root();
    let path = root.join("exchange/pomotui.sync");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut first = TimerService::start(root.join("a"), 4);
    let mut second = TimerService::start(root.join("b"), 7);
    let old = create_task(&first, "old completed Task");
    stopped_review(&first, old, "old reflection");
    enable(&first, &path);
    sync_now(&first);
    let stale = std::fs::read(&path).unwrap();
    enable(&second, &path);
    sync_now(&second);
    second.request(Command::SyncDisable);
    let offline = create_task(&second, "offline old beginning");
    second.request(Command::Start {
        kind: SessionKind::Focus,
        task_id: Some(offline),
    });
    second.request(Command::Pause);
    first.request(Command::Start {
        kind: SessionKind::Focus,
        task_id: Some(old),
    });
    first.request(Command::StopReview);
    assert!(first.snapshot().pending_review.is_some());
    assert!(matches!(
        first.request(Command::FreshStart { confirmed: false }),
        Response::Error { .. }
    ));
    assert!(first.snapshot().pending_review.is_some());
    assert!(matches!(
        first.request(Command::FreshStart { confirmed: true }),
        Response::Snapshot { .. }
    ));
    assert_clean(&first, 4);
    sync_now(&first);
    enable(&second, &path);
    sync_now(&second);
    assert_clean(&second, 7);
    assert_eq!(
        second.data(Command::SyncStatus)["warning"],
        "Fresh Start received from another device"
    );
    second.restart(7);
    assert_clean(&second, 7);
    assert_eq!(
        second.data(Command::SyncStatus)["warning"],
        "Fresh Start received from another device"
    );
    std::fs::write(
        path.with_file_name("pomotui (conflicted copy).sync"),
        &stale,
    )
    .unwrap();
    std::fs::write(&path, &stale).unwrap();
    sync_now(&first);
    sync_now(&second);
    assert_clean(&first, 4);
    assert_clean(&second, 7);
    let new_task = create_task(&second, "new beginning work");
    second.request(Command::Start {
        kind: SessionKind::Focus,
        task_id: Some(new_task),
    });
    second.request(Command::Pause);
    sync_now(&second);
    sync_now(&first);
    sync_now(&second);
    assert_eq!(second.snapshot().state, "paused");
    assert_eq!(
        second.snapshot().current_task.as_deref(),
        Some("new beginning work")
    );
    first.stop();
    second.stop();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn empty_fresh_start_document_survives_copy_restart_and_rebuild() {
    let root = test_root();
    let path = root.join("exchange/pomotui.sync");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut first = TimerService::start(root.join("a"), 4);
    create_task(&first, "retired Task");
    enable(&first, &path);
    sync_now(&first);
    let stale = std::fs::read(&path).unwrap();
    first.request(Command::FreshStart { confirmed: true });
    sync_now(&first);
    first.restart(4);
    assert_clean(&first, 4);
    first.request(Command::SyncRebuild);
    sync_now(&first);
    let copy = root.join("copied.sync");
    std::fs::copy(&path, &copy).unwrap();
    let mut fresh = TimerService::start(root.join("fresh"), 8);
    create_task(&fresh, "fresh device genesis Task");
    enable(&fresh, &copy);
    sync_now(&fresh);
    assert_clean(&fresh, 8);
    std::fs::write(&copy, stale).unwrap();
    sync_now(&fresh);
    fresh.restart(8);
    assert_clean(&fresh, 8);
    first.stop();
    fresh.stop();
    std::fs::remove_dir_all(root).unwrap();
}
