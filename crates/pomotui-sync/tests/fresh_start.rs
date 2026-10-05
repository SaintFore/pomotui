use pomotui_sync::{Beginning, Document};

#[test]
fn empty_fresh_start_is_durable_and_causally_supersedes_an_observed_reset() {
    let first = Beginning::parse(1, "ffffffff-ffff-4fff-8fff-ffffffffffff").unwrap();
    let successor = Beginning::parse(2, "00000000-0000-4000-8000-000000000001").unwrap();
    let document = Document::with_beginning(successor.clone(), &[]).unwrap();
    let restored = Document::from_json(&document.to_json().unwrap()).unwrap();
    assert_eq!(restored.beginning(), &successor);
    assert!(successor > first);
    assert_eq!(Beginning::default(), Beginning::default());
}

#[test]
fn beginning_tampering_and_invalid_tokens_are_rejected_before_adoption() {
    let token = Beginning::parse(1, "00000000-0000-4000-8000-000000000001").unwrap();
    let source = Document::with_beginning(token, &[])
        .unwrap()
        .to_json()
        .unwrap();
    let tampered = source.replace("\"generation\": 1", "\"generation\": 2");
    assert!(Document::from_json(&tampered).is_err());
    let invalid = source.replace("\"generation\": 1", "\"generation\": 0");
    assert!(Document::from_json(&invalid).is_err());
    let legacy = source.replace("\"version\": 7", "\"version\": 6");
    assert!(Document::from_json(&legacy).is_err());
    let unsupported = source.replace("\"version\": 7", "\"version\": 8");
    assert!(Document::from_json(&unsupported).is_err());
}

#[test]
fn concurrent_empty_resets_converge_and_changed_record_membership_is_contradictory() {
    use pomotui_sync::{
        EntityId, MutationInstant, Record, RecordId, RecordPayload, TaskStatus, union_documents,
    };
    let low = Beginning::parse(1, "00000000-0000-4000-8000-000000000001").unwrap();
    let high = Beginning::parse(1, "ffffffff-ffff-4fff-8fff-ffffffffffff").unwrap();
    let a = Document::with_beginning(low.clone(), &[]).unwrap();
    let b = Document::with_beginning(high.clone(), &[]).unwrap();
    assert_eq!(
        union_documents(&a, &b).unwrap(),
        union_documents(&b, &a).unwrap()
    );
    assert_eq!(union_documents(&b, &a).unwrap().beginning(), &high);
    let record = Record::in_beginning(
        low,
        RecordId::random(),
        EntityId::random(),
        MutationInstant::from_millis(0).unwrap(),
        RecordPayload::TaskVersion {
            title: "work".into(),
            status: TaskStatus::Open,
        },
    );
    let mut changed = record.clone();
    changed.beginning = high;
    assert!(
        pomotui_sync::union(&[record], &[changed])
            .unwrap_err()
            .contains("beginning")
    );
}
