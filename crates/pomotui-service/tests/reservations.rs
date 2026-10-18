use pomotui_protocol::{
    Command, Handler, PROTOCOL_VERSION, Request, Response, SessionKind, Snapshot,
};
use pomotui_service::Service;

fn send(service: &mut Service, key: &str, command: Command) -> Response {
    service.handle(Request {
        version: PROTOCOL_VERSION,
        idempotency_key: Some(key.into()),
        command,
    })
}
fn snapshot(response: Response) -> Snapshot {
    match response {
        Response::Snapshot { snapshot } => snapshot,
        other => panic!("{other:?}"),
    }
}
#[test]
fn reserve_then_start_focus_preserves_chain_and_uses_default_duration() {
    let mut service = Service::new();
    let reserved = snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: true,
        },
    ));
    let reservation = reserved.reservation.expect("reservation");
    assert_eq!(reservation.duration_seconds, 600);
    assert!(reservation.remaining_seconds <= 600);
    assert_eq!(reserved.state, "pending");
    let started = snapshot(send(
        &mut service,
        "start",
        Command::Start {
            kind: SessionKind::Focus,
            task_id: None,
        },
    ));
    assert!(started.reservation.is_none());
    assert_eq!(started.state, "running");
    assert_eq!(started.kind, SessionKind::Focus);
    assert_eq!(started.action_chain.length, 0);
    assert!(started.recent_history.is_empty());
}

#[test]
fn configurable_default_is_saved_without_retiming_an_issued_reservation() {
    let mut service = Service::new();
    let configured = snapshot(send(
        &mut service,
        "configure",
        Command::ConfigureReservation { minutes: 3 },
    ));
    assert_eq!(configured.reservation_minutes, 3);
    let reserved = snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: true,
        },
    ));
    assert_eq!(reserved.reservation.unwrap().duration_seconds, 180);
    let changed = snapshot(send(
        &mut service,
        "change",
        Command::ConfigureReservation { minutes: 7 },
    ));
    assert_eq!(changed.reservation.unwrap().duration_seconds, 180);
    assert_eq!(changed.reservation_minutes, 7);
    for minutes in [0, u64::MAX] {
        assert!(matches!(
            send(
                &mut service,
                &format!("invalid-{minutes}"),
                Command::ConfigureReservation { minutes }
            ),
            Response::Error { .. }
        ));
    }
}

