use pomotui_sync::{
    ActivityProjection, Document, EntityId, MutationInstant, Record, RecordId, RecordPayload,
    ReviewedTaskKind, SessionKind, SessionOutcome, SessionReviewJudgment, TaskProjection,
    TaskStatus, plan_sync,
};

fn record_id(value: u128) -> RecordId {
    RecordId::parse(&uuid::Uuid::from_u128(value).to_string()).expect("record identity")
}

fn entity_id(value: u128) -> EntityId {
    EntityId::parse(&uuid::Uuid::from_u128(value).to_string()).expect("entity identity")
}

fn task_version(record: u128, entity: u128, mutation: i64, title: &str) -> Record {
    Record::new(
        RecordId::parse(&uuid::Uuid::from_u128(record).to_string()).expect("record identity"),
        EntityId::parse(&uuid::Uuid::from_u128(entity).to_string()).expect("entity identity"),
        MutationInstant::from_millis(mutation).expect("mutation instant"),
        RecordPayload::TaskVersion {
            title: title.into(),
            status: TaskStatus::Open,
        },
    )
}

fn session_review(
    record: u128,
    entity: u128,
    session: u128,
    judgment: SessionReviewJudgment,
) -> Record {
    Record::new(
        RecordId::parse(&uuid::Uuid::from_u128(record).to_string()).expect("record identity"),
        EntityId::parse(&uuid::Uuid::from_u128(entity).to_string()).expect("review identity"),
        MutationInstant::from_millis(4_000).expect("mutation instant"),
        RecordPayload::SessionReviewed {
            session_entity_id: EntityId::parse(&uuid::Uuid::from_u128(session).to_string())
                .expect("session identity"),
            judgment,
            task_entity_id: EntityId::parse(&uuid::Uuid::from_u128(16).to_string())
                .expect("task identity"),
            task_kind: pomotui_sync::ReviewedTaskKind::Regular,
            task_title: "Snapshot".into(),
            actual_seconds: 731,
            reflection: (judgment == SessionReviewJudgment::Failed).then(|| "Learned".into()),
            chain_entry_title: None,
        },
    )
}

fn ended_session(record: u128, session: u128, task: Option<u128>) -> Record {
    Record::new(
        RecordId::parse(&uuid::Uuid::from_u128(record).to_string()).expect("record identity"),
        EntityId::parse(&uuid::Uuid::from_u128(session).to_string()).expect("session identity"),
        MutationInstant::from_millis(3_000).expect("mutation instant"),
        RecordPayload::SessionEnded {
            ended_at: 1_700_000_000,
            kind: SessionKind::Focus,
            outcome: SessionOutcome::Stopped,
            planned_seconds: 1_500,
            actual_seconds: 731,
            task_entity_id: task.map(|value| {
                EntityId::parse(&uuid::Uuid::from_u128(value).to_string()).expect("task identity")
            }),
            task_title: task.map(|_| "Snapshot".into()),
        },
    )
}

#[test]
fn valid_task_records_have_byte_stable_round_trips() {
    let document = Document::new(&[
        task_version(2, 10, 2_000, "Second"),
        task_version(1, 9, 1_000, "First"),
    ])
    .expect("valid document");

    let encoded = document.to_json().expect("serialize");
    let decoded = Document::from_json(&encoded).expect("parse serialized document");

    assert_eq!(decoded.to_json().expect("serialize again"), encoded);
    assert!(encoded.ends_with('\n'));
}

#[test]
fn format_four_is_validated_and_upgraded_to_current_format() {
    let current = Document::new(&[task_version(1, 9, 1_000, "First")])
        .and_then(|document| document.to_json())
        .expect("current document");
    let legacy = legacy_json(&current, 4);

    let upgraded = Document::from_json(&legacy)
        .and_then(|document| document.to_json())
        .expect("upgrade format four");

    assert!(upgraded.contains("\"version\": 7"));
    assert!(!upgraded.contains("\"version\": 4"));
}

