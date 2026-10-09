mod common;

use std::collections::HashSet;
use std::time::Duration;

use common::{spawn_explorer, TestPlanet};
use common_game::components::planet::PlanetType;
use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::protocols::orchestrator_explorer::{ExplorerToOrchestrator, OrchestratorToExplorer};

const WAIT: Duration = Duration::from_secs(4);

#[test]
fn real_planet_reports_its_generation_rules() {
    let planet = TestPlanet::spawn(
        100,
        PlanetType::B,
        vec![BasicResourceType::Oxygen, BasicResourceType::Hydrogen],
        vec![ComplexResourceType::Water],
    );
    planet.start();
    let eco = spawn_explorer(1, &planet);

    eco.send(OrchestratorToExplorer::SupportedResourceRequest);
    let got = eco
        .wait_for(WAIT, |m| match m {
            ExplorerToOrchestrator::SupportedResourceResult { supported_resources, .. } => {
                Some(supported_resources)
            }
            _ => None,
        })
        .expect("no SupportedResourceResult");

    assert_eq!(
        got,
        HashSet::from([BasicResourceType::Oxygen, BasicResourceType::Hydrogen])
    );
}

#[test]
fn real_planet_with_no_combination_rules_answers_an_empty_set() {
    // Planet type A cannot have combination rules: an empty set is a legitimate answer.
    let planet = TestPlanet::spawn(101, PlanetType::A, vec![BasicResourceType::Carbon], vec![]);
    planet.start();
    let eco = spawn_explorer(1, &planet);

    eco.send(OrchestratorToExplorer::SupportedCombinationRequest);
    let got = eco
        .wait_for(WAIT, |m| match m {
            ExplorerToOrchestrator::SupportedCombinationResult { combination_list, .. } => {
                Some(combination_list)
            }
            _ => None,
        })
        .expect("no SupportedCombinationResult");

    assert!(got.is_empty());
}