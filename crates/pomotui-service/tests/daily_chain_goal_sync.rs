mod support;

use std::fs;
use std::path::{Path, PathBuf};

use pomotui_domain::SessionDurations;
use pomotui_protocol::{
    ChainBreakSource, Command, DailyChainGoalSettings, Handler, PROTOCOL_VERSION, Request,
    Response, Snapshot,
};
use pomotui_service::Service;
use pomotui_sync::{Document, RecordId};
use support::ManualClock;

fn at(time: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(time)
        .unwrap()
        .timestamp()
}

struct Exchange {
    root: PathBuf,
}

impl Exchange {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("pomotui-goal-sync-{}", RecordId::random()));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn device(&self, name: &str, time: &str) -> Device {
        Device::new(&self.root, name, time)
    }
}

impl Drop for Exchange {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Device {
    database: PathBuf,
    exchange: PathBuf,
    clock: ManualClock,
    service: Service,
}

impl Device {
    fn new(root: &Path, name: &str, time: &str) -> Self {
        let database = root.join(format!("{name}.sqlite3"));
        let exchange = root.join(format!("{name}.sync"));
        fs::write(&exchange, Document::new(&[]).unwrap().to_json().unwrap()).unwrap();
        let clock = ManualClock::new(at(time));
        let mut service = Service::open_with_clock(&database, clock.clone()).unwrap();
        service.configure_reminder(false, None, 100);
        service
            .configure_durations(SessionDurations::new(10, 5, 15).unwrap())
            .unwrap();
        let mut device = Self {
            database,
            exchange,
            clock,
            service,
        };
        device.sync_command(Command::SyncEnable {
            path: device.exchange.clone(),
        });
        device
    }

    fn request(&mut self, command: Command) -> Response {
        self.service.handle(Request {
            version: PROTOCOL_VERSION,
            idempotency_key: command
                .mutates()
                .then(|| RecordId::random().as_str().into()),
            command,
        })
    }

    fn snapshot(&mut self, command: Command) -> Snapshot {
        let response = self.request(command);
        let Response::Snapshot { snapshot } = response else {
            panic!("expected Snapshot, got {response:?}");
        };
        snapshot
    }

    fn sync_command(&mut self, command: Command) {
        let response = self.request(command);
        let Response::Data { value } = response else {
            panic!("expected synchronization data, got {response:?}");
        };
        assert_eq!(value["enabled"], true);
        assert!(value["last_error"].is_null(), "{value:?}");
    }

    fn sync(&mut self) {
        self.sync_command(Command::SyncNow);
    }

    fn receive(&mut self, source: &[u8]) {
        fs::write(&self.exchange, source).unwrap();
        self.sync();
    }

    fn file(&self) -> Vec<u8> {
        fs::read(&self.exchange).unwrap()
    }

    fn restart(self) -> Self {
        let Self {
            database,
            exchange,
            clock,
            service,
        } = self;
        drop(service);
        let service = Service::open_with_clock(&database, clock.clone()).unwrap();
        Self {
            database,
            exchange,
            clock,
            service,
        }
    }

    fn goal(&mut self, quota: u64) -> Snapshot {
        self.snapshot(Command::ConfigureDailyChainGoal {
            settings: Some(DailyChainGoalSettings {
                quota,
                start_date: None,
                end_date: None,
                time_zone: "Asia/Shanghai".into(),
            }),
        })
    }

