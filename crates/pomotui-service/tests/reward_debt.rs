use pomotui_protocol::{Command, Handler, PROTOCOL_VERSION, Request, Response};
use pomotui_service::Service;
use pomotui_sync::{EntityId, MutationInstant, Record, RecordId, RecordPayload};
fn id(n: u128) -> EntityId {
    EntityId::parse(&format!("00000000-0000-0000-0000-{n:012x}")).unwrap()
}
fn request(command: Command) -> Request {
    Request {
        version: PROTOCOL_VERSION,
        idempotency_key: Some(RecordId::random().as_str().into()),
        command,
    }
}
fn snapshot(service: &mut Service) -> pomotui_protocol::Snapshot {
    match service.handle(request(Command::Status)) {
        Response::Snapshot { snapshot } => snapshot,
        other => panic!("{other:?}"),
    }
}
fn success(n: u128, time: i64) -> Vec<Record> {
    vec![
        Record::new(
            RecordId::random(),
            id(n + 100),
            MutationInstant::from_millis(time).unwrap(),
            RecordPayload::SessionEnded {
                ended_at: time,
                kind: pomotui_sync::SessionKind::Focus,
                outcome: pomotui_sync::SessionOutcome::Stopped,
                planned_seconds: 60,
                actual_seconds: 60,
                task_entity_id: None,
                task_title: None,
            },
        ),
        Record::new(
            RecordId::random(),
            id(n),
            MutationInstant::from_millis(time).unwrap(),
            RecordPayload::SessionReviewed {
                session_entity_id: id(n + 100),
                judgment: pomotui_sync::SessionReviewJudgment::Successful,
                task_entity_id: id(50),
                task_kind: pomotui_sync::ReviewedTaskKind::SystemVoid,
                task_title: "Void".into(),
                actual_seconds: 60,
                reflection: None,
                chain_entry_title: Some("work".into()),
            },
        ),
    ]
}
#[test]
fn two_devices_replay_claim_debt_and_offline_repayments_after_restart() {
    let root = std::env::temp_dir().join(format!("pomotui-debt-{}", RecordId::random().as_str()));
    std::fs::create_dir_all(&root).unwrap();
    let mut records = (1..8)
        .flat_map(|n| success(n, n as i64 * 10))
        .collect::<Vec<_>>();
    for (entity, payload) in [
        (
            id(30),
            RecordPayload::RewardMilestoneVersion {
                name: "Coffee".into(),
                threshold: 7,
                budget: None,
            },
        ),
        (
            id(40),
            RecordPayload::RewardUnlocked {
                milestone_entity_id: id(30),
                previous_chain_break_review_entity_id: None,
                name: "Coffee".into(),
                threshold: 7,
                budget: None,
            },
        ),
    ] {
        records.push(Record::new(
            RecordId::random(),
            entity,
            MutationInstant::from_millis(70).unwrap(),
            payload,
        ));
    }
    let mut late = success(20, 65);
    if let RecordPayload::SessionReviewed {
        judgment,
        reflection,
        ..
    } = &mut late[1].payload
    {
        *judgment = pomotui_sync::SessionReviewJudgment::Failed;
        *reflection = Some("late break".into());
    }
    let path = root.join("activity.sync");
    let mut devices = Vec::new();
    for n in 0..2 {
        let mut service = Service::open(&root.join(format!("{n}.db"))).unwrap();
        service.enable_background_sync();
        service.handle(request(Command::SyncEnable { path: path.clone() }));
        service.apply_sync_records(&path, &records).unwrap();
        if n == 0 {
            let unlock_id = snapshot(&mut service).current_chain_rewards[0].id;
            assert!(matches!(
                service.handle(request(Command::RewardClaim { unlock_id })),
                Response::Snapshot { .. }
            ));
            records = service.sync_records_for(&path).unwrap();
        }
        assert!(
            snapshot(&mut service)
                .reward_debt
                .iter()
                .all(|d| d.outstanding == 0)
        );
        service.apply_sync_records(&path, &late).unwrap();
        assert_eq!(snapshot(&mut service).reward_debt[0].outstanding, 6);
        devices.push(service);
    }
    let repayments = (8..14)
        .flat_map(|n| success(n, n as i64 * 10))
        .collect::<Vec<_>>();
    let mut reversed = repayments.clone();
    reversed.reverse();
    devices[0].apply_sync_records(&path, &repayments).unwrap();
    devices[1].apply_sync_records(&path, &reversed).unwrap();
    assert_eq!(
        snapshot(&mut devices[0]).reward_debt,
        snapshot(&mut devices[1]).reward_debt
    );
    assert_eq!(snapshot(&mut devices[0]).reward_debt[0].repaid, 6);
    assert_eq!(snapshot(&mut devices[0]).action_chain.length, 7);
    assert!(
        snapshot(&mut devices[0])
            .current_chain_rewards
            .iter()
            .all(|u| u.state != "unlocked")
    );
    drop(devices);
    let mut restarted = Service::open(&root.join("0.db")).unwrap();
    assert_eq!(snapshot(&mut restarted).reward_debt[0].outstanding, 0);
    drop(restarted);
    std::fs::remove_dir_all(root).unwrap();
}
