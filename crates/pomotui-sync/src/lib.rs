//! Provider-neutral synchronization document and convergence engine.

#![allow(clippy::missing_errors_doc)]

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

pub const FORMAT_VERSION: u16 = 7;
const FORMAT_NAME: &str = "pomotui.sync";

macro_rules! identity {
    ($name:ident, $label:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: &str) -> Result<Self, String> {
                let parsed = uuid::Uuid::parse_str(value)
                    .map_err(|_| format!("invalid {} {value}", $label))?;
                if parsed.is_nil() {
                    return Err(format!("invalid {} {value}", $label));
                }
                Ok(Self(parsed.to_string()))
            }

            #[must_use]
            pub fn random() -> Self {
                Self(uuid::Uuid::new_v4().to_string())
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::parse(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}

identity!(RecordId, "synchronization record identity");
identity!(EntityId, "synchronized entity identity");

/// Causal total-order beginning. All pre-reset documents share universal genesis.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Beginning {
    generation: u64,
    id: uuid::Uuid,
}
impl Default for Beginning {
    fn default() -> Self { Self { generation: 0, id: uuid::Uuid::nil() } }
}
impl Beginning {
    pub fn parse(generation: u64, id: &str) -> Result<Self, String> {
        let id = uuid::Uuid::parse_str(id).map_err(|e| e.to_string())?;
        if (generation == 0) != id.is_nil() { return Err("invalid Fresh Start beginning".into()); }
        Ok(Self { generation, id })
    }
    pub fn successor(&self) -> Result<Self, String> {
        Ok(Self { generation: self.generation.checked_add(1).ok_or("Fresh Start generation exhausted")?, id: uuid::Uuid::new_v4() })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct MutationInstant(i64);

impl MutationInstant {
    pub fn from_millis(value: i64) -> Result<Self, String> {
        chrono::DateTime::from_timestamp_millis(value)
            .ok_or_else(|| format!("invalid synchronization mutation instant {value}"))?;
        Ok(Self(value))
    }

    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }
}

impl<'de> Deserialize<'de> for MutationInstant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::from_millis(i64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    Completed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKind {
    Focus,
    ShortBreak,
    LongBreak,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    Completed,
    Stopped,
    Skipped,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionReviewJudgment {
    Successful,
    Failed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewedTaskKind {
    #[default]
    Regular,
    SystemVoid,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_regular_task(kind: &ReviewedTaskKind) -> bool {
    matches!(kind, ReviewedTaskKind::Regular)
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RecordPayload {
    TaskVersion {
        title: String,
        status: TaskStatus,
    },
    TaskDeleted,
    SessionEnded {
        ended_at: i64,
        kind: SessionKind,
        outcome: SessionOutcome,
        planned_seconds: u64,
        actual_seconds: u64,
        task_entity_id: Option<EntityId>,
        task_title: Option<String>,
    },
    SessionDeleted,
    SessionReviewed {
        session_entity_id: EntityId,
        judgment: SessionReviewJudgment,
        task_entity_id: EntityId,
        #[serde(default, skip_serializing_if = "is_regular_task")]
        task_kind: ReviewedTaskKind,
        task_title: String,
        actual_seconds: u64,
        reflection: Option<String>,
        chain_entry_title: Option<String>,
    },
    ChainEntryVersion {
        reflection: Option<String>,
        chain_entry_title: Option<String>,
    },
    EndedChainDeleted {
        previous_chain_break_review_entity_id: Option<EntityId>,
        deleted_review_entity_ids: Vec<EntityId>,
        observed_chain_break_review_entity_ids: Vec<EntityId>,
    },
    RewardMilestoneVersion {
        name: String,
        threshold: u64,
        budget: Option<u64>,
    },
    RewardMilestoneDeleted,
    RewardUnlocked {
        milestone_entity_id: EntityId,
        previous_chain_break_review_entity_id: Option<EntityId>,
        name: String,
        threshold: u64,
        budget: Option<u64>,
    },
    RewardClaimed {
        milestone_entity_id: EntityId,
        previous_chain_break_review_entity_id: Option<EntityId>,
        claimed_at: i64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<ClaimEvidence>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Record {
    #[serde(default)]
    pub beginning: Beginning,
    pub id: RecordId,
    pub entity_id: EntityId,
    pub mutation_time: MutationInstant,
    pub payload: RecordPayload,
}

impl Record {
    pub fn in_beginning(beginning: Beginning, id: RecordId, entity_id: EntityId, mutation_time: MutationInstant, payload: RecordPayload) -> Self {
        Self { beginning, id, entity_id, mutation_time, payload }
    }
    #[must_use]
    pub fn new(
        id: RecordId,
        entity_id: EntityId,
        mutation_time: MutationInstant,
        payload: RecordPayload,
    ) -> Self {
        Self {
            beginning: Beginning::default(),
            id,
            entity_id,
            mutation_time,
            payload,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Integrity {
    record_count: usize,
    records_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Document {
    #[serde(default)]
    beginning: Beginning,
    format: String,
    version: u16,
    integrity: Integrity,
    records: Vec<Record>,
}

impl Document {
    pub fn new(records: &[Record]) -> Result<Self, String> {
        let beginning = records.iter().map(|r| &r.beginning).max().cloned().unwrap_or_default();
        Self::with_beginning(beginning, records)
    }

    pub fn with_beginning(beginning: Beginning, records: &[Record]) -> Result<Self, String> {
        let records = union(&[], records)?;
        if records.iter().any(|r| r.beginning != beginning) { return Err("document record beginning differs from selected beginning".into()); }
        validate_records(&records)?;
        Ok(Self {
            beginning,
            format: FORMAT_NAME.into(),
            version: FORMAT_VERSION,
            integrity: Integrity {
                record_count: records.len(),
                records_sha256: checksum(&records)?,
            },
            records,
        })
    }

    pub fn from_json(source: &str) -> Result<Self, String> {
        let mut document: Self = serde_json::from_str(source)
            .map_err(|error| format!("invalid sync document: {error}"))?;
        if document.format != FORMAT_NAME {
            return Err("unsupported sync document format".into());
        }
        if !matches!(document.version, 4 | 5 | 6 | FORMAT_VERSION) {
            return Err(format!(
                "unsupported sync document version {}; reset local pre-release data with `pomotui reset --all-data --confirm`",
                document.version
            ));
        }
        validate_records(&document.records)?;
        if document.integrity.record_count != document.records.len() {
            return Err("sync document integrity check failed".into());
        }
        let expected_checksum = if document.version < FORMAT_VERSION {
            #[derive(Serialize)]
            struct LegacyRecord<'a> { id: &'a RecordId, entity_id: &'a EntityId, mutation_time: MutationInstant, payload: &'a RecordPayload }
            let legacy = document.records.iter().map(|r| LegacyRecord { id: &r.id, entity_id: &r.entity_id, mutation_time: r.mutation_time, payload: &r.payload }).collect::<Vec<_>>();
            format!("{:x}", Sha256::digest(serde_json::to_vec(&legacy).map_err(|e| e.to_string())?))
        } else { checksum(&document.records)? };
        if document.integrity.records_sha256 != expected_checksum {
            return Err("sync document checksum does not match its records".into());
        }
        if document.version == 4 {
            let void_entities = document
                .records
                .iter()
                .filter_map(|record| match &record.payload {
                    RecordPayload::TaskVersion { title, .. } if title == "Void" => {
                        Some(record.entity_id.clone())
                    }
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>();
            for record in &mut document.records {
                if let RecordPayload::SessionReviewed {
                    task_entity_id,
                    task_kind,
                    ..
                } = &mut record.payload
                    && void_entities.contains(task_entity_id)
                {
                    *task_kind = ReviewedTaskKind::SystemVoid;
                }
            }
        }
        Ok(Self {
            version: FORMAT_VERSION,
            ..document
        })
    }

    pub fn to_json(&self) -> Result<String, String> {
        let canonical = Self::with_beginning(self.beginning.clone(), &self.records)?;
        let mut source =
            serde_json::to_string_pretty(&canonical).map_err(|error| error.to_string())?;
        source.push('\n');
        Ok(source)
    }

    #[must_use]
    pub fn beginning(&self) -> &Beginning { &self.beginning }

    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    #[must_use]
    pub fn into_records(self) -> Vec<Record> {
        self.records
    }
}

pub fn union(left: &[Record], right: &[Record]) -> Result<Vec<Record>, String> {
    let mut records = BTreeMap::<RecordId, Record>::new();
    for record in left.iter().chain(right) {
        if let Some(existing) = records.get(&record.id)
            && existing != record
        {
            let mut differences = Vec::new();
            if existing.beginning != record.beginning { differences.push("beginning membership"); }
            if existing.entity_id != record.entity_id {
                differences.push("entity_id");
            }
            if existing.mutation_time != record.mutation_time {
                differences.push("mutation time");
            }
            if existing.payload != record.payload {
                differences.push("payload kind/content");
            }
            return Err(format!(
                "conflicting synchronization record {}: differing {}; retain both inputs and repair the contradictory record identity",
                record.id.as_str(),
                differences.join(", ")
            ));
        }
        records.insert(record.id.clone(), record.clone());
    }
    Ok(records.into_values().collect())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskProjection {
    Version {
        entity_id: EntityId,
        record_id: RecordId,
        title: String,
        status: TaskStatus,
    },
    Deleted {
        entity_id: EntityId,
        last_title: Option<String>,
    },
}

/// A validated synchronization result ready for the Timer Service to apply.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyncPlan {
    base_records: Vec<Record>,
    retained_records: Vec<Record>,
    task_projections: Vec<TaskProjection>,
    activity_projections: Vec<ActivityProjection>,
    session_review_projection: SessionReviewProjection,
    reward_projection: RewardProjection,
}

impl SyncPlan {
    #[must_use]
    pub fn base_records(&self) -> &[Record] {
        &self.base_records
    }

    #[must_use]
    pub fn retained_records(&self) -> &[Record] {
        &self.retained_records
    }

    #[must_use]
    pub fn task_projections(&self) -> &[TaskProjection] {
        &self.task_projections
    }

    #[must_use]
    pub fn activity_projections(&self) -> &[ActivityProjection] {
        &self.activity_projections
    }

    #[must_use]
    pub const fn session_review_projection(&self) -> &SessionReviewProjection {
        &self.session_review_projection
    }
    #[must_use]
    pub const fn reward_projection(&self) -> &RewardProjection {
        &self.reward_projection
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RewardMilestoneProjection {
    Version {
        entity_id: EntityId,
        name: String,
        threshold: u64,
        budget: Option<u64>,
    },
    Deleted {
        entity_id: EntityId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedRewardUnlock {
    pub milestone_entity_id: EntityId,
    pub previous_chain_break_review_entity_id: Option<EntityId>,
    pub name: String,
    pub threshold: u64,
    pub budget: Option<u64>,
    pub claimed_at: Option<i64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RewardProjection {
    pub milestones: Vec<RewardMilestoneProjection>,
    pub unlocks: Vec<ProjectedRewardUnlock>,
}

type RewardUnlockKey<'a> = (&'a EntityId, Option<&'a EntityId>);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedSessionReview {
    pub review_entity_id: EntityId,
    pub record_id: RecordId,
    pub session_entity_id: EntityId,
    pub judgment: SessionReviewJudgment,
    pub task_entity_id: EntityId,
    pub task_kind: ReviewedTaskKind,
    pub task_title: String,
    pub actual_seconds: u64,
    pub reflection: Option<String>,
    pub chain_entry_title: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProjectedChain {
    pub links: Vec<ProjectedSessionReview>,
    pub chain_break: Option<ProjectedSessionReview>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionReviewProjection {
    pub ended_chains: Vec<ProjectedChain>,
    pub current_chain: ProjectedChain,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActivityProjection {
    Session {
        entity_id: EntityId,
        ended_at: i64,
        kind: SessionKind,
        outcome: SessionOutcome,
        planned_seconds: u64,
        actual_seconds: u64,
        task_entity_id: Option<EntityId>,
        task_title: Option<String>,
    },
    Deleted {
        entity_id: EntityId,
    },
}

/// Produces the retained record union and its deterministic projections.
pub fn plan_sync(local: &[Record], incoming: &[Record]) -> Result<SyncPlan, String> {
    let retained_records = union(local, incoming)?;
    validate_records(&retained_records)?;
    let task_projections = project_tasks(&retained_records);
    let activity_projections = project_activity(&retained_records);
    let session_review_projection = project_session_reviews(&retained_records);
    let reward_projection = project_rewards(&retained_records);
    Ok(SyncPlan {
        base_records: local.to_vec(),
        retained_records,
        task_projections,
        activity_projections,
        session_review_projection,
        reward_projection,
    })
}

#[must_use]
#[allow(clippy::too_many_lines)]
pub fn project_rewards(records: &[Record]) -> RewardProjection {
    let deleted_chain_anchors = records
        .iter()
        .filter_map(|record| match &record.payload {
            RecordPayload::EndedChainDeleted {
                previous_chain_break_review_entity_id,
                ..
            } => Some(previous_chain_break_review_entity_id.as_ref()),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut milestone_records = BTreeMap::<&EntityId, Vec<&Record>>::new();
    for record in records {
        if matches!(
            record.payload,
            RecordPayload::RewardMilestoneVersion { .. } | RecordPayload::RewardMilestoneDeleted
        ) {
            milestone_records
                .entry(&record.entity_id)
                .or_default()
                .push(record);
        }
    }
    let milestones = milestone_records
        .into_iter()
        .filter_map(|(entity_id, versions)| {
            if versions
                .iter()
                .any(|record| matches!(record.payload, RecordPayload::RewardMilestoneDeleted))
            {
                return Some(RewardMilestoneProjection::Deleted {
                    entity_id: entity_id.clone(),
                });
            }
            versions
                .into_iter()
                .filter_map(|record| match &record.payload {
                    RecordPayload::RewardMilestoneVersion {
                        name,
                        threshold,
                        budget,
                    } => Some((record.mutation_time, &record.id, name, threshold, budget)),
                    _ => None,
                })
                .max_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)))
                .map(
                    |(_, _, name, threshold, budget)| RewardMilestoneProjection::Version {
                        entity_id: entity_id.clone(),
                        name: name.clone(),
                        threshold: *threshold,
                        budget: *budget,
                    },
                )
        })
        .collect();
    let mut unlocks = BTreeMap::<RewardUnlockKey<'_>, ProjectedRewardUnlock>::new();
    let mut claims = BTreeMap::<RewardUnlockKey<'_>, i64>::new();
    for record in records {
        match &record.payload {
            RecordPayload::RewardUnlocked {
                milestone_entity_id,
                previous_chain_break_review_entity_id,
                name,
                threshold,
                budget,
            } => {
                let key = (
                    milestone_entity_id,
                    previous_chain_break_review_entity_id.as_ref(),
                );
                if deleted_chain_anchors.contains(&key.1) {
                    continue;
                }
                unlocks.entry(key).or_insert_with(|| ProjectedRewardUnlock {
                    milestone_entity_id: milestone_entity_id.clone(),
                    previous_chain_break_review_entity_id: previous_chain_break_review_entity_id
                        .clone(),
                    name: name.clone(),
                    threshold: *threshold,
                    budget: *budget,
                    claimed_at: None,
                });
            }
            RecordPayload::RewardClaimed {
                milestone_entity_id,
                previous_chain_break_review_entity_id,
                claimed_at,
                ..
            } => {
                if deleted_chain_anchors.contains(&previous_chain_break_review_entity_id.as_ref()) {
                    continue;
                }
                claims
                    .entry((
                        milestone_entity_id,
                        previous_chain_break_review_entity_id.as_ref(),
                    ))
                    .and_modify(|current| *current = (*current).min(*claimed_at))
                    .or_insert(*claimed_at);
            }
            _ => {}
        }
    }
    for (key, claimed_at) in claims {
        if let Some(unlock) = unlocks.get_mut(&key) {
            unlock.claimed_at = Some(claimed_at);
        }
    }
    RewardProjection {
        milestones,
        unlocks: unlocks.into_values().collect(),
    }
}

/// Projects immutable Session Reviews in source-Session end order. Session Review identity is
/// the stable tie-breaker, so arrival order can never affect chain boundaries.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn project_session_reviews(records: &[Record]) -> SessionReviewProjection {
    let session_ends = records
        .iter()
        .filter_map(|record| match record.payload {
            RecordPayload::SessionEnded { ended_at, .. } => Some((&record.entity_id, ended_at)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut entry_versions = BTreeMap::<&EntityId, &Record>::new();
    let mut deleted_intervals = Vec::new();
    for record in records {
        match record.payload {
            RecordPayload::ChainEntryVersion { .. } => {
                if entry_versions.get(&record.entity_id).is_none_or(|current| {
                    (record.mutation_time, &record.id) > (current.mutation_time, &current.id)
                }) {
                    entry_versions.insert(&record.entity_id, record);
                }
            }
            RecordPayload::EndedChainDeleted {
                ref previous_chain_break_review_entity_id,
                ..
            } => {
                deleted_intervals.push((
                    previous_chain_break_review_entity_id.as_ref(),
                    &record.entity_id,
                ));
            }
            _ => {}
        }
    }
    let mut reviews = records
        .iter()
        .filter_map(|record| match &record.payload {
            RecordPayload::SessionReviewed {
                session_entity_id,
                judgment,
                task_entity_id,
                task_kind,
                task_title,
                actual_seconds,
                reflection,
                chain_entry_title,
            } => {
                let (reflection, chain_entry_title) = entry_versions
                    .get(&record.entity_id)
                    .and_then(|version| match &version.payload {
                        RecordPayload::ChainEntryVersion {
                            reflection,
                            chain_entry_title,
                        } => Some((reflection.clone(), chain_entry_title.clone())),
                        _ => None,
                    })
                    .unwrap_or_else(|| (reflection.clone(), chain_entry_title.clone()));
                Some((
                    *session_ends.get(session_entity_id)?,
                    record.entity_id.clone(),
                    ProjectedSessionReview {
                        review_entity_id: record.entity_id.clone(),
                        record_id: record.id.clone(),
                        session_entity_id: session_entity_id.clone(),
                        judgment: *judgment,
                        task_entity_id: task_entity_id.clone(),
                        task_kind: *task_kind,
                        task_title: task_title.clone(),
                        actual_seconds: *actual_seconds,
                        reflection,
                        chain_entry_title,
                    },
                ))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    reviews.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
    let review_positions = reviews
        .iter()
        .enumerate()
        .map(|(index, (_, entity_id, _))| (entity_id, index))
        .collect::<BTreeMap<_, _>>();
    let mut deleted_positions = std::collections::BTreeSet::new();
    for (previous_break, deleted_break) in deleted_intervals {
        let start = previous_break
            .and_then(|identity| review_positions.get(identity).copied())
            .map_or(0, |index| index.saturating_add(1));
        if let Some(end) = review_positions.get(deleted_break).copied() {
            deleted_positions.extend(start..=end);
        }
    }
    let mut projection = SessionReviewProjection::default();
    for (index, (_, _, review)) in reviews.into_iter().enumerate() {
        if deleted_positions.contains(&index) {
            continue;
        }
        if review.judgment == SessionReviewJudgment::Successful {
            projection.current_chain.links.push(review);
        } else {
            projection.current_chain.chain_break = Some(review);
            let ended = std::mem::take(&mut projection.current_chain);
            projection.ended_chains.push(ended);
        }
    }
    projection
}

#[must_use]
pub fn project_tasks(records: &[Record]) -> Vec<TaskProjection> {
    let system_void_entities = records
        .iter()
        .filter_map(|record| match &record.payload {
            RecordPayload::SessionReviewed {
                task_entity_id,
                task_kind: ReviewedTaskKind::SystemVoid,
                ..
            } => Some(task_entity_id),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    let mut by_entity = BTreeMap::<EntityId, Vec<&Record>>::new();
    for record in records {
        if system_void_entities.contains(&record.entity_id) {
            continue;
        }
        by_entity
            .entry(record.entity_id.clone())
            .or_default()
            .push(record);
    }
    by_entity
        .into_iter()
        .filter_map(|(entity_id, records)| {
            if records
                .iter()
                .any(|record| matches!(record.payload, RecordPayload::TaskDeleted))
            {
                let last_title = records
                    .iter()
                    .filter_map(|record| match &record.payload {
                        RecordPayload::TaskVersion { title, .. } => {
                            Some((record.mutation_time, &record.id, title))
                        }
                        _ => None,
                    })
                    .max_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)))
                    .map(|(_, _, title)| title.clone());
                return Some(TaskProjection::Deleted {
                    entity_id,
                    last_title,
                });
            }
            records
                .into_iter()
                .filter_map(|record| match &record.payload {
                    RecordPayload::TaskVersion { title, status } => {
                        Some((record.mutation_time, &record.id, title, *status))
                    }
                    RecordPayload::TaskDeleted
                    | RecordPayload::SessionEnded { .. }
                    | RecordPayload::SessionDeleted
                    | RecordPayload::SessionReviewed { .. }
                    | RecordPayload::ChainEntryVersion { .. }
                    | RecordPayload::EndedChainDeleted { .. }
                    | RecordPayload::RewardMilestoneVersion { .. }
                    | RecordPayload::RewardMilestoneDeleted
                    | RecordPayload::RewardUnlocked { .. }
                    | RecordPayload::RewardClaimed { .. } => None,
                })
                .max_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)))
                .map(|(_, record_id, title, status)| TaskProjection::Version {
                    entity_id: entity_id.clone(),
                    record_id: record_id.clone(),
                    title: title.clone(),
                    status,
                })
        })
        .collect()
}

#[must_use]
pub fn project_activity(records: &[Record]) -> Vec<ActivityProjection> {
    let mut by_entity = BTreeMap::<EntityId, Vec<&Record>>::new();
    for record in records {
        if matches!(
            record.payload,
            RecordPayload::SessionEnded { .. } | RecordPayload::SessionDeleted
        ) {
            by_entity
                .entry(record.entity_id.clone())
                .or_default()
                .push(record);
        }
    }
    by_entity
        .into_iter()
        .filter_map(|(entity_id, records)| {
            if records
                .iter()
                .any(|record| matches!(record.payload, RecordPayload::SessionDeleted))
            {
                return Some(ActivityProjection::Deleted { entity_id });
            }
            records
                .into_iter()
                .find_map(|record| match &record.payload {
                    RecordPayload::SessionEnded {
                        ended_at,
                        kind,
                        outcome,
                        planned_seconds,
                        actual_seconds,
                        task_entity_id,
                        task_title,
                    } => Some(ActivityProjection::Session {
                        entity_id: entity_id.clone(),
                        ended_at: *ended_at,
                        kind: *kind,
                        outcome: *outcome,
                        planned_seconds: *planned_seconds,
                        actual_seconds: *actual_seconds,
                        task_entity_id: task_entity_id.clone(),
                        task_title: task_title.clone(),
                    }),
                    _ => None,
                })
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn validate_records(records: &[Record]) -> Result<(), String> {
    let unique = union(&[], records)?;
    if unique.len() != records.len() {
        return Err("duplicate synchronization record identity".into());
    }
    for record in records {
        match &record.payload {
            RecordPayload::TaskVersion { title, .. } => {
                pomotui_domain::TaskTitle::parse(title)
                    .map_err(|error| format!("invalid synchronized Task: {error}"))?;
            }
            RecordPayload::SessionEnded {
                ended_at,
                kind,
                outcome,
                planned_seconds,
                actual_seconds,
                task_entity_id,
                task_title,
            } => {
                chrono::DateTime::from_timestamp(*ended_at, 0)
                    .ok_or_else(|| format!("invalid Session end time {ended_at}"))?;
                if task_entity_id.is_some() != task_title.is_some() {
                    return Err(
                        "synchronized Session Task identity and title must appear together".into(),
                    );
                }
                if !matches!(kind, SessionKind::Focus) && task_entity_id.is_some() {
                    return Err("synchronized Break Session cannot be attributed to a Task".into());
                }
                if matches!(outcome, SessionOutcome::Skipped) && *actual_seconds != 0 {
                    return Err(
                        "skipped synchronized Session must have zero actual duration".into(),
                    );
                }
                if matches!(outcome, SessionOutcome::Completed) && actual_seconds < planned_seconds
                {
                    return Err(
                        "completed synchronized Session cannot be shorter than planned".into(),
                    );
                }
            }
            RecordPayload::TaskDeleted
            | RecordPayload::SessionDeleted
            | RecordPayload::EndedChainDeleted { .. }
            | RecordPayload::RewardMilestoneDeleted => {}
            RecordPayload::RewardClaimed { evidence, .. } => {
                if let Some(e) = evidence {
                    let support = e
                        .supporting_review_entity_ids
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>();
                    let observed = e
                        .observed_review_entity_ids
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>();
                    let carried = e
                        .carried_review_entity_ids
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>();
                    if e.threshold == 0
                        || support.len() != e.supporting_review_entity_ids.len()
                        || observed.len() != e.observed_review_entity_ids.len()
                        || !support.is_subset(&observed)
                        || carried.len() != e.carried_review_entity_ids.len()
                        || !carried.is_subset(&observed)
                        || !carried.is_disjoint(&support)
                        || !observed.contains(&e.frontier_review_entity_id)
                    {
                        return Err("invalid immutable reward claim evidence".into());
                    }
                }
            }
            RecordPayload::RewardMilestoneVersion {
                name, threshold, ..
            } => {
                if name.trim().is_empty() || *threshold == 0 {
                    return Err("invalid synchronized Reward Milestone".into());
                }
            }
            RecordPayload::RewardUnlocked {
                name, threshold, ..
            } => {
                if name.trim().is_empty() || *threshold == 0 {
                    return Err("invalid synchronized reward unlock".into());
                }
            }
            RecordPayload::ChainEntryVersion {
                reflection,
                chain_entry_title,
            } => {
                if reflection
                    .as_deref()
                    .is_some_and(|value| value.trim().is_empty())
                    || chain_entry_title
                        .as_deref()
                        .is_some_and(|value| value.trim().is_empty())
                {
                    return Err("synchronized Chain Entry text cannot be blank".into());
                }
                if reflection.is_none() && chain_entry_title.is_none() {
                    return Err("synchronized Chain Entry version must change text".into());
                }
            }
            RecordPayload::SessionReviewed {
                task_title,
                judgment,
                reflection,
                ..
            } => {
                pomotui_domain::TaskTitle::parse(task_title).map_err(|error| {
                    format!("invalid synchronized Session Review Task: {error}")
                })?;
                if *judgment == SessionReviewJudgment::Failed
                    && reflection
                        .as_deref()
                        .is_none_or(|value| value.trim().is_empty())
                {
                    return Err("failed synchronized Session Review requires a Reflection".into());
                }
            }
        }
    }
    let mut entity_kinds = BTreeMap::<&EntityId, (&'static str, usize)>::new();
    for record in records {
        let (kind, immutable_fact) = match record.payload {
            RecordPayload::TaskVersion { .. } | RecordPayload::TaskDeleted => ("Task", false),
            RecordPayload::SessionEnded { .. } => ("Session", true),
            RecordPayload::SessionDeleted => ("Session", false),
            RecordPayload::SessionReviewed { .. } => ("Review", true),
            RecordPayload::ChainEntryVersion { .. } | RecordPayload::EndedChainDeleted { .. } => {
                ("Review", false)
            }
            RecordPayload::RewardMilestoneVersion { .. }
            | RecordPayload::RewardMilestoneDeleted => ("Reward Milestone", false),
            RecordPayload::RewardUnlocked { .. } | RecordPayload::RewardClaimed { .. } => {
                ("Reward", false)
            }
        };
        let entry = entity_kinds.entry(&record.entity_id).or_insert((kind, 0));
        if entry.0 != kind {
            return Err("synchronization entity mixes Task and Session records".into());
        }
        if immutable_fact {
            entry.1 += 1;
            if entry.1 > 1 {
                return Err("synchronized Session has more than one ended fact".into());
            }
        }
    }
    let task_entities = records
        .iter()
        .filter_map(|record| {
            matches!(record.payload, RecordPayload::TaskVersion { .. }).then_some(&record.entity_id)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let milestone_entities = records
        .iter()
        .filter_map(|record| {
            matches!(record.payload, RecordPayload::RewardMilestoneVersion { .. })
                .then_some(&record.entity_id)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let failed_reviews = records
        .iter()
        .filter_map(|record| {
            matches!(
                record.payload,
                RecordPayload::SessionReviewed {
                    judgment: SessionReviewJudgment::Failed,
                    ..
                }
            )
            .then_some(&record.entity_id)
        })
        .collect::<std::collections::BTreeSet<_>>();
    for record in records {
        match &record.payload {
            RecordPayload::RewardUnlocked {
                milestone_entity_id,
                previous_chain_break_review_entity_id,
                ..
            }
            | RecordPayload::RewardClaimed {
                milestone_entity_id,
                previous_chain_break_review_entity_id,
                ..
            } => {
                if !milestone_entities.contains(milestone_entity_id) {
                    return Err(
                        "synchronized Reward references unknown Reward Milestone identity".into(),
                    );
                }
                if previous_chain_break_review_entity_id
                    .as_ref()
                    .is_some_and(|identity| !failed_reviews.contains(identity))
                {
                    return Err(
                        "synchronized Reward references unknown Chain Break identity".into(),
                    );
                }
            }
            _ => {}
        }
    }
    let sessions = records
        .iter()
        .filter_map(|record| match &record.payload {
            RecordPayload::SessionEnded { kind, outcome, .. } => {
                Some((&record.entity_id, (*kind, *outcome)))
            }
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut reviewed_sessions = std::collections::BTreeSet::new();
    for record in records {
        if let RecordPayload::SessionReviewed {
            session_entity_id,
            task_entity_id,
            task_kind,
            actual_seconds,
            ..
        } = &record.payload
        {
            let Some((kind, outcome)) = sessions.get(session_entity_id) else {
                return Err(
                    "synchronized Session Review references unknown Session identity".into(),
                );
            };
            if *kind != SessionKind::Focus || *outcome == SessionOutcome::Skipped {
                return Err("only an ended reviewable Focus Session can be reviewed".into());
            }
            if !reviewed_sessions.insert(session_entity_id) {
                return Err("synchronized Session has more than one Review".into());
            }
            if *task_kind == ReviewedTaskKind::Regular && !task_entities.contains(task_entity_id) {
                return Err("synchronized Session Review references unknown Task identity".into());
            }
            let session_actual = records.iter().find_map(|candidate| {
                (&candidate.entity_id == session_entity_id)
                    .then_some(match candidate.payload {
                        RecordPayload::SessionEnded { actual_seconds, .. } => Some(actual_seconds),
                        _ => None,
                    })
                    .flatten()
            });
            if session_actual != Some(*actual_seconds) {
                return Err(
                    "synchronized Session Review duration differs from its source Session".into(),
                );
            }
        }
    }
    let reviews = records
        .iter()
        .filter_map(|record| match record.payload {
            RecordPayload::SessionReviewed { judgment, .. } => Some((&record.entity_id, judgment)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    for record in records {
        match record.payload {
            RecordPayload::ChainEntryVersion { .. } if !reviews.contains_key(&record.entity_id) => {
                return Err(
                    "synchronized Chain Entry version references unknown Review identity".into(),
                );
            }
            RecordPayload::EndedChainDeleted { .. }
                if reviews.get(&record.entity_id) != Some(&SessionReviewJudgment::Failed) =>
            {
                return Err(
                    "synchronized Ended Chain deletion references unknown Chain Break".into(),
                );
            }
            RecordPayload::EndedChainDeleted {
                previous_chain_break_review_entity_id: Some(ref previous),
                ..
            } if reviews.get(previous) != Some(&SessionReviewJudgment::Failed) => {
                return Err(
                    "synchronized Ended Chain deletion references unknown previous Chain Break"
                        .into(),
                );
            }
            _ => {}
        }
    }
    let review_order_key = |identity: &EntityId| {
        records.iter().find_map(|record| {
            (&record.entity_id == identity).then(|| match &record.payload {
                RecordPayload::SessionReviewed {
                    session_entity_id, ..
                } => records.iter().find_map(|candidate| {
                    (&candidate.entity_id == session_entity_id).then(|| {
                        if let RecordPayload::SessionEnded { ended_at, .. } = candidate.payload {
                            Some((ended_at, identity.clone()))
                        } else {
                            None
                        }
                    })?
                }),
                _ => None,
            })?
        })
    };
    for record in records {
        if let RecordPayload::EndedChainDeleted {
            previous_chain_break_review_entity_id: Some(previous),
            ..
        } = &record.payload
            && review_order_key(previous) >= review_order_key(&record.entity_id)
        {
            return Err(
                "synchronized Ended Chain deletion boundaries are not in Review Order".into(),
            );
        }
    }
    for tombstone in records {
        let RecordPayload::EndedChainDeleted {
            previous_chain_break_review_entity_id,
            deleted_review_entity_ids,
            observed_chain_break_review_entity_ids,
        } = &tombstone.payload
        else {
            continue;
        };
        let unique = deleted_review_entity_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        if deleted_review_entity_ids.is_empty()
            || unique.len() != deleted_review_entity_ids.len()
            || deleted_review_entity_ids.last() != Some(&tombstone.entity_id)
        {
            return Err(
                "synchronized Ended Chain deletion must list each deleted Review once and end with its Chain Break"
                    .into(),
            );
        }
        for identity in deleted_review_entity_ids {
            let Some(judgment) = reviews.get(identity) else {
                return Err(
                    "synchronized Ended Chain deletion references unknown Review identity".into(),
                );
            };
            if identity != &tombstone.entity_id && *judgment != SessionReviewJudgment::Successful {
                return Err(
                    "synchronized Ended Chain deletion contains an interior Chain Break".into(),
                );
            }
        }
        if deleted_review_entity_ids
            .windows(2)
            .any(|pair| review_order_key(&pair[0]) >= review_order_key(&pair[1]))
        {
            return Err("synchronized Ended Chain deletion Reviews are not in Review Order".into());
        }
        let observed_unique = observed_chain_break_review_entity_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        if observed_unique.len() != observed_chain_break_review_entity_ids.len()
            || observed_chain_break_review_entity_ids
                .iter()
                .any(|identity| reviews.get(identity) != Some(&SessionReviewJudgment::Failed))
            || observed_chain_break_review_entity_ids
                .windows(2)
                .any(|pair| review_order_key(&pair[0]) >= review_order_key(&pair[1]))
        {
            return Err(
                "synchronized Ended Chain deletion has invalid observed Chain Breaks".into(),
            );
        }
        let Some(target_index) = observed_chain_break_review_entity_ids
            .iter()
            .position(|identity| identity == &tombstone.entity_id)
        else {
            return Err("synchronized Ended Chain deletion did not observe its Chain Break".into());
        };
        let expected_previous = target_index
            .checked_sub(1)
            .map(|index| &observed_chain_break_review_entity_ids[index]);
        if previous_chain_break_review_entity_id.as_ref() != expected_previous {
            return Err(
                "synchronized Ended Chain deletion boundaries are not adjacent among observed Chain Breaks"
                    .into(),
            );
        }
    }
    for record in records {
        if let RecordPayload::SessionEnded {
            task_entity_id: Some(task_entity_id),
            ..
        } = &record.payload
            && !task_entities.contains(task_entity_id)
        {
            return Err(format!(
                "synchronized Session references unknown Task identity {}",
                task_entity_id.as_str()
            ));
        }
    }
    Ok(())
}

fn checksum(records: &[Record]) -> Result<String, String> {
    let canonical = serde_json::to_vec(records).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(canonical)))
}

impl fmt::Display for RecordId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Immutable observed support for one logical milestone/chain claim. Legacy claims omit it.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ClaimEvidence {
    pub threshold: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub carried_review_entity_ids: Vec<EntityId>,
    pub supporting_review_entity_ids: Vec<EntityId>,
    pub observed_review_entity_ids: Vec<EntityId>,
    pub frontier_review_entity_id: EntityId,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct RewardDebt {
    pub milestone_entity_id: EntityId,
    pub outstanding: u64,
    pub repaid: u64,
    pub excess_credit: u64,
    pub excess_review_entity_ids: Vec<EntityId>,
    pub repayment_review_entity_ids: Vec<EntityId>,
}
/// Replays accounting in source-session Review Order, independently of delivery time.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn project_reward_debt(records: &[Record]) -> Vec<RewardDebt> {
    let ends = records
        .iter()
        .filter_map(|r| match &r.payload {
            RecordPayload::SessionEnded { ended_at, .. } => Some((&r.entity_id, *ended_at)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let mut reviews = records
        .iter()
        .filter_map(|r| match &r.payload {
            RecordPayload::SessionReviewed {
                session_entity_id,
                judgment,
                ..
            } => ends
                .get(session_entity_id)
                .map(|end| ((*end, r.entity_id.clone()), *judgment)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>()
        .into_iter()
        .collect::<Vec<_>>();
    reviews.sort_by(|a, b| a.0.cmp(&b.0));
    let positions = reviews
        .iter()
        .enumerate()
        .map(|(i, (key, _))| (&key.1, i))
        .collect::<BTreeMap<_, _>>();
    let mut claims = BTreeMap::new();
    for r in records {
        if let RecordPayload::RewardClaimed {
            milestone_entity_id,
            previous_chain_break_review_entity_id,
            evidence: Some(e),
            ..
        } = &r.payload
        {
            let key = (
                milestone_entity_id.clone(),
                previous_chain_break_review_entity_id.clone(),
            );
            claims
                .entry(key)
                .and_modify(|current: &mut &ClaimEvidence| {
                    if e < *current {
                        *current = e;
                    }
                })
                .or_insert(e);
        }
    }
    let mut accounts = BTreeMap::<EntityId, RewardDebt>::new();
    let mut consumed_credits = BTreeMap::<EntityId, std::collections::BTreeSet<EntityId>>::new();
    let mut obligations = claims.into_iter().collect::<Vec<_>>();
    obligations.sort_by_key(|(key, e)| {
        (
            positions
                .get(&e.frontier_review_entity_id)
                .copied()
                .unwrap_or(usize::MAX),
            key.clone(),
        )
    });
    for ((milestone, _), e) in obligations {
        let frontier = positions.get(&e.frontier_review_entity_id).copied();
        let last_break = frontier.and_then(|end| {
            (0..=end)
                .rev()
                .find(|i| reviews[*i].1 == SessionReviewJudgment::Failed)
        });
        let snapshot_support = e
            .supporting_review_entity_ids
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|id| {
                positions.get(*id).is_some_and(|i| {
                    Some(*i) > last_break
                        && reviews[*i].1 == SessionReviewJudgment::Successful
                        && !accounts
                            .get(&milestone)
                            .is_some_and(|a| a.repayment_review_entity_ids.contains(id))
                })
            })
            .count() as u64;
        // Late successful reviews before the immutable claim frontier correct its support.
        // They do not erase the capacity established by its original observed evidence.
        let late_support = frontier.map_or(0, |end| {
            reviews
                .iter()
                .enumerate()
                .take(end + 1)
                .filter(|(i, ((_, id), judgment))| {
                    Some(*i) > last_break
                        && *judgment == SessionReviewJudgment::Successful
                        && !e.observed_review_entity_ids.contains(id)
                        && !e.supporting_review_entity_ids.contains(id)
                        && !accounts
                            .get(&milestone)
                            .is_some_and(|a| a.repayment_review_entity_ids.contains(id))
                })
                .count() as u64
        });
        let account = accounts
            .entry(milestone.clone())
            .or_insert_with(|| RewardDebt {
                milestone_entity_id: milestone,
                outstanding: 0,
                repaid: 0,
                excess_credit: 0,
                excess_review_entity_ids: Vec::new(),
                repayment_review_entity_ids: Vec::new(),
            });
        let consumed = consumed_credits
            .entry(account.milestone_entity_id.clone())
            .or_default();
        let carried_support = e
            .carried_review_entity_ids
            .iter()
            .filter(|id| {
                account.repayment_review_entity_ids.contains(id) && consumed.insert((*id).clone())
            })
            .count() as u64;
        let shortfall = e
            .threshold
            .saturating_sub(snapshot_support + late_support + carried_support);
        let mut capacity = e
            .threshold
            .saturating_sub(snapshot_support + carried_support);
        if let Some(frontier) = frontier {
            for (_, ((_, id), judgment)) in reviews.iter().enumerate().skip(frontier + 1) {
                if capacity == 0 {
                    break;
                }
                if *judgment == SessionReviewJudgment::Successful
                    && !e.observed_review_entity_ids.contains(id)
                    && !account.repayment_review_entity_ids.contains(id)
                {
                    account.repayment_review_entity_ids.push(id.clone());
                    account.repaid += 1;
                    capacity -= 1;
                }
            }
        }
        account.outstanding += shortfall;
    }
    for account in accounts.values_mut() {
        let shortfall = account.outstanding;
        let consumed = consumed_credits
            .get(&account.milestone_entity_id)
            .map_or(0, |ids| ids.len() as u64);
        account.outstanding = shortfall
            .saturating_add(consumed)
            .saturating_sub(account.repaid);
        account.excess_review_entity_ids = account
            .repayment_review_entity_ids
            .iter()
            .filter(|id| {
                !consumed_credits
                    .get(&account.milestone_entity_id)
                    .is_some_and(|ids| ids.contains(*id))
            })
            .skip(usize::try_from(shortfall).unwrap_or(usize::MAX))
            .cloned()
            .collect();
        account.excess_credit = account.excess_review_entity_ids.len() as u64;
    }
    accounts.into_values().collect()
}

/// Merge metadata before projecting records; retired beginnings cannot resurrect.
pub fn union_documents(left: &Document, right: &Document) -> Result<Document, String> {
    let beginning = left.beginning().max(right.beginning()).clone();
    let records = union(left.records(), right.records())?.into_iter().filter(|r| r.beginning == beginning).collect::<Vec<_>>();
    Document::with_beginning(beginning, &records)
}