    fn round(&mut self) -> Snapshot {
        self.snapshot(Command::StartTitle {
            title: "Plan tomorrow".into(),
        });
        self.clock.advance(10);
        let reviewed = self.snapshot(Command::ReviewSuccess { reflection: None });
        self.snapshot(Command::Skip);
        self.clock.advance(1);
        reviewed
    }
}

#[test]
fn a_late_goal_creates_debt_and_merged_credit_repairs_the_cutoff() {
    let exchange = Exchange::new();
    let mut a = exchange.device("a", "2026-11-20T15:59:00Z");
    let mut b = exchange.device("b", "2026-11-20T15:59:00Z");
    for name in ["Claimed", "Unclaimed"] {
        a.snapshot(Command::RewardCreate {
            name: name.into(),
            threshold: 3,
            budget: None,
        });
    }
    a.sync();
    b.receive(&a.file());
    a.goal(4);
    a.sync();
    let schedule_only = a.file();

    a.clock.advance(60);
    b.clock.advance(100);
    a.round();
    assert_eq!(a.round().daily_chain_goal.achieved, 2);
    b.round();
    b.round();

    a.clock.advance(86_400 - 22);
    b.clock.advance(86_400 - 62);
    let provisional = a.snapshot(Command::Status);
    assert_eq!(provisional.action_chain.length, 0);
    let [archive] = provisional.recent_ended_chains.as_slice() else {
        panic!("expected one provisional archive");
    };
    assert_eq!(archive.break_source, ChainBreakSource::DailyChainGoal);
    let cutoff = archive.break_daily_chain_goal.as_ref().unwrap();
    assert_eq!(
        (cutoff.date.as_str(), cutoff.achieved, cutoff.quota),
        ("2026-11-21", 2, 4)
    );

    let offline = b.round();
    assert_eq!(offline.action_chain.length, 3);
    let claim_id = offline
        .current_chain_rewards
        .iter()
        .find(|reward| reward.name == "Claimed")
        .unwrap()
        .id;
    b.snapshot(Command::RewardClaim {
        unlock_id: claim_id,
    });
    b.receive(&schedule_only);
    let reduced = b.snapshot(Command::Status);
    assert_eq!(reduced.action_chain.length, 1);
    assert!(reduced.current_chain_rewards.is_empty());
    assert_eq!(reduced.reward_debt[0].outstanding, 2);
    let partial = b.file();

    a.sync();
    b.receive(&a.file());
    a.receive(&b.file());
    for device in [a, b] {
        let mut device = device.restart();
        device.receive(&partial);
        device.sync();
        let repaired = device.snapshot(Command::Status);
        assert_eq!(repaired.daily_chain_goal.date, "2026-11-22");
        assert_eq!(repaired.daily_chain_goal.achieved, 1);
        assert_eq!(repaired.action_chain.length, 5);
        assert!(repaired.recent_ended_chains.is_empty());
        assert_eq!(repaired.reward_debt[0].outstanding, 0);
        assert_eq!(repaired.current_chain_rewards.len(), 2);
        for (name, state) in [("Claimed", "claimed"), ("Unclaimed", "unlocked")] {
            assert!(
                repaired
                    .current_chain_rewards
                    .iter()
                    .any(|reward| { reward.name == name && reward.state == state })
            );
        }
    }
}

#[test]
fn claims_around_a_repaired_cutoff_follow_the_merged_archive() {
    let exchange = Exchange::new();
    let mut a = exchange.device("a", "2026-11-20T15:58:00Z");
    let mut b = exchange.device("b", "2026-11-20T15:58:00Z");
    a.round();
    a.snapshot(Command::StartTitle {
        title: "Before the goal".into(),
    });
    let stopped = a.snapshot(Command::Stop);
    let original_archive = stopped.recent_ended_chains[0].id;

    for name in ["Claimed twice", "Unclaimed"] {
        a.snapshot(Command::RewardCreate {
            name: name.into(),
            threshold: 1,
            budget: None,
        });
    }
    a.goal(2);
    a.sync();
    b.receive(&a.file());
    a.clock.advance(109);
    b.clock.advance(123);

    let first = a.round();
    let first_claim = first
        .current_chain_rewards
        .iter()
        .find(|reward| reward.name == "Claimed twice")
        .unwrap()
        .id;
    a.snapshot(Command::RewardClaim {
        unlock_id: first_claim,
    });
    b.round();

    a.clock.advance(86_400 - 11);
    let after_cutoff = a.round();
    assert_eq!(after_cutoff.action_chain.length, 1);
    assert_eq!(after_cutoff.recent_ended_chains.len(), 2);
    let second_claim = after_cutoff
        .current_chain_rewards
        .iter()
        .find(|reward| reward.name == "Claimed twice")
        .unwrap()
        .id;
    assert_ne!(first_claim, second_claim);
    a.snapshot(Command::RewardClaim {
        unlock_id: second_claim,
    });
    a.snapshot(Command::StartTitle {
        title: "After the goal cutoff".into(),
    });
    a.snapshot(Command::Stop);

    b.clock.advance(86_400);
    b.sync();
    a.receive(&b.file());
    b.receive(&a.file());

    for (mut device, local_claims) in [
        (a.restart(), Some([first_claim, second_claim])),
        (b.restart(), None),
    ] {
        let repaired = device.snapshot(Command::Status);
        assert_eq!(repaired.action_chain.length, 0);
        let [merged, original] = repaired.recent_ended_chains.as_slice() else {
            panic!("expected the two explicit stops");
        };
        assert_eq!(merged.length, 3);
        assert_eq!(merged.break_source, ChainBreakSource::FocusStopped);
        assert!(original.rewards.is_empty());
        let claims = merged
            .rewards
            .iter()
            .filter(|reward| reward.name == "Claimed twice" && reward.state == "claimed")
            .map(|reward| reward.id)
            .collect::<Vec<_>>();
        assert_eq!(claims.len(), 2);
        assert_ne!(claims[0], claims[1]);
        if let Some(local_claims) = local_claims {
            assert_eq!(original.id, original_archive);
            assert!(local_claims.iter().all(|id| claims.contains(id)));
        }
        assert_eq!(
            merged
                .rewards
                .iter()
                .filter(|reward| reward.name == "Unclaimed")
                .count(),
            1
        );
        assert!(
            repaired
                .reward_debt
                .iter()
                .all(|debt| debt.outstanding == 0)
        );
    }
}

// A released Device could accept a successful review of a Stopped Focus Session.
fn historical_stopped_review(ended_at: i64) -> Vec<u8> {
    use pomotui_sync::{
        EntityId, MutationInstant, Record, RecordPayload, ReviewedTaskKind, SessionKind,
        SessionOutcome, SessionReviewJudgment,
    };

    let session = EntityId::random();
    let mutation = MutationInstant::from_millis(ended_at * 1_000).unwrap();
    let records = [
        Record::new(
            RecordId::random(),
            session.clone(),
            mutation,
            RecordPayload::SessionEnded {
                ended_at,
                kind: SessionKind::Focus,
                outcome: SessionOutcome::Stopped,
                planned_seconds: 10,
                actual_seconds: 3,
                task_entity_id: None,
                task_title: None,
            },
        ),
        Record::new(
            RecordId::random(),
            EntityId::random(),
            mutation,
            RecordPayload::SessionReviewed {
                session_entity_id: session,
                judgment: SessionReviewJudgment::Successful,
                task_entity_id: EntityId::random(),
                task_kind: ReviewedTaskKind::SystemVoid,
                task_title: "Void".into(),
                actual_seconds: 3,
                reflection: None,
                chain_entry_title: Some("Historical stopped work".into()),
            },
        ),
    ];
    Document::new(&records)
        .unwrap()
        .to_json()
        .unwrap()
        .into_bytes()
}

#[test]
fn repairing_a_deleted_archives_previous_cutoff_keeps_the_deletion_and_legacy_link() {
    let exchange = Exchange::new();
    let mut a = exchange.device("a", "2026-11-20T15:59:00Z");
    let mut b = exchange.device("b", "2026-11-20T15:59:00Z");
    a.goal(2);
    a.sync();
    b.receive(&a.file());
    a.clock.advance(61);
    b.clock.advance(90);
    a.receive(&historical_stopped_review(at("2026-11-20T16:00:00Z")));
    let local = a.round();
    assert_eq!(local.action_chain.length, 2);
    assert_eq!(local.daily_chain_goal.achieved, 1);
    b.round();

    a.clock.advance(86_400 - 12);
    assert_eq!(a.snapshot(Command::Status).recent_ended_chains[0].length, 2);
    a.round();
    a.snapshot(Command::StartTitle {
        title: "End the next chain".into(),
    });
    let stopped = a.snapshot(Command::Stop);
    assert_eq!(stopped.recent_ended_chains.len(), 2);
    a.snapshot(Command::EndedChainDelete {
        id: stopped.recent_ended_chains[0].id,
    });
    a.sync();
    let before_repair = a.file();
    b.clock.advance(86_400);
    b.receive(&before_repair);
    a.receive(&b.file());
    for device in [a, b] {
        let mut device = device.restart();
        device.receive(&before_repair);
        let repaired = device.snapshot(Command::Status);
        assert!(repaired.recent_ended_chains.is_empty());
        assert_eq!(repaired.action_chain.length, 3);
        assert_eq!(repaired.daily_chain_goal.achieved, 1);
        assert!(
            repaired.recent_chain_links.iter().any(|link| {
                link.chain_entry_title.as_deref() == Some("Historical stopped work")
            })
        );
    }
}

#[test]
fn fresh_start_retires_the_goal_and_work_of_an_offline_device() {
    let exchange = Exchange::new();
    let mut a = exchange.device("a", "2026-11-20T15:59:00Z");
    let mut b = exchange.device("b", "2026-11-20T15:59:00Z");
    a.goal(2);
    a.sync();
    b.receive(&a.file());
    a.clock.advance(60);
    b.clock.advance(60);
    a.round();
    b.round();
    let revised = b.goal(3);
    assert_eq!(revised.daily_chain_goal.active.unwrap().quota, 2);
    assert_eq!(
        revised.daily_chain_goal.scheduled.unwrap().effective_date,
        "2026-11-22"
    );
    b.sync();
    let retired = b.file();

    a.snapshot(Command::FreshStart { confirmed: true });
    a.sync();
    b.receive(&a.file());
    a.receive(&retired);
    for device in [a, b] {
        let mut device = device.restart();
        device.receive(&retired);
        let cleared = device.snapshot(Command::Status);
        assert!(cleared.daily_chain_goal.active.is_none());
        assert!(cleared.daily_chain_goal.scheduled.is_none());
        assert!(cleared.recent_ended_chains.is_empty());
        assert!(cleared.recent_history.is_empty());
        assert_eq!(cleared.action_chain.length, 0);
    }
}
