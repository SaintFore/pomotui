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

#[test]
fn taskless_reservation_failure_creates_a_real_break_without_a_session() {
    let mut service = Service::new();
    let issued = snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: false,
        },
    ))
    .reservation
    .unwrap();
    let failed = snapshot(send(
        &mut service,
        "fail",
        Command::ReservationFailure {
            id: issued.id.clone(),
            reflection: "Could not begin".into(),
            confirmed: true,
        },
    ));
    assert!(failed.reservation.is_none());
    assert_eq!(failed.action_chain.length, 0);
    assert_eq!(failed.recent_ended_chains.len(), 1);
    let chain_break = &failed.recent_ended_chains[0];
    assert_eq!(
        chain_break.break_source,
        pomotui_protocol::ChainBreakSource::ReservationReview
    );
    assert_eq!(chain_break.break_task_title, None);
    assert_eq!(chain_break.break_actual_seconds, None);
    assert!(failed.recent_history.is_empty());
    let replayed = snapshot(send(
        &mut service,
        "fail",
        Command::ReservationFailure {
            id: issued.id.clone(),
            reflection: "Could not begin".into(),
            confirmed: true,
        },
    ));
    assert_eq!(replayed.recent_ended_chains.len(), 1);
    assert!(matches!(
        send(
            &mut service,
            "again",
            Command::ReservationFailure {
                id: issued.id,
                reflection: "again".into(),
                confirmed: true,
            }
        ),
        Response::Error { .. }
    ));
}

#[test]
fn failure_requires_confirmation_without_consuming_the_reservation() {
    let mut service = Service::new();
    let issued = snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: false,
        },
    ))
    .reservation
    .unwrap();
    for (key, reflection, confirmed) in [
        ("unconfirmed", "interrupted", false),
        ("blank", "  ", false),
    ] {
        assert!(matches!(
            send(
                &mut service,
                key,
                Command::ReservationFailure {
                    id: issued.id.clone(),
                    reflection: reflection.into(),
                    confirmed,
                }
            ),
            Response::Error { .. }
        ));
        let state = snapshot(send(&mut service, "state", Command::Status));
        assert_eq!(state.reservation.unwrap().id, issued.id);
        assert!(state.recent_ended_chains.is_empty());
    }
}

#[test]
fn failure_commit_failure_preserves_reservation_chain_and_rewards_across_restart() {
    use pomotui_platform::install_persistent_save_state_failure_trigger;
    let root = std::env::temp_dir().join(format!(
        "pomotui-reservation-failure-{}",
        pomotui_sync::RecordId::random()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite3");
    let mut service = Service::open(&path).unwrap();
    let issued = snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: false,
        },
    ))
    .reservation
    .unwrap();
    install_persistent_save_state_failure_trigger(&path).unwrap();
    assert!(matches!(
        send(
            &mut service,
            "fail",
            Command::ReservationFailure {
                id: issued.id.clone(),
                reflection: "interrupted".into(),
                confirmed: true
            }
        ),
        Response::Error { .. }
    ));
    let state = snapshot(send(&mut service, "state", Command::Status));
    assert_eq!(state.reservation.unwrap().id, issued.id);
    assert!(state.recent_ended_chains.is_empty());
    assert!(state.recent_history.is_empty());
    drop(service);
    let mut reopened = Service::open(&path).unwrap();
    let state = snapshot(send(&mut reopened, "state-reopened", Command::Status));
    assert_eq!(state.reservation.unwrap().id, issued.id);
    assert!(state.recent_ended_chains.is_empty());
    drop(reopened);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