#[test]
fn format_four_void_title_upgrades_to_system_void_attribution() {
    let legacy_records = vec![
        task_version(1, 16, 1_000, "Void"),
        ended_session(2, 20, Some(16)),
        session_review(3, 30, 20, SessionReviewJudgment::Successful),
    ];
    let current = Document::new(&legacy_records)
        .and_then(|document| document.to_json())
        .expect("legacy-shaped document");
    let legacy = legacy_json(&current, 4);

    let upgraded = Document::from_json(&legacy).expect("upgrade format four");
    assert!(upgraded.records().iter().any(|record| matches!(
        record.payload,
        RecordPayload::SessionReviewed {
            task_kind: ReviewedTaskKind::SystemVoid,
            ..
        }
    )));
    assert!(
        plan_sync(&[], upgraded.records())
            .expect("project upgraded document")
            .task_projections()
            .is_empty()
    );
}

#[test]
fn task_deletion_cannot_contain_version_state() {
    let source = r#"{
      "format":"pomotui.sync",
      "version":4,
      "integrity":{"record_count":1,"records_sha256":"ignored"},
      "records":[{
        "id":"00000000-0000-0000-0000-000000000001",
        "entity_id":"00000000-0000-0000-0000-000000000002",
        "mutation_time":1000,
        "payload":{"type":"task_deleted","data":{"title":"impossible"}}
      }]
    }"#;

    let error = Document::from_json(source).expect_err("impossible payload must fail");
    assert!(
        error.contains("expected unit variant"),
        "unexpected error: {error}"
    );
}

#[test]
fn union_is_commutative_associative_and_idempotent() {
    let first = task_version(1, 10, 1_000, "First");
    let second = task_version(2, 20, 2_000, "Second");
    let third = task_version(3, 30, 3_000, "Third");

    let left = pomotui_sync::union(std::slice::from_ref(&first), std::slice::from_ref(&second))
        .expect("left union");
    let right = pomotui_sync::union(std::slice::from_ref(&second), std::slice::from_ref(&first))
        .expect("right union");
    assert_eq!(left, right);
    assert_eq!(
        pomotui_sync::union(&left, std::slice::from_ref(&third)).expect("(a union b) union c"),
        pomotui_sync::union(
            std::slice::from_ref(&first),
            &pomotui_sync::union(&[second], &[third]).expect("b union c"),
        )
        .expect("a union (b union c)"),
    );
    assert_eq!(
        pomotui_sync::union(std::slice::from_ref(&first), std::slice::from_ref(&first))
            .expect("self union"),
        vec![first],
    );
}

#[test]
fn task_projection_is_deterministic_and_deletion_is_permanent() {
    let entity = EntityId::parse("00000000-0000-0000-0000-000000000010").expect("entity");
    let older = task_version(1, 16, 1_000, "Older");
    let newer = task_version(2, 16, 2_000, "Newer");
    let deletion = Record::new(
        RecordId::parse("00000000-0000-0000-0000-000000000003").expect("record"),
        entity.clone(),
        MutationInstant::from_millis(500).expect("mutation instant"),
        RecordPayload::TaskDeleted,
    );

    assert_eq!(
        pomotui_sync::project_tasks(&[older, newer, deletion]),
        vec![pomotui_sync::TaskProjection::Deleted {
            entity_id: entity,
            last_title: Some("Newer".into()),
        }],
    );
}

#[test]
fn sync_engine_plans_the_retained_union_and_task_projection_together() {
    let local = task_version(1, 16, 1_000, "Local");
    let remote = task_version(2, 16, 2_000, "Remote");

    let plan =
        plan_sync(&[local], std::slice::from_ref(&remote)).expect("valid synchronization plan");

    assert_eq!(
        plan.retained_records(),
        &[task_version(1, 16, 1_000, "Local"), remote]
    );
    assert_eq!(
        plan.task_projections(),
        &[TaskProjection::Version {
            entity_id: EntityId::parse("00000000-0000-0000-0000-000000000010").expect("entity"),
            record_id: RecordId::parse("00000000-0000-0000-0000-000000000002").expect("record"),
            title: "Remote".into(),
            status: TaskStatus::Open,
        }]
    );
}

