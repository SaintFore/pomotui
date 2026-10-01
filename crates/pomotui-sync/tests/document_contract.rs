use pomotui_sync::{
    ActivityProjection, Document, EntityId, MutationInstant, Record, RecordId, RecordPayload,
    SessionKind, SessionOutcome, SessionReviewJudgment, TaskProjection, TaskStatus, plan_sync,
};

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
fn task_deletion_cannot_contain_version_state() {
    let source = r#"{
      "format":"pomotui.sync",
      "version":3,
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
