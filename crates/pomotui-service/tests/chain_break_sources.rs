use pomotui_protocol::{
    ChainBreakSource, Command, Handler, PROTOCOL_VERSION, Request, Response, SessionKind, Snapshot,
};
use pomotui_service::Service;
use pomotui_sync::RecordId;

fn command(service: &mut Service, command: Command) -> Response {
    let response = service.handle(Request {
        version: PROTOCOL_VERSION,
        idempotency_key: Some(RecordId::random().as_str().into()),
        command,
    });
    assert!(!matches!(response, Response::Error { .. }), "{response:?}");
    response
}

fn snapshot(service: &mut Service) -> Snapshot {
    let Response::Snapshot { snapshot } = command(service, Command::Status) else {
        panic!("status")
    };
    snapshot
}

#[test]
#[allow(clippy::too_many_lines)]
fn session_break_source_rewards_and_edits_survive_sync_restart_and_archive_deletion() {
    let root = std::env::temp_dir().join(format!(
        "pomotui-chain-source-{}",
        RecordId::random().as_str()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let database = root.join("first.db");
    let path = root.join("shared.sync");
    let clock = support::ManualClock::new(1_700_000_000);
    let mut first = Service::open_with_clock(&database, clock.clone()).unwrap();
    first.configure_reminder(false, None, 100);
    first
        .configure_durations(pomotui_domain::SessionDurations::new(1, 1, 1).unwrap())
        .unwrap();
    first.enable_background_sync();
    command(&mut first, Command::SyncEnable { path: path.clone() });
    for name in ["Claimed", "Unclaimed"] {
        command(
            &mut first,
            Command::RewardCreate {
                name: name.into(),
                threshold: 1,
                budget: None,
            },
        );
    }
    command(
        &mut first,
        Command::TaskCreate {
            title: "Real work".into(),
        },
    );
    command(
        &mut first,
        Command::Start {
            kind: SessionKind::Focus,
            task_id: Some(1),
        },
    );
    clock.advance(1);
    command(&mut first, Command::ReviewSuccess { reflection: None });
    let claim = snapshot(&mut first).current_chain_rewards[0].id;
    command(&mut first, Command::RewardClaim { unlock_id: claim });
    command(&mut first, Command::Skip);
    command(
        &mut first,
        Command::Start {
            kind: SessionKind::Focus,
            task_id: Some(1),
        },
    );
    clock.advance(1);
    command(
        &mut first,
        Command::ReviewFailure {
            reflection: "Interrupted".into(),
            task_id: None,
            use_void: false,
            chain_entry_title: None,
        },
    );
    let ended = snapshot(&mut first).recent_ended_chains.remove(0);
    command(
        &mut first,
        Command::ChainEntryEdit {
            id: ended.break_id,
            reflection: Some("Changed reflection".into()),
            chain_entry_title: Some("Changed title".into()),
        },
    );
    let mut second = Service::open(&root.join("second.db")).unwrap();
    second.enable_background_sync();
    command(&mut second, Command::SyncEnable { path: path.clone() });
    let records = first.sync_records_for(&path).unwrap();
    second.apply_sync_records(&path, &records).unwrap();
    second.apply_sync_records(&path, &records).unwrap();
    drop(first);
    let mut restarted = Service::open_with_clock(&database, clock).unwrap();
    for service in [&mut restarted, &mut second] {
        let state = snapshot(service);
        assert_eq!(state.action_chain.length, 0);
        assert_eq!(state.recent_ended_chains.len(), 1);
        let chain = &state.recent_ended_chains[0];
        assert_eq!(chain.length, 1);
        assert_eq!(chain.break_source, ChainBreakSource::SessionReview);
        assert_eq!(chain.break_task_title.as_deref(), Some("Real work"));
        assert!(chain.break_actual_seconds.is_some());
        assert_eq!(chain.break_reflection, "Changed reflection");
        assert_eq!(
            chain.break_chain_entry_title.as_deref(),
            Some("Changed title")
        );
        assert_eq!(
            chain
                .rewards
                .iter()
                .filter(|reward| reward.state == "claimed")
                .count(),
            1
        );
        assert_eq!(
            chain
                .rewards
                .iter()
                .filter(|reward| reward.state == "unavailable")
                .count(),
            1
        );
    }
    let id = snapshot(&mut restarted).recent_ended_chains[0].id;
    command(&mut restarted, Command::EndedChainDelete { id });
    second
        .apply_sync_records(&path, &restarted.sync_records_for(&path).unwrap())
        .unwrap();
    assert!(snapshot(&mut restarted).recent_ended_chains.is_empty());
    assert!(snapshot(&mut second).recent_ended_chains.is_empty());
    drop(restarted);
    drop(second);
    std::fs::remove_dir_all(root).unwrap();
}

mod support;

#[test]
fn stopping_focus_for_a_task_switch_replays_the_same_break_and_reward_history() {
    let root = std::env::temp_dir().join(format!(
        "pomotui-stopped-focus-{}",
        RecordId::random().as_str()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let database = root.join("first.db");
    let path = root.join("shared.sync");
    let clock = support::ManualClock::new(1_700_000_000);
    let mut first = Service::open_with_clock(&database, clock.clone()).unwrap();
    first.enable_background_sync();
    command(&mut first, Command::SyncEnable { path: path.clone() });
    first.configure_reminder(false, None, 100);
    first
        .configure_durations(pomotui_domain::SessionDurations::new(10, 1, 1).unwrap())
        .unwrap();
    for name in ["Claimed", "Unclaimed"] {
        command(
            &mut first,
            Command::RewardCreate {
                name: name.into(),
                threshold: 1,
                budget: None,
            },
        );
    }
    for title in ["Original task", "Next task"] {
        command(
            &mut first,
            Command::TaskCreate {
                title: title.into(),
            },
        );
    }
    let start = Command::Start {
        kind: SessionKind::Focus,
        task_id: Some(1),
    };
    command(&mut first, start.clone());
    clock.advance(10);
    command(&mut first, Command::ReviewSuccess { reflection: None });
    let unlock_id = snapshot(&mut first).current_chain_rewards[0].id;
    command(&mut first, Command::RewardClaim { unlock_id });
    command(&mut first, Command::Skip);
    command(&mut first, start);
    clock.advance(3);
    command(&mut first, Command::Pause);
    clock.advance(7);
    command(&mut first, Command::Resume);
    command(
        &mut first,
        Command::TaskSelect {
            id: 2,
            stop_current: true,
        },
    );
    let stopped = snapshot(&mut first);
    assert_eq!(stopped.action_chain.length, 0);
    assert!(stopped.pending_review.is_none());
    assert_eq!(stopped.recent_history[0].actual_seconds, 3);
    assert_eq!(
        stopped.recent_history[0].task_title.as_deref(),
        Some("Original task")
    );
    let records = first.sync_records_for(&path).unwrap();

    let mut second = Service::open_with_clock(&root.join("second.db"), clock.clone()).unwrap();
    second.enable_background_sync();
    command(&mut second, Command::SyncEnable { path: path.clone() });
    second.apply_sync_records(&path, &records).unwrap();
    second.apply_sync_records(&path, &records).unwrap();
    drop(first);
    let mut restarted = Service::open_with_clock(&database, clock).unwrap();
    for service in [&mut restarted, &mut second] {
        let state = snapshot(service);
        assert_eq!(state.action_chain.length, 0);
        assert_eq!(state.recent_ended_chains.len(), 1);
        let ended = &state.recent_ended_chains[0];
        assert_eq!(ended.length, 1);
        assert_eq!(ended.break_source, ChainBreakSource::FocusStopped);
        assert_eq!(ended.break_task_title.as_deref(), Some("Original task"));
        assert_eq!(ended.break_actual_seconds, Some(3));
        assert!(ended.rewards.iter().any(|reward| reward.state == "claimed"));
        assert!(
            ended
                .rewards
                .iter()
                .any(|reward| reward.state == "unavailable")
        );
    }

    let id = snapshot(&mut restarted).recent_ended_chains[0].id;
    command(&mut restarted, Command::EndedChainDelete { id });
    second
        .apply_sync_records(&path, &restarted.sync_records_for(&path).unwrap())
        .unwrap();
    assert!(snapshot(&mut second).recent_ended_chains.is_empty());
    assert_eq!(snapshot(&mut second).action_chain.length, 0);
    drop(restarted);
    drop(second);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_focus_stop_commit_preserves_the_active_session_history_and_chain() {
    let root = std::env::temp_dir().join(format!(
        "pomotui-failed-focus-stop-{}",
        RecordId::random().as_str()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let database = root.join("timer.db");
    let clock = support::ManualClock::new(1_700_000_000);
    let mut service = Service::open_with_clock(&database, clock.clone()).unwrap();
    command(
        &mut service,
        Command::Start {
            kind: SessionKind::Focus,
            task_id: None,
        },
    );
    clock.advance(2);
    pomotui_platform::install_persistent_save_state_failure_trigger(&database).unwrap();
    assert!(matches!(
        service.handle(Request {
            version: PROTOCOL_VERSION,
            idempotency_key: Some("failed-stop".into()),
            command: Command::Stop,
        }),
        Response::Error { .. }
    ));
    let state = snapshot(&mut service);
    assert_eq!(state.state, "running");
    assert!(state.recent_history.is_empty());
    assert!(state.recent_ended_chains.is_empty());
    drop(service);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stopping_paused_focus_archives_even_with_the_legacy_stop_command() {
    let clock = support::ManualClock::new(1_700_000_000);
    let mut service = Service::new_with_clock(clock.clone());
    let start = Command::Start {
        kind: SessionKind::Focus,
        task_id: None,
    };
    command(&mut service, start);
    clock.advance(3);
    command(&mut service, Command::Pause);
    clock.advance(100);
    command(&mut service, Command::StopReview);
    let stopped = snapshot(&mut service);
    assert_eq!(stopped.state, "pending");
    assert!(stopped.pending_review.is_none());
    assert_eq!(stopped.recent_ended_chains.len(), 1);
    let ended = &stopped.recent_ended_chains[0];
    assert_eq!(ended.break_source, ChainBreakSource::FocusStopped);
    assert_eq!(ended.break_actual_seconds, Some(3));
    assert_eq!(stopped.recent_history[0].outcome, "Stopped");
    assert_eq!(stopped.recent_history[0].actual_seconds, 3);
}