#[test]
fn ended_sessions_project_once_and_a_tombstone_permanently_hides_them() {
    let session = ended_session(20, 30, Some(16));
    let task = task_version(19, 16, 2_000, "Snapshot");
    let plan = plan_sync(&[], &[task.clone(), session.clone()]).expect("valid Session record");
    assert_eq!(
        plan.activity_projections(),
        &[ActivityProjection::Session {
            entity_id: EntityId::parse("00000000-0000-0000-0000-00000000001e").expect("entity"),
            ended_at: 1_700_000_000,
            kind: SessionKind::Focus,
            outcome: SessionOutcome::Stopped,
            planned_seconds: 1_500,
            actual_seconds: 731,
            task_entity_id: Some(
                EntityId::parse("00000000-0000-0000-0000-000000000010").expect("task entity")
            ),
            task_title: Some("Snapshot".into()),
        }]
    );

    let deletion = Record::new(
        RecordId::parse("00000000-0000-0000-0000-000000000021").expect("record"),
        EntityId::parse("00000000-0000-0000-0000-00000000001e").expect("entity"),
        MutationInstant::from_millis(4_000).expect("mutation instant"),
        RecordPayload::SessionDeleted,
    );
    let plan = plan_sync(&[task, session], &[deletion]).expect("valid tombstone");
    assert_eq!(
        plan.activity_projections(),
        &[ActivityProjection::Deleted {
            entity_id: EntityId::parse("00000000-0000-0000-0000-00000000001e").expect("entity")
        }]
    );
}

#[test]
fn sync_plan_rejects_a_session_referencing_an_unknown_task_identity() {
    let error = plan_sync(&[], &[ended_session(20, 30, Some(16))])
        .expect_err("unknown Task reference must fail before orchestration");

    assert!(
        error.contains("unknown Task identity"),
        "unexpected error: {error}"
    );
}

#[test]
fn reviews_project_by_session_end_then_identity_independent_of_arrival_order() {
    let task = task_version(1, 16, 1_000, "Snapshot");
    let mut first_session = ended_session(2, 20, Some(16));
    let mut second_session = ended_session(3, 30, Some(16));
    if let RecordPayload::SessionEnded { ended_at, .. } = &mut first_session.payload {
        *ended_at = 100;
    }
    if let RecordPayload::SessionEnded { ended_at, .. } = &mut second_session.payload {
        *ended_at = 200;
    }
    let success = session_review(5, 50, 30, SessionReviewJudgment::Successful);
    let late_failure = session_review(4, 40, 20, SessionReviewJudgment::Failed);
    let forward = plan_sync(
        &[],
        &[
            task.clone(),
            second_session.clone(),
            success.clone(),
            first_session.clone(),
            late_failure.clone(),
        ],
    )
    .expect("valid Reviews");
    let reversed = plan_sync(
        &[],
        &[task, late_failure, first_session, success, second_session],
    )
    .expect("same records in reverse order");

    assert_eq!(
        forward.session_review_projection(),
        reversed.session_review_projection()
    );
    assert_eq!(forward.session_review_projection().ended_chains.len(), 1);
    assert!(
        forward.session_review_projection().ended_chains[0]
            .links
            .is_empty()
    );
    assert_eq!(
        forward
            .session_review_projection()
            .current_chain
            .links
            .len(),
        1
    );
}

