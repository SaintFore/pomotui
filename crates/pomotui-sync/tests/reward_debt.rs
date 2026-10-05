use pomotui_sync::{
    ClaimEvidence, EntityId, MutationInstant, Record, RecordId, RecordPayload, project_reward_debt,
};
fn id(n: u128) -> EntityId {
    EntityId::parse(&uuid::Uuid::from_u128(n).to_string()).unwrap()
}
#[test]
fn unsupported_claim_keeps_snapshot_obligation_and_duplicate_claims_charge_once() {
    let evidence = ClaimEvidence {
        carried_review_entity_ids: Vec::new(),
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
                    carried_review_entity_ids: Vec::new(),
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
        records.extend(success(n, i64::try_from(n).unwrap()));
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
    assert!(project_reward_debt(std::slice::from_ref(&claim)).is_empty());
    assert!(!serde_json::to_string(&claim).unwrap().contains("evidence"));
}

#[test]
fn late_support_correction_preserves_four_credits_and_carries_one_forward() {
    let mut records = success(1, 10);
    records.push(Record::new(
        RecordId::random(),
        id(20),
        MutationInstant::from_millis(10).unwrap(),
        RecordPayload::RewardClaimed {
            milestone_entity_id: id(30),
            previous_chain_break_review_entity_id: None,
            claimed_at: 10,
            evidence: Some(ClaimEvidence {
                carried_review_entity_ids: Vec::new(),
                threshold: 7,
                supporting_review_entity_ids: vec![id(1)],
                observed_review_entity_ids: vec![id(1)],
                frontier_review_entity_id: id(1),
            }),
        },
    ));
    for n in 2..6 {
        records.extend(success(n, 10 + i64::try_from(n).unwrap()));
    }
    assert_eq!(
        (
            project_reward_debt(&records)[0].outstanding,
            project_reward_debt(&records)[0].repaid
        ),
        (2, 4)
    );
    for n in 6..9 {
        records.extend(success(n, i64::try_from(n).unwrap()));
    }
    let debt = project_reward_debt(&records);
    assert_eq!(
        (debt[0].outstanding, debt[0].repaid, debt[0].excess_credit),
        (0, 4, 1)
    );
    records.reverse();
    records.extend(records.clone());
    assert_eq!(project_reward_debt(&records), debt);
}

#[test]
fn distinct_claims_allocate_each_success_once_per_milestone_even_after_breaks() {
    let mut records = (1..7)
        .flat_map(|n| success(n, i64::try_from(n).unwrap() * 10))
        .collect::<Vec<_>>();
    let mut failed = success(9, 35);
    if let RecordPayload::SessionReviewed { judgment, .. } = &mut failed[1].payload {
        *judgment = pomotui_sync::SessionReviewJudgment::Failed;
    }
    records.extend(failed);
    for (frontier, anchor) in [(1, None), (4, Some(id(9)))] {
        let claim = Record::new(
            RecordId::random(),
            id(40 + frontier),
            MutationInstant::from_millis(i64::try_from(frontier).unwrap() * 10).unwrap(),
            RecordPayload::RewardClaimed {
                milestone_entity_id: id(30),
                previous_chain_break_review_entity_id: anchor,
                claimed_at: i64::try_from(frontier).unwrap() * 10,
                evidence: Some(ClaimEvidence {
                    threshold: 3,
                    carried_review_entity_ids: vec![],
                    supporting_review_entity_ids: vec![id(frontier)],
                    observed_review_entity_ids: (1..=frontier).map(id).collect(),
                    frontier_review_entity_id: id(frontier),
                }),
            },
        );
        records.push(claim.clone());
        records.push(Record {
            id: RecordId::random(),
            ..claim
        });
    }
    let debt = project_reward_debt(&records);
    assert_eq!((debt[0].outstanding, debt[0].repaid), (0, 4));
    assert_eq!(
        debt[0].repayment_review_entity_ids,
        vec![id(2), id(3), id(5), id(6)]
    );
    records.reverse();
    assert_eq!(project_reward_debt(&records), debt);
}
