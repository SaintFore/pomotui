use chrono::NaiveDate;
use pomotui_protocol::{
    DailyChainGoalChange, DailyChainGoalSettings, DailyChainGoalSnapshot, Response,
};
use pomotui_sync::{
    DailyGoalCredit, DailyGoalPolicy, DailyGoalSchedule, ProjectedChainBreakSource, Record,
    RecordId, RecordPayload, Tz, daily_goal_entity, day_start, local_date, project_rewards,
    project_session_reviews_with_daily_goal,
};

use crate::Service;

pub(super) fn device_time_zone() -> Tz {
    std::env::var("TZ")
        .ok()
        .and_then(|name| name.trim_start_matches(':').parse().ok())
        .or_else(|| {
            iana_time_zone::get_timezone()
                .ok()
                .and_then(|name| name.parse().ok())
        })
        .unwrap_or(Tz::UTC)
}

fn settings_summary(settings: &DailyGoalPolicy) -> DailyChainGoalSettings {
    DailyChainGoalSettings {
        quota: settings.quota,
        start_date: Some(settings.start_date.to_string()),
        end_date: settings.end_date.map(|date| date.to_string()),
        time_zone: settings.time_zone.name().into(),
    }
}

fn parse_date(value: &str) -> Result<NaiveDate, String> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| "Daily Chain Goal dates must use YYYY-MM-DD")?;
    if date.to_string() != value {
        return Err("Daily Chain Goal dates must use YYYY-MM-DD".into());
    }
    Ok(date)
}

impl Service {
    pub(super) fn daily_chain_goal_snapshot(&self) -> DailyChainGoalSnapshot {
        let schedule = DailyGoalSchedule::new(&self.sync.records);
        let active = schedule.active_at(self.wall);
        let next = schedule.next_change(self.wall);
        let time_zone = active.map_or_else(
            || next.map_or(self.device_time_zone, |(_, zone, _)| zone),
            |settings| settings.time_zone,
        );
        DailyChainGoalSnapshot {
            device_time_zone: self.device_time_zone.name().into(),
            device_local_date: local_date(self.device_time_zone, self.wall)
                .map_or_else(String::new, |date| date.to_string()),
            date: local_date(time_zone, self.wall)
                .map_or_else(String::new, |date| date.to_string()),
            achieved: active.map_or(0, |settings| {
                local_date(settings.time_zone, self.wall).map_or(0, |date| {
                    DailyGoalCredit::new(&self.sync.records).on_date(settings.time_zone, date)
                })
            }),
            active: active.map(settings_summary),
            scheduled: next.and_then(|(effective_at, zone, settings)| {
                Some(DailyChainGoalChange {
                    effective_date: local_date(zone, effective_at)?.to_string(),
                    settings: settings.map(settings_summary),
                })
            }),
        }
    }

    pub(super) fn commit_daily_chain_goal(
        &mut self,
        settings: Option<DailyChainGoalSettings>,
        key: Option<&str>,
    ) -> Response {
        let (effective_at, settings) = match self.daily_goal_configuration(settings) {
            Ok(configuration) => configuration,
            Err(error) => return Self::rejected(error),
        };
        let old_sync = self.sync.clone();
        if settings.is_some()
            && !self.sync.legacy_export_completed
            && !self.sync.records.iter().any(|record| {
                matches!(
                    record.payload,
                    RecordPayload::DailyGoalConfigured {
                        settings: Some(_),
                        ..
                    }
                )
            })
            && let Err(error) = self.backfill_sync_records()
        {
            self.sync = old_sync;
            return Self::rejected(error);
        }
        let record = Record::in_beginning(
            self.sync.beginning.clone(),
            RecordId::random(),
            daily_goal_entity(&self.sync.beginning),
            self.next_sync_mutation_time(),
            RecordPayload::DailyGoalConfigured {
                effective_at,
                settings,
            },
        );
        self.sync.records.push(record);
        self.sync.records.sort();
        let through = self.daily_goal_observed_through();
        self.sync.daily_goal_observed_through = Some(through);
        self.sync.daily_goal_next_cutoff =
            DailyGoalSchedule::new(&self.sync.records).next_cutoff(through);
        match self.persist(key) {
            Ok(()) => Response::Snapshot {
                snapshot: self.snapshot(),
            },
            Err(error) => {
                self.sync = old_sync;
                self.durable_rejected(error)
            }
        }
    }