#[test]
fn equal_session_end_times_use_review_identity_as_the_tie_breaker() {
    let task = task_version(1, 16, 1_000, "Snapshot");
    let first_session = ended_session(2, 20, Some(16));
    let second_session = ended_session(3, 30, Some(16));
    // Record identity deliberately orders opposite to Session Review identity.
    let earlier_review_identity = session_review(5, 40, 30, SessionReviewJudgment::Successful);
    let later_review_identity = session_review(4, 50, 20, SessionReviewJudgment::Failed);
    let plan = plan_sync(
        &[],
        &[
            task,
            first_session,
            second_session,
            later_review_identity,
            earlier_review_identity,
        ],
    )
    .expect("equal timestamps remain orderable");

    assert_eq!(
        plan.session_review_projection().ended_chains[0].links.len(),
        1
    );
    assert!(
        plan.session_review_projection()
            .current_chain
            .links
            .is_empty()
    );
}

#[test]
fn retry_is_idempotent_and_a_second_review_for_one_session_is_rejected() {
    let records = vec![
        task_version(1, 16, 1_000, "Snapshot"),
        ended_session(2, 20, Some(16)),
        session_review(3, 30, 20, SessionReviewJudgment::Successful),
    ];
    let first = plan_sync(&[], &records).expect("first import");
    let retry = plan_sync(first.retained_records(), &records).expect("retry");
    assert_eq!(first.retained_records(), retry.retained_records());
    assert_eq!(
        first.session_review_projection(),
        retry.session_review_projection()
    );

    let duplicate = session_review(4, 40, 20, SessionReviewJudgment::Failed);
    let error = plan_sync(&records, &[duplicate]).expect_err("one immutable Review per Session");
    assert!(
        error.contains("more than one Review"),
        "unexpected error: {error}"
    );
}

#[test]
fn chain_entry_edits_converge_by_mutation_time_then_record_identity() {
    let records = vec![
        task_version(1, 16, 1_000, "Snapshot"),
        ended_session(2, 20, Some(16)),
        session_review(3, 30, 20, SessionReviewJudgment::Successful),
        Record::new(
            record_id(4),
            entity_id(30),
            MutationInstant::from_millis(4_000).expect("instant"),
            RecordPayload::ChainEntryVersion {
                reflection: Some("older wording".into()),
                chain_entry_title: Some("older title".into()),
            },
        ),
        Record::new(
            record_id(5),
            entity_id(30),
            MutationInstant::from_millis(5_000).expect("instant"),
            RecordPayload::ChainEntryVersion {
                reflection: Some("corrected wording".into()),
                chain_entry_title: Some("corrected title".into()),
            },
        ),
    ];

    let forward = plan_sync(&[], &records).expect("valid edit versions");
    let mut reversed = records.clone();
    reversed.reverse();
    let backward = plan_sync(&[], &reversed).expect("same versions in reverse order");

    assert_eq!(
        forward.session_review_projection(),
        backward.session_review_projection()
    );
    let link = &forward.session_review_projection().current_chain.links[0];
    assert_eq!(link.reflection.as_deref(), Some("corrected wording"));
    assert_eq!(link.chain_entry_title.as_deref(), Some("corrected title"));
    assert_eq!(link.actual_seconds, 731);
    assert_eq!(link.judgment, SessionReviewJudgment::Successful);
}

