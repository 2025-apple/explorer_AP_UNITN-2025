/// Price table for this regime. These numbers are a design choice of Eco's, not given by the course spec.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EconomyRegime {
    Neutral,
    Flourish,
    Recession,
}

#[derive(Debug, Clone, Copy)]
pub struct CostTable {
    pub mv: u32,
    pub stay: u32,
    pub mine: u32,
    pub combine: u32,
    pub daily_income: u32,
}

impl EconomyRegime {
    /// Hardcoded price tables from spec. Eco knows these outright — they
    /// are constants of the world, not something announced by anyone.
    pub fn costs(self) -> CostTable {
        match self {
            EconomyRegime::Neutral => CostTable {
                mv: 4,
                stay: 4,
                mine: 4,
                combine: 4,
                daily_income: 12,
            },
            EconomyRegime::Flourish => CostTable {
                mv: 5,
                stay: 2,
                mine: 1,
                combine: 3,
                daily_income: 20,
            },
            EconomyRegime::Recession => CostTable {
                mv: 9,
                stay: 7,
                mine: 5,
                combine: 6,
                daily_income: 16,
            },
        }
    }

    pub(super) fn next(self) -> EconomyRegime {
        match self {
            EconomyRegime::Neutral => EconomyRegime::Flourish,
            EconomyRegime::Flourish => EconomyRegime::Recession,
            EconomyRegime::Recession => EconomyRegime::Neutral,
        }
    }
}

/// Tracks observed Flourish/Recession durations so "blind mode" planning
/// can estimate how much longer the current regime has left, without
/// ever reading ground truth. Neutral is skipped since its length is
/// fixed and known outright.
#[derive(Debug, Clone)]
pub struct RegimeEstimator {
    ema_flourish_days: f64,
    ema_recession_days: f64,
    alpha: f64,
}

impl RegimeEstimator {
    pub fn new() -> Self {
        Self {
            // Seeded at the midpoint of the draw ranges as a reasonable
            // prior before any regime has actually been observed to end.
            ema_flourish_days: 5.5,
            ema_recession_days: 4.0,
            alpha: 0.3,
        }
    }

    pub fn observe(&mut self, regime: EconomyRegime, days_lasted: u32) {
        let x = f64::from(days_lasted);
        match regime {
            EconomyRegime::Flourish => {
                self.ema_flourish_days =
                    self.alpha * x + (1.0 - self.alpha) * self.ema_flourish_days;
            }
            EconomyRegime::Recession => {
                self.ema_recession_days =
                    self.alpha * x + (1.0 - self.alpha) * self.ema_recession_days;
            }
            EconomyRegime::Neutral => {}
        }
    }

    /// Estimated days remaining in `regime`, given `days_spent_in_regime`
    /// already elapsed. Rounds down / floors at zero — overestimating
    /// "relief is coming soon" is the costlier mistake, since it
    /// encourages waiting through a regime that doesn't actually end.
    pub fn estimate_days_left(&self, regime: EconomyRegime, days_spent_in_regime: u32) -> f64 {
        let expected_total = match regime {
            EconomyRegime::Flourish => self.ema_flourish_days,
            EconomyRegime::Recession => self.ema_recession_days,
            EconomyRegime::Neutral => 5.0,
        };
        (expected_total - f64::from(days_spent_in_regime)).max(0.0)
    }
}

impl Default for RegimeEstimator {
    fn default() -> Self {
        Self::new()
    }
}

/// What the planner is allowed to see about regime timing.
/// Blind = the real AI. Oracle = test/baseline harness only.
pub enum ForecastMode<'a> {
    Blind {
        estimator: &'a RegimeEstimator,
        days_spent_in_regime: u32,
    },
    Oracle {
        days_left_in_regime: u32,
    },
}

impl ForecastMode<'_> {
    pub(super) fn days_left_estimate(&self, regime: EconomyRegime) -> f64 {
        match self {
            ForecastMode::Blind {
                estimator,
                days_spent_in_regime,
            } => estimator.estimate_days_left(regime, *days_spent_in_regime),
            ForecastMode::Oracle {
                days_left_in_regime,
            } => f64::from(*days_left_in_regime),
        }
    }
}
