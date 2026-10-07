//! Eco's day/cycle clock — *how time advances* and when regimes flip.
//!
//! This module only tracks timing: which cycle/day it is, how long is
//! left in the current regime, and drawing a length for the next
//! Flourish/Recession when one starts. Prices and the regime-switching
//! *order* live in `regime.rs`.

use rand::{Rng, RngExt};

use super::regime::{CostTable, EconomyRegime};

/// Cycles-per-day, fixed by spec.
pub const CYCLES_PER_DAY: u8 = 5;
/// Flourish/Recession durations are "unknown" per spec; these are the
/// ranges Eco's own RNG draws from at transition time. Tune freely — the
/// spec only constrains Neutral's length (always 5 days).
const FLOURISH_DAYS_RANGE: (u32, u32) = (3, 8);
const RECESSION_DAYS_RANGE: (u32, u32) = (2, 6);

#[derive(Debug, Clone)]
pub struct EconomyClock {
    pub day: u32,
    pub cycle_in_day: u8, // 0..CYCLES_PER_DAY
    pub regime: EconomyRegime,
    pub days_left_in_regime: u32,
    regime_started_len: u32,
    income_due: bool,
}

impl EconomyClock {
    pub fn new() -> Self {
        Self {
            day: 1,
            cycle_in_day: 0,
            regime: EconomyRegime::Neutral,
            days_left_in_regime: 5,
            regime_started_len: 5,
            income_due: false,
        }
    }

    pub fn costs(&self) -> CostTable {
        self.regime.costs()
    }

    pub fn days_spent_in_regime(&self) -> u32 {
        self.regime_started_len.saturating_sub(self.days_left_in_regime)
    }

    /// Advances one clock cycle. Call exactly once per `ai_step`.
    ///
    /// Returns `Some((regime_that_just_ended, its_length_in_days))` on the
    /// cycle where a regime transition happens, so the caller can feed
    /// `RegimeEstimator::observe`. Returns `None` on every other cycle.
    pub fn tick(&mut self, rng: &mut impl Rng) -> Option<(EconomyRegime, u32)> {
        self.cycle_in_day += 1;
        let mut ended = None;

        if self.cycle_in_day >= CYCLES_PER_DAY {
            self.cycle_in_day = 0;
            self.day += 1;
            self.income_due = true;

            if self.days_left_in_regime > 0 {
                self.days_left_in_regime -= 1;
            }
            if self.days_left_in_regime == 0 {
                ended = Some((self.regime, self.regime_started_len));
                self.advance_regime(rng);
            }
        }

        ended
    }

    fn advance_regime(&mut self, rng: &mut impl Rng) {
        self.regime = self.regime.next();
        self.days_left_in_regime = match self.regime {
            EconomyRegime::Neutral => 5,
            EconomyRegime::Flourish => {
                rng.random_range(FLOURISH_DAYS_RANGE.0..=FLOURISH_DAYS_RANGE.1)
            }
            EconomyRegime::Recession => {
                rng.random_range(RECESSION_DAYS_RANGE.0..=RECESSION_DAYS_RANGE.1)
            }
        };
        self.regime_started_len = self.days_left_in_regime;
    }

    /// Consumes the "income due" flag — fires true at most once per day
    /// rollover, so the caller credits the wallet exactly once per day.
    pub fn take_income_due(&mut self) -> bool {
        std::mem::replace(&mut self.income_due, false)
    }
}

impl Default for EconomyClock {
    fn default() -> Self {
        Self::new()
    }
}