#[allow(clippy::too_many_lines)]
fn reservation_failure_identity_rewards_archive_and_fresh_start_converge_across_devices() {
    use pomotui_sync::{EntityId, RecordId, RecordPayload};
    let root = std::env::temp_dir().join(format!(
        "pomotui-reservation-convergence-{}",
        RecordId::random()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let paths = [root.join("a.sync"), root.join("b.sync")];
    let databases = [root.join("a.db"), root.join("b.db")];
    let mut a = Service::open(&databases[0]).unwrap();
    let mut b = Service::open(&databases[1]).unwrap();
    for (service, path) in [(&mut a, &paths[0]), (&mut b, &paths[1])] {
        service.enable_background_sync();
        assert!(matches!(
            send(
                service,
                "enable",
                Command::SyncEnable { path: path.clone() }
            ),
            Response::Data { .. }
        ));
    }
    snapshot(send(
        &mut a,
        "milestone-claimed",
        Command::RewardCreate {
            name: "Coffee".into(),
            threshold: 1,
            budget: None,
        },
    ));
    snapshot(send(
        &mut a,
        "milestone-unclaimed",
        Command::RewardCreate {
            name: "Walk".into(),
            threshold: 1,
            budget: None,
        },
    ));
    // Import a genuine ended Session and its explicitly submitted review.
    let session = EntityId::random();
    let review = EntityId::random();
    let records = vec![
        pomotui_sync::Record::new(
            RecordId::random(),
            session.clone(),
            pomotui_sync::MutationInstant::from_millis(1000).unwrap(),
            RecordPayload::SessionEnded {
                ended_at: 1,
                kind: pomotui_sync::SessionKind::Focus,
                outcome: pomotui_sync::SessionOutcome::Stopped,
                planned_seconds: 60,
                actual_seconds: 30,
                task_entity_id: None,
                task_title: None,
            },
        ),
        pomotui_sync::Record::new(
            RecordId::random(),
            review,
            pomotui_sync::MutationInstant::from_millis(1000).unwrap(),
            RecordPayload::SessionReviewed {
                session_entity_id: session,
                judgment: pomotui_sync::SessionReviewJudgment::Successful,
                task_entity_id: EntityId::random(),
                task_kind: pomotui_sync::ReviewedTaskKind::SystemVoid,
                task_title: "Void".into(),
                actual_seconds: 30,
                reflection: None,
                chain_entry_title: Some("Work".into()),
            },
        ),
    ];
    a.apply_sync_records(&paths[0], &records).unwrap();
    let before = snapshot(send(&mut a, "before", Command::Status));
    assert_eq!(before.action_chain.length, 1);
    let claimed = before
        .current_chain_rewards
        .iter()
        .find(|reward| reward.name == "Coffee")
        .unwrap()
        .id;
    snapshot(send(
        &mut a,
        "claim",
        Command::RewardClaim { unlock_id: claimed },
    ));
    b.apply_sync_document(&paths[1], &a.sync_document_for(&paths[0]).unwrap())
        .unwrap();
    let b_reservation = snapshot(send(
        &mut b,
        "reserve-b",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: false,
        },
    ))
    .reservation
    .unwrap();
    let issued = snapshot(send(
        &mut a,
        "reserve-a",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: false,
        },
    ))
    .reservation
    .unwrap();
    let preview = send(
        &mut a,
        "preview",
        Command::ReservationFailure {
            id: issued.id.clone(),
            reflection: String::new(),
            confirmed: false,
        },
    );
    assert!(format!("{preview:?}").contains("Walk"));
    let failed = snapshot(send(
        &mut a,
        "failure",
        Command::ReservationFailure {
            id: issued.id,
            reflection: String::new(),
            confirmed: true,
        },
    ));
    assert_eq!(failed.recent_ended_chains[0].length, 1);
    let rewards = &failed.recent_ended_chains[0].rewards;
    assert_eq!(
        rewards
            .iter()
            .find(|reward| reward.name == "Coffee")
            .unwrap()
            .state,
        "claimed"
    );
    assert_eq!(
        rewards
            .iter()
            .find(|reward| reward.name == "Walk")
            .unwrap()
            .state,
        "unavailable"
    );
    assert_eq!(failed.recent_history, before.recent_history);
    let old_document = a.sync_document_for(&paths[0]).unwrap();
    b.apply_sync_document(&paths[1], &old_document).unwrap();
    b.apply_sync_document(&paths[1], &old_document).unwrap();
    let imported = snapshot(send(&mut b, "imported", Command::Status));
    assert_eq!(imported.reservation.unwrap().id, b_reservation.id);
    assert_eq!(
        imported.recent_ended_chains[0].break_source,
        pomotui_protocol::ChainBreakSource::ReservationReview
    );
    assert_eq!(imported.recent_ended_chains[0].length, 1);
    assert_eq!(imported.recent_ended_chains[0].break_reflection, "");
    drop(b);
    b = Service::open(&databases[1]).unwrap();
    let restored = snapshot(send(&mut b, "restored", Command::Status));
    assert_eq!(restored.recent_ended_chains[0].break_reflection, "");
    b.enable_background_sync();
    let ended = &failed.recent_ended_chains[0];
    snapshot(send(
        &mut a,
        "edit",
        Command::ChainEntryEdit {
            id: ended.break_id,
            reflection: Some("New explanation".into()),
            chain_entry_title: Some("Missed appointment".into()),
        },
    ));
    b.apply_sync_document(&paths[1], &a.sync_document_for(&paths[0]).unwrap())
        .unwrap();
    let edited = snapshot(send(&mut b, "edited", Command::Status));
    assert_eq!(
        edited.recent_ended_chains[0].break_reflection,
        "New explanation"
    );
    assert_eq!(
        edited.recent_ended_chains[0]
            .break_chain_entry_title
            .as_deref(),
        Some("Missed appointment")
    );
    snapshot(send(
        &mut a,
        "delete",
        Command::EndedChainDelete { id: ended.id },
    ));
    b.apply_sync_document(&paths[1], &a.sync_document_for(&paths[0]).unwrap())
        .unwrap();
    b.apply_sync_document(&paths[1], &old_document).unwrap();
    assert!(
        snapshot(send(&mut b, "deleted", Command::Status))
            .recent_ended_chains
            .is_empty()
    );
    drop(a);
    a = Service::open(&databases[0]).unwrap();
    assert!(
        snapshot(send(&mut a, "restarted", Command::Status))
            .recent_ended_chains
            .is_empty()
    );
    snapshot(send(
        &mut a,
        "default",
        Command::ConfigureReservation { minutes: 7 },
    ));
    snapshot(send(
        &mut a,
        "reserve-reset",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: false,
        },
    ));
    let reset = snapshot(send(
        &mut a,
        "reset",
        Command::FreshStart { confirmed: true },
    ));
    assert!(reset.reservation.is_none());
    assert_eq!(reset.reservation_minutes, 7);
    b.apply_sync_document(&paths[1], &a.sync_document_for(&paths[0]).unwrap())
        .unwrap();
    b.apply_sync_document(&paths[1], &old_document).unwrap();
    let adopted = snapshot(send(&mut b, "reset-adopted", Command::Status));
    assert!(adopted.reservation.is_none());
    assert!(adopted.recent_ended_chains.is_empty());
    assert!(adopted.recent_history.is_empty());
    drop((a, b));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovery_rejects_reservations_that_cannot_be_created_or_validly_advanced() {
    use pomotui_platform::SqliteRepository;
    let root = std::env::temp_dir().join(format!(
        "pomotui-reservation-validation-{}",
        pomotui_sync::RecordId::random()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite3");
    let mut service = Service::open(&path).unwrap();
    snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: None,
            use_current_task: false,
        },
    ));
    drop(service);
    let mut repository = SqliteRepository::open(&path).unwrap();
    let valid: serde_json::Value =
        serde_json::from_str(&repository.current_session_payload().unwrap().unwrap()).unwrap();
    let invalid_fields = [
        ("duration_seconds", serde_json::json!(0)),
        ("duration_seconds", serde_json::json!(59)),
        ("remaining_seconds", serde_json::json!(601)),
        ("awaiting_review", serde_json::json!(true)),
        ("id", serde_json::json!("not-an-identity")),
        ("task_id", serde_json::json!(123)),
        ("task_title", serde_json::json!("No matching task identity")),
    ];
    for (field, value) in invalid_fields {
        let mut invalid = valid.clone();
        invalid["reservation"][field] = value;
        repository.save_state(&invalid.to_string()).unwrap();
        assert!(
            Service::open(&path).is_err(),
            "invalid reservation {field} was accepted"
        );
    }
    let mut invalid = valid.clone();
    invalid["pending_review"] = serde_json::json!({ "session_id": 999, "actual_seconds": 0, "task_id": null, "task_title": null });
    repository.save_state(&invalid.to_string()).unwrap();
    assert!(
        Service::open(&path).is_err(),
        "reservation cannot coexist with Pending Review"
    );
    let mut invalid = valid;
    invalid["timer"]["session"] = serde_json::json!({ "state": "running", "kind": "focus", "planned_seconds": 1500, "accumulated_seconds": 0, "started_at": 0, "task_id": null });
    repository.save_state(&invalid.to_string()).unwrap();
    assert!(
        Service::open(&path).is_err(),
        "reservation cannot coexist with running Focus"
    );
    drop(repository);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn recovered_pending_judgment_with_a_deleted_task_remains_valid() {
    use pomotui_platform::SqliteRepository;
    let root = std::env::temp_dir().join(format!(
        "pomotui-reservation-recovery-valid-{}",
        pomotui_sync::RecordId::random()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("state.sqlite3");
    let mut service = Service::open(&path).unwrap();
    send(
        &mut service,
        "task",
        Command::TaskCreate {
            title: "Captured deleted task".into(),
        },
    );
    let issued = snapshot(send(
        &mut service,
        "reserve",
        Command::Reserve {
            minutes: None,
            task_id: Some(1),
            use_current_task: false,
        },
    ))
    .reservation
    .unwrap();
    send(&mut service, "delete", Command::TaskDelete { id: 1 });
    drop(service);
    let mut repository = SqliteRepository::open(&path).unwrap();
    let mut saved: serde_json::Value =
        serde_json::from_str(&repository.current_session_payload().unwrap().unwrap()).unwrap();
    saved["reservation"]["remaining_seconds"] = serde_json::json!(0);
    saved["reservation"]["awaiting_review"] = serde_json::json!(true);
    repository.save_state(&saved.to_string()).unwrap();
    let mut service = Service::open(&path).unwrap();
    let status = snapshot(send(&mut service, "status", Command::Status));
    assert_eq!(
        status.reservation.unwrap().state,
        pomotui_protocol::ReservationStatus::AwaitingReview
    );
    let started = snapshot(send(
        &mut service,
        "success",
        Command::ReservationSuccess { id: issued.id },
    ));
    assert_eq!(started.state, "running");
    assert_eq!(started.current_task_id, None);
    drop(service);
    drop(repository);
    std::fs::remove_dir_all(root).unwrap();
}
