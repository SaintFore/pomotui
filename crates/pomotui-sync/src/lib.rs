//! Provider-neutral synchronization document and convergence engine.

#![allow(clippy::missing_errors_doc)]

use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};

pub const FORMAT_VERSION: u16 = 4;
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
    },
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Record {
    pub id: RecordId,
    pub entity_id: EntityId,
    pub mutation_time: MutationInstant,
    pub payload: RecordPayload,
}

impl Record {
    #[must_use]
    pub const fn new(
        id: RecordId,
        entity_id: EntityId,
        mutation_time: MutationInstant,
        payload: RecordPayload,
    ) -> Self {
        Self {
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
    format: String,
    version: u16,
    integrity: Integrity,
    records: Vec<Record>,
}

impl Document {
    pub fn new(records: &[Record]) -> Result<Self, String> {
        let records = union(&[], records)?;
        validate_records(&records)?;
        Ok(Self {
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
        let document: Self = serde_json::from_str(source)
            .map_err(|error| format!("invalid sync document: {error}"))?;
        if document.format != FORMAT_NAME {
            return Err("unsupported sync document format".into());
        }
        if document.version != FORMAT_VERSION {
            return Err(format!(
                "unsupported sync document version {}; reset local pre-release data with `pomotui reset --all-data --confirm`",
                document.version
            ));
        }
        validate_records(&document.records)?;
        if document.integrity.record_count != document.records.len() {
            return Err("sync document integrity check failed".into());
        }
        if document.integrity.records_sha256 != checksum(&document.records)? {
            return Err("sync document checksum does not match its records".into());
        }
        Ok(document)
    }

    pub fn to_json(&self) -> Result<String, String> {
        let canonical = Self::new(&self.records)?;
        let mut source =
            serde_json::to_string_pretty(&canonical).map_err(|error| error.to_string())?;
        source.push('\n');
        Ok(source)
    }

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
            return Err(format!(
                "conflicting synchronization record {}",
                record.id.as_str()
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
    let mut by_entity = BTreeMap::<EntityId, Vec<&Record>>::new();
    for record in records {
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
            | RecordPayload::RewardMilestoneDeleted
            | RecordPayload::RewardClaimed { .. } => {}
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
            if !task_entities.contains(task_entity_id) {
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
