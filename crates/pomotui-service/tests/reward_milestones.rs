mod support;

use pomotui_protocol::{
    Command, Handler, PROTOCOL_VERSION, Request, Response, SessionKind, Snapshot,
};
use pomotui_service::Service;
use pomotui_sync::RecordId;

fn command(service: &mut Service, command: Command) -> Response {
    service.handle(Request {
        version: PROTOCOL_VERSION,
        idempotency_key: Some(RecordId::random().as_str().into()),
        command,
    })
}

fn snapshot(service: &mut Service) -> Snapshot {
    match command(service, Command::Status) {
        Response::Snapshot { snapshot } => snapshot,
        other => panic!("{other:?}"),
    }
}

fn create(service: &mut Service, threshold: u64) -> Response {
    command(
        service,
        Command::RewardCreate {
            name: "Coffee".into(),
            threshold,
            budget: Some(35),
        },
    )
}

fn success(service: &mut Service, clock: &support::ManualClock) {
    if snapshot(service).kind != SessionKind::Focus {
        assert!(matches!(
            command(service, Command::Skip),
            Response::Snapshot { .. }
        ));
    }
    command(
        service,
        Command::TaskCreate {
            title: "Work".into(),
        },
    );
    command(
        service,
        Command::Start {
            kind: SessionKind::Focus,
            task_id: Some(1),
        },
    );
    clock.advance(1);
    assert!(matches!(
        command(service, Command::ReviewSuccess { reflection: None }),
        Response::Snapshot { .. }
    ));
}

#[test]
fn creating_rewards_requires_a_threshold_above_current_chain_length() {
    let clock = support::ManualClock::new(1_700_000_000);
    let mut service = Service::new_with_clock(clock.clone());
    service.configure_reminder(false, None, 100);
    service
        .configure_durations(pomotui_domain::SessionDurations::new(1, 1, 1).unwrap())
        .unwrap();
    success(&mut service, &clock);
    success(&mut service, &clock);
    for threshold in [0, 1, 2] {
        assert!(
            matches!(create(&mut service, threshold), Response::Error { .. }),
            "accepted threshold {threshold}"
        );
    }
    assert!(snapshot(&mut service).reward_milestones.is_empty());
    assert!(snapshot(&mut service).current_chain_rewards.is_empty());
    assert!(matches!(create(&mut service, 3), Response::Snapshot { .. }));
    success(&mut service, &clock);
    assert_eq!(
        snapshot(&mut service).current_chain_rewards[0].state,
        "unlocked"
    );
}

#[test]
fn changing_threshold_cannot_unlock_a_reward_retroactively() {
    let clock = support::ManualClock::new(1_700_000_000);
    let mut service = Service::new_with_clock(clock.clone());
    service.configure_reminder(false, None, 100);
    service
        .configure_durations(pomotui_domain::SessionDurations::new(1, 1, 1).unwrap())
        .unwrap();
    create(&mut service, 3);
    success(&mut service, &clock);
    success(&mut service, &clock);
    let id = snapshot(&mut service).reward_milestones[0].id;
    for threshold in [1, 2] {
        assert!(matches!(
            command(
                &mut service,
                Command::RewardUpdate {
                    id,
                    name: "Changed".into(),
                    threshold,
                    budget: None,
                }
            ),
            Response::Error { .. }
        ));
    }
    assert_eq!(snapshot(&mut service).reward_milestones[0].threshold, 3);
    assert!(snapshot(&mut service).current_chain_rewards.is_empty());
    for threshold in [4, 3] {
        assert!(matches!(
            command(
                &mut service,
                Command::RewardUpdate {
                    id,
                    name: "Coffee".into(),
                    threshold,
                    budget: Some(35),
                }
            ),
            Response::Snapshot { .. }
        ));
    }
    success(&mut service, &clock);
    // Metadata edits remain possible after a reward has been earned.
    assert!(matches!(
        command(
            &mut service,
            Command::RewardUpdate {
                id,
                name: "Better coffee".into(),
                threshold: 3,
                budget: Some(40),
            }
        ),
        Response::Snapshot { .. }
    ));
    let earned = snapshot(&mut service).current_chain_rewards.remove(0);
    assert_eq!(earned.name, "Coffee");
    assert_eq!(earned.budget, Some(35));
}

#[test]
fn deleting_a_milestone_cancels_unclaimed_rewards_across_sync_and_restart() {
    let root = std::env::temp_dir().join(format!(
        "pomotui-reward-delete-{}",
        RecordId::random().as_str()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let db = root.join("first.db");
    let path = root.join("activity.sync");
    let clock = support::ManualClock::new(1_700_000_000);
    let mut first = Service::open_with_clock(&db, clock.clone()).unwrap();
    first.configure_reminder(false, None, 100);
    first
        .configure_durations(pomotui_domain::SessionDurations::new(1, 1, 1).unwrap())
        .unwrap();
    first.enable_background_sync();
    command(&mut first, Command::SyncEnable { path: path.clone() });
    create(&mut first, 1);
    create(&mut first, 1);
    success(&mut first, &clock);
    let before = snapshot(&mut first);
    let claimed_id = before.current_chain_rewards[0].id;
    let cancelled_id = before.current_chain_rewards[1].id;
    command(
        &mut first,
        Command::RewardClaim {
            unlock_id: claimed_id,
        },
    );
    let mut second = Service::open(&root.join("second.db")).unwrap();
    second.enable_background_sync();
    command(&mut second, Command::SyncEnable { path: path.clone() });
    second
        .apply_sync_records(&path, &first.sync_records_for(&path).unwrap())
        .unwrap();
    for milestone in before.reward_milestones {
        command(&mut first, Command::RewardDelete { id: milestone.id });
    }
    assert!(matches!(
        command(
            &mut first,
            Command::RewardClaim {
                unlock_id: cancelled_id
            }
        ),
        Response::Error { .. }
    ));
    let records = first.sync_records_for(&path).unwrap();
    second.apply_sync_records(&path, &records).unwrap();
    drop(first);
    let mut restarted = Service::open_with_clock(&db, clock).unwrap();
    for service in [&mut restarted, &mut second] {
        let state = snapshot(service);
        assert!(state.reward_milestones.is_empty());
        assert_eq!(
            state
                .current_chain_rewards
                .iter()
                .filter(|reward| reward.state == "claimed")
                .count(),
            1
        );
        assert!(
            state
                .current_chain_rewards
                .iter()
                .all(|reward| reward.state != "unlocked")
        );
        let cancelled = state
            .current_chain_rewards
            .iter()
            .find(|reward| reward.state != "claimed")
            .unwrap();
        assert!(matches!(
            command(
                service,
                Command::RewardClaim {
                    unlock_id: cancelled.id
                }
            ),
            Response::Error { .. }
        ));
    }
    drop(restarted);
    drop(second);
    std::fs::remove_dir_all(root).unwrap();
}
