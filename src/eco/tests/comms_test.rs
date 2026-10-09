use std::collections::HashSet;
use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};

use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::protocols::orchestrator_explorer::{ExplorerToOrchestrator, OrchestratorToExplorer};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};

use crate::eco::explorer::{BagContent, Explorer, PlanetError};

/// The planet's end of the channels, driven by hand in each test.
struct PlanetSide {
    requests: Receiver<ExplorerToPlanet>,
    replies: Sender<PlanetToExplorer>,
}

fn rig() -> (Explorer, PlanetSide) {
    let (_to_explorer, rx_orch) = unbounded::<OrchestratorToExplorer>();
    let (tx_orch, _from_explorer) = unbounded::<ExplorerToOrchestrator<BagContent>>();
    let (tx_planet, requests) = unbounded::<ExplorerToPlanet>();
    let (replies, rx_planet) = unbounded::<PlanetToExplorer>();
    let eco = Explorer::new(1, 100, rx_orch, tx_orch, rx_planet, tx_planet, true);
    (eco, PlanetSide { requests, replies })
}

#[test]
fn stopped_planet_is_an_error_and_nothing_is_recorded() {
    let (mut eco, planet) = rig();
    let t = thread::spawn(move || {
        planet.requests.recv().unwrap();
        planet.replies.send(PlanetToExplorer::Stopped).unwrap();
    });
    assert_eq!(eco.query_supported_resources(), Err(PlanetError::Stopped));
    t.join().unwrap();
    assert!(!eco.world.resources.contains_key(&100));
}

#[test]
fn silent_planet_times_out_and_nothing_is_recorded() {
    let (mut eco, _planet) = rig(); // kept alive, but never answers (takes ~2 s)
    assert_eq!(eco.query_supported_combinations(), Err(PlanetError::NoReply));
    assert!(!eco.world.combos.contains_key(&100));
}

#[test]
fn closed_channel_is_reported_as_gone() {
    let (mut eco, planet) = rig();
    drop(planet);
    assert_eq!(eco.query_supported_resources(), Err(PlanetError::Gone));
    assert!(!eco.world.resources.contains_key(&100));
}

#[test]
fn a_valid_empty_answer_is_recorded() {
    let (mut eco, planet) = rig();
    let t = thread::spawn(move || {
        planet.requests.recv().unwrap();
        planet
            .replies
            .send(PlanetToExplorer::SupportedCombinationResponse { combination_list: HashSet::new() })
            .unwrap();
    });
    assert_eq!(eco.query_supported_combinations(), Ok(HashSet::<ComplexResourceType>::new()));
    t.join().unwrap();
    assert_eq!(eco.world.combos.get(&100), Some(&HashSet::new()));
}

#[test]
fn stale_reply_waiting_in_the_channel_is_not_taken_for_the_answer() {
    let (mut eco, planet) = rig();
    // A leftover reply of the SAME kind from some earlier request:
    planet
        .replies
        .send(PlanetToExplorer::SupportedResourceResponse {
            resource_list: HashSet::from([BasicResourceType::Oxygen]),
        })
        .unwrap();
    let t = thread::spawn(move || {
        planet.requests.recv().unwrap();
        planet
            .replies
            .send(PlanetToExplorer::SupportedResourceResponse {
                resource_list: HashSet::from([BasicResourceType::Carbon]),
            })
            .unwrap();
    });
    assert_eq!(
        eco.query_supported_resources(),
        Ok(HashSet::from([BasicResourceType::Carbon]))
    );
    t.join().unwrap();
}

#[test]
fn a_late_reply_of_the_wrong_kind_is_skipped() {
    let (mut eco, planet) = rig();
    let t = thread::spawn(move || {
        planet.requests.recv().unwrap();
        // First a late answer to some OTHER question, then the real one.
        planet
            .replies
            .send(PlanetToExplorer::SupportedCombinationResponse { combination_list: HashSet::new() })
            .unwrap();
        planet
            .replies
            .send(PlanetToExplorer::SupportedResourceResponse {
                resource_list: HashSet::from([BasicResourceType::Hydrogen]),
            })
            .unwrap();
    });
    assert_eq!(
        eco.query_supported_resources(),
        Ok(HashSet::from([BasicResourceType::Hydrogen]))
    );
    t.join().unwrap();
}