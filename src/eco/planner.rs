//! Turns (task, bag, world model, economy) into one action per cycle.
//! Recomputed every cycle (receding horizon) so it always reacts to the
//! latest prices and the latest bit of the world graph learned, rather
//! than committing to a long plan that a regime flip could invalidate.

use std::collections::HashMap;

use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::utils::ID;


use super::recipes::{is_step_ready, remaining_work};use super::regime::{CostTable, EconomyRegime, ForecastMode};
use super::time::CYCLES_PER_DAY;
use super::world::WorldModel;

#[derive(Debug, Clone)]
pub enum Action {
    Move(ID),
    Stay,
    Mine(BasicResourceType),
    Combine(ComplexResourceType),
    ExploreTowards(ID),
    RequestNeighbors,
}

pub struct Planner;

impl Planner {
    #[allow(clippy::too_many_arguments)]
    /// True if the *known* galaxy can still deliver `target`: every basic still
    /// to be mined has a known supplier and every combine still to be run has
    /// a known planet that can do it. Stock already in the bag counts.
    pub fn is_feasible(
        target: ComplexResourceType,
        world: &WorldModel,
        bag_basics: &HashMap<BasicResourceType, u32>,
        bag_complex: &HashMap<ComplexResourceType, u32>,
    ) -> bool {
        let remaining = remaining_work(target, bag_basics, bag_complex);
        let basics_ok = remaining
            .basics_to_mine
            .iter()
            .all(|b| world.resources.values().any(|s| s.contains(b)));
        let steps_ok = remaining
            .steps_to_run
            .iter()
            .all(|c| world.combos.values().any(|s| s.contains(c)));
        basics_ok && steps_ok
    }
    pub fn next_action(
        target: ComplexResourceType,
        current_planet: ID,
        world: &WorldModel,
        bag_basics: &HashMap<BasicResourceType, u32>,
        bag_complex: &HashMap<ComplexResourceType, u32>,
        regime: EconomyRegime,
        costs: CostTable,
        forecast: &ForecastMode,
    ) -> Action {
        let remaining = remaining_work(target, bag_basics, bag_complex);

        // 1. A combine whose ingredients are all in the bag.
        if let Some(&step) = remaining
            .steps_to_run
            .iter()
            .find(|&&s| is_step_ready(s, bag_basics, bag_complex))
        {
            if world.combos.get(&current_planet).is_some_and(|s| s.contains(&step)) {
                return Self::maybe_wait(Action::Combine(step), costs.combine, regime, costs.stay, forecast);
            }
            return match world.nearest_known_combo_planet(current_planet, step) {
                Some(path) => Self::step_along(path, regime, costs, forecast),
                None => Self::explore_or_request(current_planet, world),
            };
        }

        // 2. Otherwise we need basics.
        if !remaining.basics_to_mine.is_empty() {
            // 2a. This planet supplies one of them: mine it here.
            let here = world.resources.get(&current_planet);
            if let Some(&missing) = remaining
                .basics_to_mine
                .iter()
                .find(|b| here.is_some_and(|s| s.contains(b)))
            {
                return Self::maybe_wait(Action::Mine(missing), costs.mine, regime, costs.stay, forecast);
            }
            // 2b. Otherwise walk to the nearest known planet that supplies any of them.
            let nearest = remaining
                .basics_to_mine
                .iter()
                .filter_map(|&b| world.nearest_known_source(current_planet, b))
                .min_by_key(|path| path.len());
            return match nearest {
                Some(path) => Self::step_along(path, regime, costs, forecast),
                None => Self::explore_or_request(current_planet, world),
            };
        }

        Action::Stay
    }


    /// First hop of a known BFS path, subject to the wait-vs-act check.
    /// `Stay` if we're already at the destination (path of length 1).
    fn step_along(path: Vec<ID>, regime: EconomyRegime, costs: CostTable, forecast: &ForecastMode) -> Action {
        if path.len() <= 1 {
            return Action::Stay;
        }
        Self::maybe_wait(Action::Move(path[1]), costs.mv, regime, costs.stay, forecast)
    }

    fn explore_or_request(current_planet: ID, world: &WorldModel) -> Action {
        match world.neighbors.get(&current_planet) {
            None => Action::RequestNeighbors,
            Some(_) => match world.nearest_unexplored(current_planet){
                Some(path) if path.len() > 1 => Action::ExploreTowards(path[1]),
                _ => Action::RequestNeighbors, // known frontier exhausted — ask again
            },
        }
    }

    /// Wait-vs-act: only ever worth delaying out of Recession (Neutral
    /// and Flourish prices don't justify stalling — Flourish is already
    /// the cheap regime, and Neutral is a fixed 5-day block so "waiting
    /// it out" isn't a forecasting problem). If the estimated remaining
    /// Recession length is short, compare paying `stay` each remaining
    /// cycle plus the Neutral-regime price, against paying today's
    /// inflated price right now.
    ///
    /// This is a one-step-lookahead heuristic, not an exact DP over the
    /// full remaining plan — it deliberately stays cheap since it reruns
    /// every cycle (receding horizon), so a wrong call self-corrects
    /// quickly rather than needing to be optimal up front.
    fn maybe_wait(
        action: Action,
        action_cost_now: u32,
        regime: EconomyRegime,
        stay_cost_now: u32,
        forecast: &ForecastMode,
    ) -> Action {
        if regime != EconomyRegime::Recession {
            return action;
        }

        let action_cost_after_wait = match &action {
            Action::Move(_) | Action::ExploreTowards(_) => EconomyRegime::Neutral.costs().mv,
            Action::Mine(_) => EconomyRegime::Neutral.costs().mine,
            Action::Combine(_) => EconomyRegime::Neutral.costs().combine,
            Action::Stay | Action::RequestNeighbors => return action, // nothing to compare
        };

        let days_left = forecast.days_left_estimate(regime);
        if days_left <= 0.05 {
            return action; // regime believed to be ending "now" — no time saved
        }

        let cycles_left = (days_left * CYCLES_PER_DAY as f64).max(1.0);
        let cost_now = action_cost_now as f64;
        let cost_if_waited = cycles_left * stay_cost_now as f64 + action_cost_after_wait as f64;

        if cost_if_waited < cost_now {
            Action::Stay
        } else {
            action
        }
    }
}