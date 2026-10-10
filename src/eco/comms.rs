//! Eco's bookkeeping of "what am I waiting for from the orchestrator?".
//! Pure state + time arithmetic: it never sends anything and never touches
//! the economy (clock, wallet, prices). Time is passed in as `now` so it
//! can be tested without sleeping.

use std::time::{Duration, Instant};

use common_game::utils::ID;

/// How long Eco waits for the orchestrator to answer one request before
/// giving up on it.
pub(super) const PENDING_TIMEOUT: Duration = Duration::from_secs(10);

/// Quiet period after a failed attempt (timeout, failed mine/combine)
/// before Eco may try something again.
pub(super) const RETRY_COOLDOWN: Duration = Duration::from_secs(1);

/// The one request Eco is waiting on. At most one at a time.
// `for_planet` / `to` are read from Step 5 on; remove this allow then.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pending {
    Idle,
    /// Sent `NeighborsRequest` while on `for_planet`.
    Neighbors { for_planet: ID, since: Instant },
    /// Sent `TravelToPlanetRequest` towards `to`.
    Travel { to: ID, since: Instant },
}

#[derive(Debug)]
pub(super) struct Comms {
    pub(super) pending: Pending,
    /// No new action before this instant (after a failure).
    pub(super) retry_at: Option<Instant>,
}

impl Comms {
    pub(super) fn new() -> Self {
        Comms { pending: Pending::Idle, retry_at: None }
    }

    /// Called once per decision step. First expires an overdue pending
    /// request (and starts a cooldown), then says whether Eco may choose a
    /// new action right now.
    pub(super) fn may_act(&mut self, now: Instant) -> bool {
        let since = match self.pending {
            Pending::Idle => None,
            Pending::Neighbors { since, .. } | Pending::Travel { since, .. } => Some(since),
        };
        if let Some(since) = since {
            if now.saturating_duration_since(since) >= PENDING_TIMEOUT {
                self.pending = Pending::Idle;
                self.retry_at = Some(now + RETRY_COOLDOWN);
            }
        }

        if self.pending != Pending::Idle {
            return false;
        }
        match self.retry_at {
            Some(until) if now < until => false,
            _ => {
                self.retry_at = None;
                true
            }
        }
    }

    pub(super) fn start_neighbors(&mut self, for_planet: ID, now: Instant) {
        self.pending = Pending::Neighbors { for_planet, since: now };
    }

    pub(super) fn start_travel(&mut self, to: ID, now: Instant) {
        self.pending = Pending::Travel { to, since: now };
    }

    /// A `NeighborsResponse` arrived: clears only a pending neighbors request.
    pub(super) fn resolve_neighbors(&mut self) {
        if matches!(self.pending, Pending::Neighbors { .. }) {
            self.pending = Pending::Idle;
        }
    }

    /// A `MoveToPlanet` arrived: clears only a pending travel request.
    pub(super) fn resolve_travel(&mut self) {
        if matches!(self.pending, Pending::Travel { .. }) {
            self.pending = Pending::Idle;
        }
    }

    /// A planet action failed: wait a moment before trying again.
    pub(super) fn cool_down(&mut self, now: Instant) {
        self.retry_at = Some(now + RETRY_COOLDOWN);
    }
}