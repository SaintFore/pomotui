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
    let mut first = Service::open(&database).unwrap();
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
    command(&mut first, Command::StopReview);
    command(&mut first, Command::ReviewSuccess { reflection: None });
    // Distinct source-session end times make Review Order independent of UUID ties.
    std::thread::sleep(std::time::Duration::from_secs(1));
    let claim = snapshot(&mut first).current_chain_rewards[0].id;
    command(&mut first, Command::RewardClaim { unlock_id: claim });
    command(
        &mut first,
        Command::Start {
            kind: SessionKind::Focus,
            task_id: Some(1),
        },
    );
    command(&mut first, Command::StopReview);
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
    let mut restarted = Service::open(&database).unwrap();
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