    pub(super) fn settle_daily_goal_if_due(&mut self) -> Result<(), String> {
        if self
            .sync
            .daily_goal_next_cutoff
            .is_none_or(|cutoff| cutoff > self.wall)
        {
            return Ok(());
        }
        let old_sync = self.sync.clone();
        let old_chain = self.chain_backup();
        let old_history = self.history.clone();
        let old_reward_milestones = self.reward_milestones.clone();
        let old_next_reward_milestone_id = self.next_reward_milestone_id;
        let old_next_reward_unlock_id = self.next_reward_unlock_id;
        if let Err(error) = self.reconcile_daily_goal().and_then(|_| self.persist(None)) {
            self.sync = old_sync;
            self.restore_chain_mutation(old_chain);
            self.history = old_history;
            self.reward_milestones = old_reward_milestones;
            self.next_reward_milestone_id = old_next_reward_milestone_id;
            self.next_reward_unlock_id = old_next_reward_unlock_id;
            self.mark_durable_failure(error.clone());
            return Err(error);
        }
        Ok(())
    }

    fn daily_goal_observed_through(&self) -> i64 {
        self.sync
            .daily_goal_observed_through
            .map_or(self.wall, |at| at.max(self.wall))
    }

    pub(super) fn reconcile_daily_goal(&mut self) -> Result<bool, String> {
        if !self.sync.records.iter().any(|record| {
            matches!(
                record.payload,
                RecordPayload::DailyGoalConfigured {
                    settings: Some(_),
                    ..
                }
            )
        }) {
            return Ok(false);
        }
        let through = self.daily_goal_observed_through();
        let schedule = DailyGoalSchedule::new(&self.sync.records);
        let next_cutoff = schedule.next_cutoff(through);
        let credit = DailyGoalCredit::new(&self.sync.records);
        let misses = schedule.misses(&credit, through);
        let projection = project_session_reviews_with_daily_goal(&self.sync.records, &misses);
        for chain in &projection.ended_chains {
            if let Some(entry) = &chain.chain_break
                && let ProjectedChainBreakSource::DailyChainGoal(miss) = &entry.source
                && !self.sync.records.iter().any(|record| {
                    record.entity_id == entry.review_entity_id
                        && matches!(record.payload, RecordPayload::DailyGoalBoundary { .. })
                })
            {
                self.sync.records.push(miss.boundary(&self.sync.beginning));
            }
        }
        self.sync.records.sort();
        self.apply_session_review_projection(&projection)?;
        self.apply_reward_projection(&project_rewards(&self.sync.records));
        self.sync.daily_goal_observed_through = Some(through);
        self.sync.daily_goal_next_cutoff = next_cutoff;
        Ok(true)
    }

    fn daily_goal_configuration(
        &self,
        settings: Option<DailyChainGoalSettings>,
    ) -> Result<(i64, Option<DailyGoalPolicy>), String> {
        let schedule = DailyGoalSchedule::new(&self.sync.records);
        let active = schedule.active_at(self.wall);
        let tomorrow = |zone| {
            local_date(zone, self.wall)
                .and_then(|date| date.succ_opt())
                .ok_or_else(|| "Daily Chain Goal date is outside the supported range".to_owned())
        };
        let effective_at = if let Some(active) = active {
            day_start(active.time_zone, tomorrow(active.time_zone)?)
                .ok_or("Daily Chain Goal deadline is outside the supported range")?
        } else {
            self.wall
        };
        let Some(settings) = settings else {
            return Ok((effective_at, None));
        };
        let time_zone = settings
            .time_zone
            .parse::<Tz>()
            .map_err(|_| "Daily Chain Goal requires an IANA time zone such as Europe/Paris")?;
        let requested_start = settings.start_date.as_deref().map(parse_date).transpose()?;
        let start_date = if let Some(active) = active {
            if time_zone != active.time_zone
                || requested_start.is_some_and(|date| date != active.start_date)
            {
                return Err(
                    "The start date and time zone of an active Daily Chain Goal are fixed".into(),
                );
            }
            active.start_date
        } else {
            let tomorrow = tomorrow(time_zone)?;
            let start = requested_start.unwrap_or(tomorrow);
            if start < tomorrow {
                return Err("Daily Chain Goal start date must be tomorrow or later".into());
            }
            start
        };
        let policy = DailyGoalPolicy {
            quota: settings.quota,
            start_date,
            end_date: settings.end_date.as_deref().map(parse_date).transpose()?,
            time_zone,
        };
        policy.validate()?;
        Ok((effective_at, Some(policy)))
    }
}
