use crate::DomainError;

/// The timing and judgment phase of a commitment to begin a Focus Session.
/// Identity, task attribution, persistence, and clock observations belong to callers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FocusReservation {
    duration_seconds: u64,
    remaining_at_observation: u64,
    observed_at: u64,
    awaiting_review: bool,
}

impl FocusReservation {
    /// Converts a positive whole-minute reservation window into seconds.
    ///
    /// # Errors
    /// Returns an invalid duration error for zero or overflowing minutes.
    pub fn seconds_for_minutes(minutes: u64) -> Result<u64, DomainError> {
        minutes
            .checked_mul(60)
            .filter(|seconds| *seconds > 0)
            .ok_or(DomainError::InvalidDuration)
    }

    /// Issues a reservation with its original window anchored to the observation.
    ///
    /// # Errors
    /// Returns an invalid duration error for zero or overflowing minutes.
    pub fn new(minutes: u64, now: u64) -> Result<Self, DomainError> {
        let duration_seconds = Self::seconds_for_minutes(minutes)?;
        Self::restore(duration_seconds, duration_seconds, now, false)
    }

    /// Restores a validated timing state. A countdown may have reached zero before
    /// its next observation; advancing it still produces the expiry transition.
    ///
    /// # Errors
    /// Rejects invalid whole-minute windows, excess remaining time, or a waiting
    /// judgment with a nonzero countdown.
    pub fn restore(
        duration_seconds: u64,
        remaining_seconds: u64,
        now: u64,
        awaiting_review: bool,
    ) -> Result<Self, DomainError> {
        if duration_seconds == 0
            || !duration_seconds.is_multiple_of(60)
            || remaining_seconds > duration_seconds
        {
            return Err(DomainError::InvalidDuration);
        }
        if awaiting_review && remaining_seconds != 0 {
            return Err(DomainError::InvalidTransition);
        }
        Ok(Self {
            duration_seconds,
            remaining_at_observation: remaining_seconds,
            observed_at: now,
            awaiting_review,
        })
    }

    #[must_use]
    pub const fn duration_seconds(self) -> u64 {
        self.duration_seconds
    }

    #[must_use]
    pub const fn is_awaiting_review(self) -> bool {
        self.awaiting_review
    }

    #[must_use]
    pub fn remaining_seconds(self, now: u64) -> u64 {
        self.remaining_at_observation
            .saturating_sub(now.saturating_sub(self.observed_at))
    }

    /// Applies expiry once, without deciding success or failure.
    /// Returns true only for the transition to awaiting judgment.
    pub fn advance(&mut self, now: u64) -> bool {
        if self.awaiting_review || self.remaining_seconds(now) > 0 {
            return false;
        }
        self.awaiting_review = true;
        self.remaining_at_observation = 0;
        self.observed_at = now;
        true
    }
}
