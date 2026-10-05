use pomotui_sync::{
    ClaimEvidence, EntityId, MutationInstant, Record, RecordId, RecordPayload, project_reward_debt,
};
fn id(n: u128) -> EntityId {
    EntityId::parse(&uuid::Uuid::from_u128(n).to_string()).unwrap()
}
#[test]
fn unsupported_claim_keeps_snapshot_obligation_and_duplicate_claims_charge_once() {
    let evidence = ClaimEvidence {
        threshold: 7,
        supporting_review_entity_ids: vec![id(1)],
        observed_review_entity_ids: vec![id(1)],
        frontier_review_entity_id: id(1),
    };
    let claim = Record::new(
        RecordId::random(),
        id(20),
        MutationInstant::from_millis(1).unwrap(),
        RecordPayload::RewardClaimed {
            milestone_entity_id: id(30),
            previous_chain_break_review_entity_id: None,
            claimed_at: 1,
            evidence: Some(evidence),
        },
    );
    let debt = project_reward_debt(&[claim.clone(), claim]);
    assert_eq!(debt[0].outstanding, 7);
    assert_eq!(debt[0].repaid, 0);
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
fn one_supporting_success_owes_six_and_six_future_successes_repay_each_milestone() {
    let mut records = success(1, 1);
    for milestone in [30, 31] {
        records.push(Record::new(
            RecordId::random(),
            id(milestone + 100),
            MutationInstant::from_millis(1).unwrap(),
            RecordPayload::RewardClaimed {
                milestone_entity_id: id(milestone),
                previous_chain_break_review_entity_id: None,
                claimed_at: 1,
                evidence: Some(ClaimEvidence {
                    threshold: 7,
                    supporting_review_entity_ids: vec![id(1)],
                    observed_review_entity_ids: vec![id(1)],
                    frontier_review_entity_id: id(1),
                }),
            },
        ));
    }
    assert!(
        project_reward_debt(&records)
            .iter()
            .all(|d| d.outstanding == 6)
    );
    for n in 2..8 {
        records.extend(success(n, n as i64));
    }
    let expected = project_reward_debt(&records);
    assert!(expected.iter().all(|d| d.outstanding == 0 && d.repaid == 6));
    records.reverse();
    records.extend(records.clone());
    assert_eq!(expected, project_reward_debt(&records));
}

#[test]
fn legacy_claims_without_observed_support_do_not_invent_debt() {
    let claim = Record::new(
        RecordId::random(),
        id(20),
        MutationInstant::from_millis(1).unwrap(),
        RecordPayload::RewardClaimed {
            milestone_entity_id: id(30),
            previous_chain_break_review_entity_id: None,
            claimed_at: 1,
            evidence: None,
        },
    );
    assert!(project_reward_debt(&[claim.clone()]).is_empty());
    assert!(!serde_json::to_string(&claim).unwrap().contains("evidence"));
}
