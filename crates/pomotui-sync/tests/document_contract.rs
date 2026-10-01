use pomotui_sync::{
    Document, EntityId, MutationInstant, Record, RecordId, RecordPayload, TaskStatus,
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
      "version":2,
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
        vec![pomotui_sync::TaskProjection::Deleted { entity_id: entity }],
    );
}
