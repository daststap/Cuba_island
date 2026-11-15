//! A single countdown timer (pure state machine; the scheduler ticks it).

use serde::Serialize;

/// Mirrored in `src/ipc/commands.ts`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerState {
    /// Total length in ms; 0 = no timer.
    pub duration_ms: u64,
    /// Unix ms when it ends; `None` while paused or idle.
    pub ends_at: Option<i64>,
    /// Remaining ms while paused.
    pub paused_remaining_ms: Option<u64>,
}

/// Longest timer: 24 h.
pub const MAX_DURATION_MS: u64 = 24 * 60 * 60 * 1000;

impl TimerState {
    pub fn start(duration_ms: u64, now: i64) -> Self {
        let duration_ms = duration_ms.min(MAX_DURATION_MS);
        Self {
            duration_ms,
            ends_at: Some(now + i64::try_from(duration_ms).unwrap_or(i64::MAX)),
            paused_remaining_ms: None,
        }
    }

    pub fn is_running(&self) -> bool {
        self.ends_at.is_some()
    }

    pub fn remaining_ms(&self, now: i64) -> u64 {
        match (self.ends_at, self.paused_remaining_ms) {
            (Some(end), _) => u64::try_from(end - now).unwrap_or(0),
            (None, Some(left)) => left,
            (None, None) => 0,
        }
    }

    pub fn pause(self, now: i64) -> Self {
        if !self.is_running() {
            return self;
        }
        Self {
            ends_at: None,
            paused_remaining_ms: Some(self.remaining_ms(now)),
            ..self
        }
    }

    pub fn resume(self, now: i64) -> Self {
        match self.paused_remaining_ms {
            Some(left) => Self {
                ends_at: Some(now + i64::try_from(left).unwrap_or(i64::MAX)),
                paused_remaining_ms: None,
                ..self
            },
            None => self,
        }
    }

    /// If the timer just ran out, returns the idle state to switch to.
    pub fn finished(&self, now: i64) -> Option<Self> {
        (self.is_running() && self.remaining_ms(now) == 0).then(Self::default)
    }
}

/// How long the "time's up" sound keeps repeating if nobody reacts.
pub const RING_FOR_MS: i64 = 60_000;
/// Pause between repeats.
pub const RING_EVERY_MS: i64 = 2_000;

/// Repeats the "time's up" sound, like a phone alarm, until silenced or for a minute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ringer {
    until: i64,
    next: i64,
}

impl Ringer {
    pub fn start(now: i64) -> Self {
        Self {
            until: now + RING_FOR_MS,
            next: now,
        }
    }

    /// Whether to play the sound at `now`; `None` once the ringing is over.
    pub fn poll(&mut self, now: i64) -> Option<bool> {
        if now >= self.until {
            return None;
        }
        if now >= self.next {
            self.next = now + RING_EVERY_MS;
            return Some(true);
        }
        Some(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_down_pauses_and_resumes() {
        let t = TimerState::start(60_000, 1_000);
        assert_eq!(t.remaining_ms(31_000), 30_000);
        let paused = t.pause(31_000);
        assert!(!paused.is_running());
        assert_eq!(paused.remaining_ms(999_999), 30_000);
        let resumed = paused.resume(100_000);
        assert_eq!(resumed.remaining_ms(110_000), 20_000);
        assert_eq!(resumed.duration_ms, 60_000);
    }

    #[test]
    fn finishes_once_and_only_while_running() {
        let t = TimerState::start(1_000, 0);
        assert_eq!(t.finished(500), None);
        assert_eq!(t.finished(1_000), Some(TimerState::default()));
        assert_eq!(t.pause(500).finished(5_000), None);
        assert_eq!(TimerState::default().finished(5_000), None);
    }

    #[test]
    fn caps_the_duration() {
        assert_eq!(TimerState::start(u64::MAX, 0).duration_ms, MAX_DURATION_MS);
    }

    #[test]
    fn rings_every_two_seconds_for_a_minute() {
        let mut ringer = Ringer::start(0);
        assert_eq!(ringer.poll(0), Some(true));
        assert_eq!(ringer.poll(1_000), Some(false));
        assert_eq!(ringer.poll(2_000), Some(true));
        assert_eq!(ringer.poll(3_500), Some(false));
        assert_eq!(ringer.poll(4_100), Some(true));
        assert_eq!(ringer.poll(RING_FOR_MS), None);
    }
}