#[test]
fn ended_chain_tombstone_dominates_stale_replay_and_late_reviews() {
    let task = task_version(1, 16, 1_000, "Snapshot");
    let mut first_session = ended_session(2, 20, Some(16));
    let mut break_session = ended_session(3, 21, Some(16));
    let mut late_success_session = ended_session(4, 22, Some(16));
    let mut late_failure_session = ended_session(9, 23, Some(16));
    for (session, ended_at) in [
        (&mut first_session, 100),
        (&mut late_success_session, 200),
        (&mut late_failure_session, 250),
        (&mut break_session, 300),
    ] {
        if let RecordPayload::SessionEnded {
            ended_at: value, ..
        } = &mut session.payload
        {
            *value = ended_at;
        }
    }
    let success = session_review(5, 30, 20, SessionReviewJudgment::Successful);
    let failed = session_review(6, 31, 21, SessionReviewJudgment::Failed);
    let deletion = Record::new(
        record_id(7),
        entity_id(31),
        MutationInstant::from_millis(7_000).expect("instant"),
        RecordPayload::EndedChainDeleted {
            previous_chain_break_review_entity_id: None,
            deleted_review_entity_ids: vec![entity_id(30), entity_id(31)],
            observed_chain_break_review_entity_ids: vec![entity_id(31)],
        },
    );
    let mut late_success = session_review(8, 32, 22, SessionReviewJudgment::Successful);
    late_success.mutation_time = MutationInstant::from_millis(5_000).expect("offline instant");
    let mut late_failure = session_review(10, 33, 23, SessionReviewJudgment::Failed);
    late_failure.mutation_time = MutationInstant::from_millis(6_000).expect("offline instant");

    let deleted = plan_sync(
        &[],
        &[
            task.clone(),
            first_session.clone(),
            break_session.clone(),
            success.clone(),
            failed.clone(),
            deletion.clone(),
        ],
    )
    .expect("valid deletion");
    assert!(deleted.session_review_projection().ended_chains.is_empty());

    let replayed = plan_sync(
        deleted.retained_records(),
        &[
            task,
            first_session,
            break_session,
            success,
            failed,
            late_success_session,
            late_failure_session,
            late_success,
            late_failure,
            deletion,
        ],
    )
    .expect("stale and late records remain projectable");
    assert!(replayed.session_review_projection().ended_chains.is_empty());
    assert!(
        replayed
            .session_review_projection()
            .current_chain
            .links
            .is_empty()
    );
}

#[test]
fn ended_chain_tombstone_cannot_span_an_already_known_intervening_chain() {
    let task = task_version(1, 16, 1_000, "Snapshot");
    let mut sessions = [
        ended_session(2, 20, Some(16)),
        ended_session(3, 21, Some(16)),
        ended_session(4, 22, Some(16)),
    ];
    for (index, session) in sessions.iter_mut().enumerate() {
        if let RecordPayload::SessionEnded { ended_at, .. } = &mut session.payload {
            *ended_at = 100 + i64::try_from(index).expect("small index") * 100;
        }
    }
    let first = session_review(5, 30, 20, SessionReviewJudgment::Failed);
    let intervening = session_review(6, 31, 21, SessionReviewJudgment::Failed);
    let third = session_review(7, 32, 22, SessionReviewJudgment::Failed);
    let invalid = Record::new(
        record_id(8),
        entity_id(32),
        MutationInstant::from_millis(8_000).expect("instant"),
        RecordPayload::EndedChainDeleted {
            previous_chain_break_review_entity_id: Some(entity_id(30)),
            deleted_review_entity_ids: vec![entity_id(32)],
            observed_chain_break_review_entity_ids: vec![
                entity_id(30),
                entity_id(31),
                entity_id(32),
            ],
        },
    );
    let records = vec![
        task,
        sessions[0].clone(),
        sessions[1].clone(),
        sessions[2].clone(),
        first,
        intervening,
        third,
        invalid,
    ];

    let error = plan_sync(&[], &records).expect_err("cannot delete across another known chain");
    assert!(error.contains("not adjacent"), "{error}");
}

#[test]
fn conflicting_record_identity_reports_each_changed_field() {
    let original = task_version(1, 2, 1000, "Original");
    let contradictory = task_version(1, 3, 2000, "Different");
    let error = pomotui_sync::union(&[original], &[contradictory]).unwrap_err();
    assert!(error.contains("entity_id"), "{error}");
    assert!(error.contains("mutation"), "{error}");
    assert!(error.contains("payload"), "{error}");
}

fn legacy_json(current: &str, version: u16) -> String {
    use sha2::{Digest, Sha256};
    let mut value: serde_json::Value = serde_json::from_str(current).unwrap();
    value["version"] = version.into();
    value.as_object_mut().unwrap().remove("beginning");
    let records: Vec<pomotui_sync::Record> =
        serde_json::from_value(value["records"].clone()).unwrap();
    value["integrity"]["records_sha256"] = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&records).unwrap())
    )
    .into();
    serde_json::to_string(&value).unwrap()
}
