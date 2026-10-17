use std::collections::{HashMap, HashSet};

use common_game::components::resource::{BasicResourceType as B, ComplexResourceType as C};

use crate::eco::planner::{Action, Planner};
use crate::eco::regime::{EconomyRegime, ForecastMode};
use crate::eco::world::WorldModel;

fn act(
    target: C,
    here: u32,
    world: &WorldModel,
    basics: HashMap<B, u32>,
    complex: HashMap<C, u32>,
) -> Action {
    let regime = EconomyRegime::Neutral;
    Planner::next_action(
        target,
        here,
        world,
        &basics,
        &complex,
        regime,
        regime.costs(),
        &ForecastMode::Oracle {
            days_left_in_regime: 0,
        },
    )
}

#[test]
fn mines_a_missing_resource_the_planet_supplies() {
    let mut w = WorldModel::default();
    w.record_resources(1, HashSet::from([B::Carbon]));
    let a = act(C::Diamond, 1, &w, HashMap::new(), HashMap::new());
    assert!(matches!(a, Action::Mine(B::Carbon)));
}

#[test]
fn walks_to_a_supplier_instead_of_mining_here() {
    let mut w = WorldModel::default();
    w.record_neighbors(1, vec![2]);
    w.record_neighbors(2, vec![1]);
    w.record_resources(1, HashSet::from([B::Hydrogen])); // useless for Diamond
    w.record_resources(2, HashSet::from([B::Carbon]));
    let a = act(C::Diamond, 1, &w, HashMap::new(), HashMap::new());
    assert!(matches!(a, Action::Move(2)));
}

#[test]
fn never_mines_what_the_planet_does_not_supply() {
    let mut w = WorldModel::default();
    w.record_resources(1, HashSet::from([B::Hydrogen]));
    // no neighbours known and no supplier known: must explore, not mine
    let a = act(C::Diamond, 1, &w, HashMap::new(), HashMap::new());
    assert!(matches!(a, Action::RequestNeighbors));
}

#[test]
fn combines_when_ready_and_supported_here() {
    let mut w = WorldModel::default();
    w.record_combos(1, HashSet::from([C::Diamond]));
    let a = act(
        C::Diamond,
        1,
        &w,
        HashMap::from([(B::Carbon, 2)]),
        HashMap::new(),
    );
    assert!(matches!(a, Action::Combine(C::Diamond)));
}

#[test]
fn one_carbon_is_not_enough_for_diamond() {
    let mut w = WorldModel::default();
    w.record_resources(1, HashSet::from([B::Carbon]));
    w.record_combos(1, HashSet::from([C::Diamond]));
    let a = act(
        C::Diamond,
        1,
        &w,
        HashMap::from([(B::Carbon, 1)]),
        HashMap::new(),
    );
    assert!(matches!(a, Action::Mine(B::Carbon)));
}

#[test]
fn life_with_water_in_bag_asks_only_for_carbon() {
    let mut w = WorldModel::default();
    w.record_resources(1, HashSet::from([B::Hydrogen, B::Oxygen, B::Carbon]));
    let a = act(
        C::Life,
        1,
        &w,
        HashMap::new(),
        HashMap::from([(C::Water, 1)]),
    );
    assert!(matches!(a, Action::Mine(B::Carbon)));
}

#[test]
fn feasible_when_every_ingredient_has_a_known_planet() {
    let mut w = WorldModel::default();
    w.record_resources(1, HashSet::from([B::Carbon]));
    w.record_combos(1, HashSet::from([C::Diamond]));
    assert!(Planner::is_feasible(
        C::Diamond,
        &w,
        &HashMap::new(),
        &HashMap::new()
    ));
}

#[test]
fn infeasible_without_a_known_combiner() {
    let mut w = WorldModel::default();
    w.record_resources(1, HashSet::from([B::Carbon]));
    assert!(!Planner::is_feasible(
        C::Diamond,
        &w,
        &HashMap::new(),
        &HashMap::new()
    ));
}

#[test]
fn infeasible_when_no_known_planet_supplies_silicon() {
    // the real failure: Robot needs Silicon, and planet 4 (its only supplier) is gone
    let mut w = WorldModel::default();
    w.record_resources(2, HashSet::from([B::Carbon, B::Hydrogen, B::Oxygen]));
    w.record_combos(7, HashSet::from([C::Water, C::Life, C::Robot]));
    assert!(!Planner::is_feasible(
        C::Robot,
        &w,
        &HashMap::new(),
        &HashMap::new()
    ));
}

#[test]
fn held_stock_makes_a_task_feasible_again() {
    // AIPartner = Robot + Diamond; holding both Robot and Diamond, only the
    // final combine is left, so no supplier is needed at all
    let mut w = WorldModel::default();
    w.record_combos(7, HashSet::from([C::AIPartner]));
    let held = HashMap::from([(C::Robot, 1), (C::Diamond, 1)]);
    assert!(Planner::is_feasible(
        C::AIPartner,
        &w,
        &HashMap::new(),
        &held
    ));
}
