mod common;

use std::time::Duration;

use common::{spawn_explorer_with_task, Galaxy, TestPlanet};
use common_game::components::planet::PlanetType;
use common_game::components::resource::{BasicResourceType, ComplexResourceType, ResourceType};
use common_game::protocols::orchestrator_explorer::OrchestratorToExplorer;

/// 1 -- 2 -- 3, all real planets, all started and charged.
///   planet 1 (type A) generates Hydrogen
///   planet 2 (type A) generates Oxygen
///   planet 3 (type B) generates Carbon and can combine Water
/// Eco starts on planet 1 with the task "Water" (Hydrogen + Oxygen).
fn three_planet_line(outgoing_first: bool) -> Galaxy {
    let mut galaxy = Galaxy::new(outgoing_first);
    galaxy.add_planet(TestPlanet::spawn(1, PlanetType::A, vec![BasicResourceType::Hydrogen], vec![]));
    galaxy.add_planet(TestPlanet::spawn(2, PlanetType::A, vec![BasicResourceType::Oxygen], vec![]));
    galaxy.add_planet(TestPlanet::spawn(
        3,
        PlanetType::B,
        vec![BasicResourceType::Carbon],
        vec![ComplexResourceType::Diamond],
    ));
    galaxy.connect(1, 2);
    galaxy.connect(2, 3);
    galaxy.start_all();
    for _ in 0..5 {
        galaxy.planet(1).sunray();
        galaxy.planet(2).sunray();
    }
    galaxy.planet(3).sunray();
    galaxy
}

fn run_scenario(outgoing_first: bool) {
    let _ = env_logger::builder().is_test(true).try_init();

    let galaxy = three_planet_line(outgoing_first);
    let eco = spawn_explorer_with_task(1, galaxy.planet(1), ComplexResourceType::Diamond);
    eco.send(OrchestratorToExplorer::StartExplorerAI);

    let water = ResourceType::Complex(ComplexResourceType::Diamond);
    assert!(
        galaxy.serve_until_bag_contains(&eco, water, Duration::from_secs(40)),
        "Eco did not build Water within 40 s"
    );
    // `eco` is dropped before `galaxy`, so Eco stops before its planets do.
}

#[test]
fn eco_builds_water_across_three_real_planets() {
    run_scenario(false);
}

#[test]
fn the_result_does_not_depend_on_incoming_outgoing_order() {
    run_scenario(true);
}