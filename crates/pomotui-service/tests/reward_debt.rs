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
#[allow(clippy::too_many_lines)]
fn two_devices_replay_claim_debt_and_offline_repayments_after_restart() {
    let root = std::env::temp_dir().join(format!("pomotui-debt-{}", RecordId::random().as_str()));
    std::fs::create_dir_all(&root).unwrap();
    let mut records = (1..8)
        .flat_map(|n| success(n, i64::try_from(n).unwrap() * 10))
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
    let paths = [
        root.join("replica-a/activity.sync"),
        root.join("replica-b/activity.sync"),
    ];
    for path in &paths {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    }
    let mut devices = Vec::new();
    for (n, path) in paths.iter().enumerate() {
        let mut service = Service::open(&root.join(format!("{n}.db"))).unwrap();
        service.enable_background_sync();
        service.handle(request(Command::SyncEnable { path: path.clone() }));
        service.apply_sync_records(path, &records).unwrap();
        if n == 0 {
            let unlock_id = snapshot(&mut service).current_chain_rewards[0].id;
            assert!(matches!(
                service.handle(request(Command::RewardClaim { unlock_id })),
                Response::Snapshot { .. }
            ));
            records = service.sync_records_for(path).unwrap();
        }
        assert!(
            snapshot(&mut service)
                .reward_debt
                .iter()
                .all(|d| d.outstanding == 0)
        );
        service.apply_sync_records(path, &late).unwrap();
        assert_eq!(snapshot(&mut service).reward_debt[0].outstanding, 6);
        devices.push(service);
    }
    let first_repayments = (8..12)
        .flat_map(|n| success(n, i64::try_from(n).unwrap() * 10))
        .collect::<Vec<_>>();
    for (service, path) in devices.iter_mut().zip(&paths) {
        service.apply_sync_records(path, &first_repayments).unwrap();
    }
    assert_eq!(
        (
            snapshot(&mut devices[0]).reward_debt[0].outstanding,
            snapshot(&mut devices[0]).reward_debt[0].repaid
        ),
        (2, 4)
    );
    let correction = (21..24)
        .flat_map(|n| success(n, 45 + i64::try_from(n).unwrap()))
        .collect::<Vec<_>>();
    for (service, path) in devices.iter_mut().zip(&paths) {
        service.apply_sync_records(path, &correction).unwrap();
    }
    assert_eq!(
        (
            snapshot(&mut devices[0]).reward_debt[0].outstanding,
            snapshot(&mut devices[0]).reward_debt[0].excess_credit
        ),
        (0, 1)
    );
    let mut inverse = success(24, 69);
    if let RecordPayload::SessionReviewed {
        judgment,
        reflection,
        ..
    } = &mut inverse[1].payload
    {
        *judgment = pomotui_sync::SessionReviewJudgment::Failed;
        *reflection = Some("inverse correction".into());
    }
    for (service, path) in devices.iter_mut().zip(&paths) {
        service.apply_sync_records(path, &inverse).unwrap();
    }
    assert_eq!(
        (
            snapshot(&mut devices[0]).reward_debt[0].outstanding,
            snapshot(&mut devices[0]).reward_debt[0].repaid
        ),
        (2, 4)
    );
    let repayments = (12..14)
        .flat_map(|n| success(n, i64::try_from(n).unwrap() * 10))
        .collect::<Vec<_>>();
    let mut reversed = repayments.clone();
    reversed.reverse();
    devices[0]
        .apply_sync_records(&paths[0], &repayments)
        .unwrap();
    devices[1].apply_sync_records(&paths[1], &reversed).unwrap();
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
    let before_deletion = snapshot(&mut devices[0]).reward_debt;
    let ended = snapshot(&mut devices[0]).recent_ended_chains;
    for chain in ended {
        assert!(matches!(
            devices[0].handle(request(Command::EndedChainDelete { id: chain.id })),
            Response::Snapshot { .. }
        ));
    }
    assert!(snapshot(&mut devices[0]).recent_ended_chains.is_empty());
    assert_eq!(snapshot(&mut devices[0]).reward_debt, before_deletion);
    let milestone_id = snapshot(&mut devices[0]).reward_milestones[0].id;
    assert!(matches!(
        devices[0].handle(request(Command::RewardDelete { id: milestone_id })),
        Response::Snapshot { .. }
    ));
    assert_eq!(snapshot(&mut devices[0]).reward_debt, before_deletion);
    drop(devices);
    let mut restarted = Service::open(&root.join("0.db")).unwrap();
    assert_eq!(snapshot(&mut restarted).reward_debt[0].outstanding, 0);
    drop(restarted);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn corrected_surplus_crosses_break_and_is_consumed_by_one_later_claim() {
    let root = std::env::temp_dir().join(format!("pomotui-credit-{}", RecordId::random().as_str()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("activity.sync");
    let mut service = Service::open(&root.join("service.db")).unwrap();
    service.enable_background_sync();
    service.handle(request(Command::SyncEnable { path: path.clone() }));
    let mut records = success(1, 10);
    records.push(Record::new(
        RecordId::random(),
        id(30),
        MutationInstant::from_millis(10).unwrap(),
        RecordPayload::RewardMilestoneVersion {
            name: "Coffee".into(),
            threshold: 7,
            budget: None,
        },
    ));
    records.push(Record::new(
        RecordId::random(),
        id(40),
        MutationInstant::from_millis(10).unwrap(),
        RecordPayload::RewardClaimed {
            milestone_entity_id: id(30),
            previous_chain_break_review_entity_id: None,
            claimed_at: 10,
            evidence: Some(pomotui_sync::ClaimEvidence {
                threshold: 7,
                carried_review_entity_ids: vec![],
                supporting_review_entity_ids: vec![id(1)],
                observed_review_entity_ids: vec![id(1)],
                frontier_review_entity_id: id(1),
            }),
        },
    ));
    records.extend((2..6).flat_map(|n| success(n, 10 + i64::try_from(n).unwrap())));
    records.extend((6..9).flat_map(|n| success(n, i64::try_from(n).unwrap())));
    let mut failed = success(9, 20);
    if let RecordPayload::SessionReviewed {
        judgment,
        reflection,
        ..
    } = &mut failed[1].payload
    {
        *judgment = pomotui_sync::SessionReviewJudgment::Failed;
        *reflection = Some("break".into());
    }
    records.extend(failed);
    service.apply_sync_records(&path, &records).unwrap();
    assert_eq!(snapshot(&mut service).action_chain.length, 0);
    assert_eq!(snapshot(&mut service).reward_debt[0].excess_credit, 1);
    let future = (10..16)
        .flat_map(|n| success(n, 20 + i64::try_from(n).unwrap()))
        .collect::<Vec<_>>();
    service.apply_sync_records(&path, &future).unwrap();
    let state = snapshot(&mut service);
    assert_eq!(state.action_chain.length, 6);
    let unlock = state
        .current_chain_rewards
        .iter()
        .find(|r| r.state == "unlocked")
        .expect("carried credit completes the threshold")
        .id;
    assert!(matches!(
        service.handle(request(Command::RewardClaim { unlock_id: unlock })),
        Response::Snapshot { .. }
    ));
    assert_eq!(snapshot(&mut service).reward_debt[0].excess_credit, 0);
    drop(service);
    let mut restarted = Service::open(&root.join("service.db")).unwrap();
    assert_eq!(snapshot(&mut restarted).reward_debt[0].excess_credit, 0);
    drop(restarted);
    std::fs::remove_dir_all(root).unwrap();
}
