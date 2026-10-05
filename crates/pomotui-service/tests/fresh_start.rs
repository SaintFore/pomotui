use pomotui_protocol::{Command, Handler, PROTOCOL_VERSION, Request, Response};
use pomotui_service::Service;
fn send(service: &mut Service, key: &str, command: Command) -> Response {
    service.handle(Request {
        version: PROTOCOL_VERSION,
        idempotency_key: Some(key.into()),
        command,
    })
}
#[test]
fn fresh_start_clears_business_and_keeps_replay_protection() {
    let mut service = Service::new();
    send(
        &mut service,
        "old",
        Command::TaskCreate {
            title: "retired".into(),
        },
    );
    assert!(matches!(
        send(
            &mut service,
            "reset",
            Command::FreshStart { confirmed: false }
        ),
        Response::Error { .. }
    ));
    assert!(matches!(
        send(
            &mut service,
            "reset",
            Command::FreshStart { confirmed: true }
        ),
        Response::Snapshot { .. }
    ));
    send(
        &mut service,
        "old",
        Command::TaskCreate {
            title: "new".into(),
        },
    );
    send(
        &mut service,
        "reset",
        Command::FreshStart { confirmed: true },
    );
    let Response::Data { value } = send(&mut service, "list", Command::TaskList) else {
        panic!()
    };
    let text = value.to_string();
    assert!(text.contains("new"));
    assert!(!text.contains("retired"));
}

#[test]
fn failed_atomic_reset_preserves_old_state_and_keys() {
    use pomotui_platform::{SqliteRepository, install_persistent_save_state_failure_trigger};
    let root =
        std::env::temp_dir().join(format!("pomotui-reset-transaction-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite3");
    let mut service = Service::open(&path).unwrap();
    send(
        &mut service,
        "old",
        Command::TaskCreate {
            title: "keep on failure".into(),
        },
    );
    let before = SqliteRepository::open(&path)
        .unwrap()
        .current_session_payload()
        .unwrap();
    install_persistent_save_state_failure_trigger(&path).unwrap();
    assert!(matches!(
        send(
            &mut service,
            "reset",
            Command::FreshStart { confirmed: true }
        ),
        Response::Error { .. }
    ));
    assert_eq!(
        SqliteRepository::open(&path)
            .unwrap()
            .current_session_payload()
            .unwrap(),
        before
    );
    let Response::Data { value } = send(&mut service, "list", Command::TaskList) else {
        panic!()
    };
    assert!(value.to_string().contains("keep on failure"));
    assert!(
        SqliteRepository::open(&path)
            .unwrap()
            .mutation_keys()
            .unwrap()
            .contains(&"old".into())
    );
    drop(service);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn committed_reset_cancels_queued_effects_and_clears_old_keys_across_restart() {
    use pomotui_platform::{ReminderEffectKind, SqliteRepository};
    let root = std::env::temp_dir().join(format!("pomotui-reset-effects-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite3");
    let mut service = Service::open(&path).unwrap();
    send(
        &mut service,
        "old",
        Command::TaskCreate {
            title: "old".into(),
        },
    );
    let mut repository = SqliteRepository::open(&path).unwrap();
    let payload = repository.current_session_payload().unwrap().unwrap();
    repository
        .save_completion(
            &payload,
            "retired-session",
            &[ReminderEffectKind::Notification, ReminderEffectKind::Sound],
            0,
        )
        .unwrap();
    assert_eq!(repository.pending_reminder_effects().unwrap().len(), 2);
    send(
        &mut service,
        "reset",
        Command::FreshStart { confirmed: true },
    );
    assert!(repository.pending_reminder_effects().unwrap().is_empty());
    assert_eq!(repository.mutation_keys().unwrap(), vec!["reset"]);
    drop(service);
    let mut reopened = Service::open(&path).unwrap();
    send(
        &mut reopened,
        "old",
        Command::TaskCreate {
            title: "after restart".into(),
        },
    );
    send(
        &mut reopened,
        "reset",
        Command::FreshStart { confirmed: true },
    );
    let Response::Data { value } = send(&mut reopened, "list", Command::TaskList) else {
        panic!()
    };
    assert!(value.to_string().contains("after restart"));
    drop(reopened);
    drop(repository);
    std::fs::remove_dir_all(root).unwrap();
}
