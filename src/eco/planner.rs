//! Turns (task, bag, world model, economy) into one action per cycle.
//! Recomputed every cycle (receding horizon) so it always reacts to the
//! latest prices and the latest bit of the world graph learned, rather
//! than committing to a long plan that a regime flip could invalidate.

use std::collections::{HashMap, HashSet};

use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::utils::ID;

use super::recipes::{build_plan_for, recipe_of, BuildPlan, Ingredient};
use super::regime::{CostTable, EconomyRegime, ForecastMode};
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
    pub fn next_action(
        target: ComplexResourceType,
        current_planet: ID,
        world: &WorldModel,
        bag_basics: &HashMap<BasicResourceType, u32>,
        bag_complex: &HashSet<ComplexResourceType>,
        regime: EconomyRegime,
        costs: CostTable,
        forecast: &ForecastMode,
    ) -> Action {
        let plan = build_plan_for(target);

        // 1. Prefer combining once a step's ingredients are satisfied —
        //    it clears bag slots and moves the plan toward completion.
        if let Some(step) = Self::next_ready_combine(&plan, bag_basics, bag_complex) {
            if world.combos.get(&current_planet).is_some_and(|s| s.contains(&step)) {
                return Self::maybe_wait(Action::Combine(step), costs.combine, regime, costs.stay, forecast);
            }
            return match world.nearest_known_combo_planet(current_planet, step) {
                Some(path) => Self::step_along(path, regime, costs, forecast),
                None => Self::explore_or_request(current_planet, world),
            };
        }

        // 2. Otherwise, go get the next missing basic resource.
        if let Some(missing) = Self::next_missing_basic(&plan, bag_basics) {
            if world.resources.get(&current_planet).is_some_and(|s| s.contains(&missing)) {
                return Self::maybe_wait(Action::Mine(missing), costs.mine, regime, costs.stay, forecast);
            }
            return match world.nearest_known_source(current_planet, missing) {
                Some(path) => Self::step_along(path, regime, costs, forecast),
                None => Self::explore_or_request(current_planet, world),
            };
        }

        // Nothing outstanding — caller checks the bag for the finished
        // target on the next tick. Idle safely in the meantime.
        Action::Stay
    }

    fn next_missing_basic(
        plan: &BuildPlan,
        bag_basics: &HashMap<BasicResourceType, u32>,
    ) -> Option<BasicResourceType> {
        let mut needed: HashMap<BasicResourceType, u32> = HashMap::new();
        for &b in &plan.basics_needed {
            *needed.entry(b).or_insert(0) += 1;
        }
        needed
            .into_iter()
            .find(|(bt, need_count)| bag_basics.get(bt).copied().unwrap_or(0) < *need_count)
            .map(|(bt, _)| bt)
    }

    fn next_ready_combine(
        plan: &BuildPlan,
        bag_basics: &HashMap<BasicResourceType, u32>,
        bag_complex: &HashSet<ComplexResourceType>,
    ) -> Option<ComplexResourceType> {
        for &step in &plan.combine_steps {
            if bag_complex.contains(&step) {
                continue; // already built
            }
            let (a, b) = recipe_of(step);
            let ready = [a, b].iter().all(|ing| match ing {
                Ingredient::Basic(bt) => bag_basics.get(bt).copied().unwrap_or(0) > 0,
                Ingredient::Complex(ct) => bag_complex.contains(ct),
            });
            if ready {
                return Some(step);
            }
        }
        None
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
            Some(_) => match world.nearest_unvisited(current_planet) {
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