#[test]
fn configuring_default_updates_root_toml_without_destroying_other_settings() {
    let root =
        std::env::temp_dir().join(format!("pomotui-reservation-config-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let config = root.join("config.toml");
    std::fs::write(
        &config,
        "# mine\nfocus_minutes = 50\n\n[keybindings]\nquit = 'q'\n",
    )
    .unwrap();
    let mut service = Service::new();
    service.set_configuration_path(config.clone());
    snapshot(send(
        &mut service,
        "config",
        Command::ConfigureReservation { minutes: 12 },
    ));
    let saved = std::fs::read_to_string(&config).unwrap();
    assert!(saved.contains("# mine"));
    assert!(saved.starts_with("reservation_minutes = 12\n"));
    assert!(saved.contains("quit = 'q'"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn reservation_target_is_captured_and_missing_task_does_not_block_start() {
    for (use_current_task, override_id, expected_id) in [
        (true, None, Some(1)),
        (false, Some(2), Some(2)),
        (false, None, None),
    ] {
        let mut service = Service::new();
        send(
            &mut service,
            "task1",
            Command::TaskCreate {
                title: "Selected task".into(),
            },
        );
        send(
            &mut service,
            "task2",
            Command::TaskCreate {
                title: "Other task".into(),
            },
        );
        send(
            &mut service,
            "select",
            Command::TaskSelect {
                id: 1,
                stop_current: false,
            },
        );
        let reserved = snapshot(send(
            &mut service,
            "reserve",
            Command::Reserve {
                minutes: None,
                task_id: override_id,
                use_current_task,
            },
        ));
        let reservation = reserved.reservation.unwrap();
        assert_eq!(reservation.task_id, expected_id);
        let started = snapshot(send(
            &mut service,
            "start",
            Command::ReservationSuccess {
                id: reservation.id.clone(),
            },
        ));
        assert_eq!(started.current_task_id, expected_id);
        assert!(matches!(
            send(
                &mut service,
                "duplicate-other-client",
                Command::ReservationSuccess { id: reservation.id }
            ),
            Response::Error { .. }
        ));
        assert_eq!(
            snapshot(send(&mut service, "state", Command::Status)).state,
            "running"
        );
    }
    let mut service = Service::new();
    send(
        &mut service,
        "task",
        Command::TaskCreate {
            title: "Deleted".into(),
        },
    );
    let reserved = snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: Some(1),
            use_current_task: false,
        },
    ));
    let reservation = reserved.reservation.unwrap();
    send(&mut service, "delete", Command::TaskDelete { id: 1 });
    let after = snapshot(send(&mut service, "state", Command::Status));
    assert_eq!(
        after.reservation.unwrap().task_title.as_deref(),
        Some("Deleted")
    );
    let started = snapshot(send(
        &mut service,
        "success",
        Command::ReservationSuccess { id: reservation.id },
    ));
    assert_eq!(started.current_task_id, None);
}

#[test]
fn durable_reservation_recovers_identity_and_failed_commit_keeps_pending_state() {
    use pomotui_platform::install_persistent_save_state_failure_trigger;
    let root = std::env::temp_dir().join(format!(
        "pomotui-reservation-durable-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite3");
    let mut service = Service::open(&path).unwrap();
    snapshot(send(
        &mut service,
        "config",
        Command::ConfigureReservation { minutes: 7 },
    ));
    let issued = snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: true,
        },
    ))
    .reservation
    .unwrap();
    drop(service);
    let mut service = Service::open(&path).unwrap();
    let recovered = snapshot(send(&mut service, "state", Command::Status));
    assert_eq!(recovered.reservation_minutes, 7);
    let reservation = recovered.reservation.unwrap();
    assert_eq!(reservation.id, issued.id);
    assert!(reservation.remaining_seconds <= issued.remaining_seconds);
    install_persistent_save_state_failure_trigger(&path).unwrap();
    assert!(matches!(
        send(
            &mut service,
            "start-failure",
            Command::ReservationSuccess {
                id: reservation.id.clone()
            }
        ),
        Response::Error { .. }
    ));
    let after = snapshot(send(&mut service, "state-after-failure", Command::Status));
    assert_eq!(after.state, "pending");
    assert_eq!(after.reservation.unwrap().id, reservation.id);
    assert!(after.recent_history.is_empty());
    drop(service);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn running_paused_or_pending_session_review_prevents_reservation() {
    let mut service = Service::new();
    snapshot(send(
        &mut service,
        "start",
        Command::Start {
            kind: SessionKind::Focus,
            task_id: None,
        },
    ));
    for (index, transition) in [None, Some(Command::Pause), Some(Command::StopReview)]
        .into_iter()
        .enumerate()
    {
        if let Some(command) = transition {
            snapshot(send(&mut service, &format!("transition-{index}"), command));
        }
        assert!(matches!(
            send(
                &mut service,
                &format!("reserve-{index}"),
                Command::Reserve {
                    minutes: None,
                    task_id: None,
                    use_current_task: true
                }
            ),
            Response::Error { .. }
        ));
    }
}

#[test]
fn failed_default_commit_restores_toml_and_runtime_default() {
    use pomotui_platform::install_persistent_save_state_failure_trigger;
    let root = std::env::temp_dir().join(format!(
        "pomotui-reservation-config-rollback-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite3");
    let config = root.join("config.toml");
    let original = "# retain me\nreservation_minutes = 10\n[keybindings]\nquit = 'q'\n";
    std::fs::write(&config, original).unwrap();
    let mut service = Service::open(&path).unwrap();
    service.set_configuration_path(config.clone());
    install_persistent_save_state_failure_trigger(&path).unwrap();
    assert!(matches!(
        send(
            &mut service,
            "config-failure",
            Command::ConfigureReservation { minutes: 17 }
        ),
        Response::Error { .. }
    ));
    assert_eq!(std::fs::read_to_string(config).unwrap(), original);
    assert_eq!(
        snapshot(send(&mut service, "status", Command::Status)).reservation_minutes,
        10
    );
    drop(service);
    std::fs::remove_dir_all(root).unwrap();
}
