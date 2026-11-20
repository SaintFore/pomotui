use crate::{Beginning, EntityId, Record, RecordPayload};
use chrono::{Datelike, LocalResult, NaiveDate, Offset, TimeZone};
use chrono_tz::GapInfo;
pub use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A commitment in the selected shared time zone, inclusive of both dates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DailyGoalPolicy {
    pub quota: u64,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub time_zone: Tz,
}

impl Ord for DailyGoalPolicy {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (
            self.start_date,
            self.end_date,
            self.time_zone.name(),
            self.quota,
        )
            .cmp(&(
                other.start_date,
                other.end_date,
                other.time_zone.name(),
                other.quota,
            ))
    }
}

impl PartialOrd for DailyGoalPolicy {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl DailyGoalPolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.quota == 0 {
            return Err("Daily Chain Goal quota must be a positive whole number".into());
        }
        if !(1..=9999).contains(&self.start_date.year()) {
            return Err("Daily Chain Goal start date is outside the supported range".into());
        }
        if self
            .end_date
            .is_some_and(|end| end < self.start_date || !(1..=9999).contains(&end.year()))
        {
            return Err("Daily Chain Goal end date must be on or after its start date".into());
        }
        Ok(())
    }

    #[must_use]
    pub fn contains(&self, date: NaiveDate) -> bool {
        self.start_date <= date && self.end_date.is_none_or(|end| date <= end)
    }
}

#[derive(Clone, Copy)]
struct Revision<'a> {
    record: &'a Record,
    effective_at: i64,
    settings: Option<&'a DailyGoalPolicy>,
}

/// Resolves the winning revision at each instant, keeping earlier commitments.
pub struct DailyGoalSchedule<'a> {
    revisions: Vec<Revision<'a>>,
}

impl<'a> DailyGoalSchedule<'a> {
    #[must_use]
    pub fn new(records: &'a [Record]) -> Self {
        let revisions = records
            .iter()
            .filter_map(|record| match &record.payload {
                RecordPayload::DailyGoalConfigured {
                    effective_at,
                    settings,
                } => Some(Revision {
                    record,
                    effective_at: *effective_at,
                    settings: settings.as_ref(),
                }),
                _ => None,
            })
            .collect();
        Self { revisions }
    }

    fn revision_at(&self, timestamp: i64) -> Option<Revision<'a>> {
        self.revisions
            .iter()
            .filter(|revision| revision.effective_at <= timestamp)
            .max_by_key(|revision| (revision.record.mutation_time, &revision.record.id))
            .copied()
    }

    #[must_use]
    pub fn active_at(&self, timestamp: i64) -> Option<&'a DailyGoalPolicy> {
        let settings = self.revision_at(timestamp)?.settings?;
        settings
            .contains(local_date(settings.time_zone, timestamp)?)
            .then_some(settings)
    }

    /// Finds a future commitment or explicit disabling that can actually take effect.
    #[must_use]
    pub fn next_change(&self, timestamp: i64) -> Option<(i64, Tz, Option<&'a DailyGoalPolicy>)> {
        let current = self.active_at(timestamp);
        let candidates = self
            .revisions
            .iter()
            .filter_map(|revision| {
                let start = if let Some(settings) = revision.settings {
                    revision
                        .effective_at
                        .max(day_start(settings.time_zone, settings.start_date)?)
                } else {
                    revision.effective_at
                };
                (start > timestamp).then_some(start)
            })
            .collect::<std::collections::BTreeSet<_>>();
        for start in candidates {
            let Some(winner) = self.revision_at(start) else {
                continue;
            };
            if let Some(next) = self.active_at(start) {
                if Some(next) != current {
                    return Some((start, next.time_zone, Some(next)));
                }
            } else if winner.effective_at == start
                && let Some(current) = current
            {
                return Some((start, current.time_zone, None));
            }
        }
        None
    }
}

#[must_use]
pub fn daily_goal_entity(beginning: &Beginning) -> EntityId {
    EntityId(goal_identity("daily-goal-settings", beginning, ""))
}

fn goal_identity(namespace: &str, beginning: &Beginning, name: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(namespace);
    hash.update(beginning.generation.to_be_bytes());
    hash.update(beginning.id.as_bytes());
    hash.update(name);
    let digest = hash.finalize();
    let mut bytes: [u8; 16] = digest[..16]
        .try_into()
        .expect("SHA-256 digest has 32 bytes");
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes).to_string()
}

#[must_use]
pub fn local_date(time_zone: Tz, timestamp: i64) -> Option<NaiveDate> {
    let local = time_zone.timestamp_opt(timestamp, 0).single()?;
    local
        .naive_utc()
        .checked_add_offset(local.offset().fix())
        .map(|date_time| date_time.date())
}

/// Returns the first instant at the civil boundary. A skipped date and its
/// successor have the same boundary and therefore create no deadline interval.
#[must_use]
pub fn day_start(time_zone: Tz, date: NaiveDate) -> Option<i64> {
    let midnight = date.and_hms_opt(0, 0, 0)?;
    match time_zone.from_local_datetime(&midnight) {
        LocalResult::Single(instant) => Some(instant.timestamp()),
        LocalResult::Ambiguous(first, second) => Some(first.timestamp().min(second.timestamp())),
        LocalResult::None => Some(GapInfo::new(&midnight, &time_zone)?.end?.timestamp()),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DailyGoalMiss {
    pub date: NaiveDate,
    pub cutoff_at: i64,
    pub time_zone: Tz,
    pub achieved: u64,
    pub quota: u64,
}

impl DailyGoalMiss {
    #[must_use]
    pub fn boundary(&self, beginning: &Beginning) -> Record {
        boundary_record(beginning, self.date, self.time_zone, self.cutoff_at)
    }
}

pub(crate) fn boundary_record(
    beginning: &Beginning,
    date: NaiveDate,
    time_zone: Tz,
    cutoff_at: i64,
) -> Record {
    let key = format!("{}/{date}", time_zone.name());
    Record::in_beginning(
        beginning.clone(),
        crate::RecordId(goal_identity("daily-goal-boundary-record", beginning, &key)),
        EntityId(goal_identity("daily-goal-boundary-entity", beginning, &key)),
        crate::MutationInstant(cutoff_at.saturating_mul(1_000)),
        RecordPayload::DailyGoalBoundary {
            date,
            cutoff_at,
            time_zone: time_zone.name().into(),
        },
    )
}

/// Counts a qualifying Session once even when its reviews arrived in different imports.
pub struct DailyGoalCredit {
    completed_at: Vec<i64>,
    first_link_at: Option<i64>,
}

impl DailyGoalCredit {
    #[must_use]
    pub fn new(records: &[Record]) -> Self {
        use crate::{SessionKind, SessionOutcome, SessionReviewJudgment};

        let successful_sessions = records
            .iter()
            .filter_map(|record| match &record.payload {
                RecordPayload::SessionReviewed {
                    session_entity_id,
                    judgment: SessionReviewJudgment::Successful,
                    ..
                } => Some(session_entity_id),
                _ => None,
            })
            .collect::<std::collections::BTreeSet<_>>();
        let mut completed_at = Vec::new();
        let mut first_link_at = None::<i64>;
        for record in records {
            if !successful_sessions.contains(&record.entity_id) {
                continue;
            }
            if let RecordPayload::SessionEnded {
                ended_at,
                kind: SessionKind::Focus,
                outcome,
                planned_seconds,
                actual_seconds,
                ..
            } = record.payload
            {
                first_link_at = Some(first_link_at.map_or(ended_at, |first| first.min(ended_at)));
                if outcome == SessionOutcome::Completed
                    && planned_seconds > 0
                    && actual_seconds >= planned_seconds
                {
                    completed_at.push(ended_at);
                }
            }
        }
        Self {
            completed_at,
            first_link_at,
        }
    }

    #[must_use]
    pub fn on_date(&self, time_zone: Tz, date: NaiveDate) -> u64 {
        self.completed_at
            .iter()
            .filter(|timestamp| local_date(time_zone, **timestamp) == Some(date))
            .count() as u64
    }

    fn by_date(&self, time_zone: Tz) -> std::collections::BTreeMap<NaiveDate, u64> {
        let mut days = std::collections::BTreeMap::new();
        for timestamp in &self.completed_at {
            if let Some(date) = local_date(time_zone, *timestamp) {
                *days.entry(date).or_default() += 1;
            }
        }
        days
    }
}

impl DailyGoalSchedule<'_> {
    /// Returns the next deadline of a full civil day covered by the schedule.
    #[must_use]
    pub fn next_cutoff(&self, through: i64) -> Option<i64> {
        self.revisions
            .iter()
            .filter_map(|revision| {
                let settings = revision.settings?;
                let zone = settings.time_zone;
                let mut date =
                    local_date(zone, through.max(revision.effective_at))?.max(settings.start_date);
                loop {
                    if !settings.contains(date) {
                        return None;
                    }
                    let start = day_start(zone, date)?;
                    let next = date.succ_opt()?;
                    let cutoff = day_start(zone, next)?;
                    if start >= revision.effective_at && start < cutoff && cutoff > through {
                        let winner = self.revision_at(start)?;
                        return (winner.record.id == revision.record.id).then_some(cutoff);
                    }
                    date = next;
                }
            })
            .min()
    }

    /// Returns calendar misses up to the observation. The chain projector
    /// decides which misses end a nonempty chain.
    #[must_use]
    pub fn misses(&self, credit: &DailyGoalCredit, through: i64) -> Vec<DailyGoalMiss> {
        let Some(first_link_at) = credit.first_link_at else {
            return Vec::new();
        };
        let mut zones = std::collections::BTreeMap::new();
        for revision in &self.revisions {
            let Some(settings) = revision.settings else {
                continue;
            };
            let zone = settings.time_zone;
            let Some(first) = local_date(zone, first_link_at.max(revision.effective_at)) else {
                continue;
            };
            let Some(last) = local_date(zone, through) else {
                continue;
            };
            let first = first.max(settings.start_date);
            let last = settings.end_date.map_or(last, |end| end.min(last));
            if first <= last {
                zones
                    .entry(zone.name())
                    .or_insert_with(|| (zone, Vec::new()))
                    .1
                    .push((first, last));
            }
        }
        let mut misses = Vec::new();
        for (zone, mut ranges) in zones.into_values() {
            let credit = credit.by_date(zone);
            ranges.sort_unstable();
            let mut examined_through = None::<NaiveDate>;
            for (first, last) in ranges {
                let start = examined_through
                    .and_then(|date| date.succ_opt())
                    .map_or(first, |date| first.max(date));
                for date in start.iter_days().take_while(|date| *date <= last) {
                    let achieved = credit.get(&date).copied().unwrap_or(0);
                    if let Some(miss) = self.miss_for_date(zone, date, achieved, through) {
                        misses.push(miss);
                    }
                }
                examined_through =
                    Some(examined_through.map_or(last, |previous| previous.max(last)));
            }
        }
        misses
    }

    fn miss_for_date(
        &self,
        zone: Tz,
        date: NaiveDate,
        achieved: u64,
        through: i64,
    ) -> Option<DailyGoalMiss> {
        let start = day_start(zone, date)?;
        let cutoff_at = day_start(zone, date.succ_opt()?)?;
        if start >= cutoff_at || cutoff_at > through {
            return None;
        }
        let policy = self.revision_at(start)?.settings?;
        if policy.time_zone != zone || !policy.contains(date) {
            return None;
        }
        (achieved < policy.quota).then_some(DailyGoalMiss {
            date,
            cutoff_at,
            time_zone: zone,
            achieved,
            quota: policy.quota,
        })
    }
}
