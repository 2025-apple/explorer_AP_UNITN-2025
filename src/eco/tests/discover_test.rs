use std::collections::HashSet;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crossbeam_channel::{unbounded, Receiver, Sender};

use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::protocols::orchestrator_explorer::{ExplorerToOrchestrator, OrchestratorToExplorer};
use common_game::protocols::planet_explorer::{
    ExplorerToPlanet, ExplorerToPlanetKind, PlanetToExplorer,
};

use crate::eco::explorer::{BagContent, Explorer};

type FromEco = Receiver<ExplorerToOrchestrator<BagContent>>;

/// Builds an Eco on planet 100 with a Water task, plus a fake planet thread
/// that answers every request with `answer` and returns, when Eco is
/// dropped, the kinds of request it received (in order).
fn start(
    answer: impl Fn(&ExplorerToPlanet) -> PlanetToExplorer + Send + 'static,
) -> (Explorer, FromEco, Sender<OrchestratorToExplorer>, JoinHandle<Vec<ExplorerToPlanetKind>>) {
    let (to_eco, rx_orch) = unbounded::<OrchestratorToExplorer>();
    let (tx_orch, from_eco) = unbounded::<ExplorerToOrchestrator<BagContent>>();
    let (tx_planet, requests) = unbounded::<ExplorerToPlanet>();
    let (replies, rx_planet) = unbounded::<PlanetToExplorer>();

    let mut eco = Explorer::new(1, 100, rx_orch, tx_orch, rx_planet, tx_planet, true);
    // A fixed task keeps the test deterministic (no random first step).
    eco.task = Some(ComplexResourceType::Water);

    let responder = thread::spawn(move || {
        let mut seen = Vec::new();
        while let Ok(req) = requests.recv() {
            seen.push(ExplorerToPlanetKind::from(&req));
            if replies.send(answer(&req)).is_err() {
                break;
            }
        }
        seen
    });
    (eco, from_eco, to_eco, responder)
}

fn helpful_planet(req: &ExplorerToPlanet) -> PlanetToExplorer {
    match req {
        ExplorerToPlanet::SupportedResourceRequest { .. } => PlanetToExplorer::SupportedResourceResponse {
            resource_list: HashSet::from([BasicResourceType::Carbon]),
        },
        ExplorerToPlanet::SupportedCombinationRequest { .. } => PlanetToExplorer::SupportedCombinationResponse {
            combination_list: HashSet::from([ComplexResourceType::Diamond]),
        },
        _ => PlanetToExplorer::Stopped,
    }
}

fn planet_without_combinations(req: &ExplorerToPlanet) -> PlanetToExplorer {
    match req {
        ExplorerToPlanet::SupportedCombinationRequest { .. } => PlanetToExplorer::SupportedCombinationResponse {
            combination_list: HashSet::new(),
        },
        other => helpful_planet(other),
    }
}

fn stopped_planet(_: &ExplorerToPlanet) -> PlanetToExplorer {
    PlanetToExplorer::Stopped
}

fn neighbors_requests(from_eco: &FromEco) -> usize {
    from_eco
        .try_iter()
        .filter(|m| matches!(m, ExplorerToOrchestrator::NeighborsRequest { .. }))
        .count()
}

#[test]
fn discovery_asks_the_planet_once_and_costs_nothing() {
    let (mut eco, from_eco, _keep, responder) = start(helpful_planet);
    for _ in 0..4 {
        eco.ai_step();
    }

    assert_eq!(
        eco.world.resources.get(&100),
        Some(&HashSet::from([BasicResourceType::Carbon]))
    );
    assert_eq!(
        eco.world.combos.get(&100),
        Some(&HashSet::from([ComplexResourceType::Diamond]))
    );
    // Free and not a clock event: 4 steps = 4 ticks, no charge.
    assert_eq!(eco.wallet.coins, 120);
    assert_eq!(eco.clock.cycle_in_day, 4);
    // The orchestrator was never asked about capabilities: it only got the
    // (free) neighbors request.
    assert_eq!(neighbors_requests(&from_eco), 1);
    assert!(from_eco.try_recv().is_err());

    drop(eco);
    assert_eq!(
        responder.join().unwrap(),
        vec![
            ExplorerToPlanetKind::SupportedResourceRequest,
            ExplorerToPlanetKind::SupportedCombinationRequest,
        ]
    );
}

#[test]
fn a_stopped_planet_is_not_marked_known_and_does_not_trap_eco() {
    let (mut eco, from_eco, _keep, responder) = start(stopped_planet);
    for _ in 0..4 {
        eco.ai_step();
    }

    assert!(eco.world.resources.is_empty());
    assert!(eco.world.combos.is_empty());
    // Exploration still went ahead despite the failed probe.
    assert_eq!(neighbors_requests(&from_eco), 1);
    assert!(!eco.comms.may_probe(Instant::now())); // probing is cooling down

    drop(eco);
    // One failed probe only; the combinations question was skipped.
    assert_eq!(
        responder.join().unwrap(),
        vec![ExplorerToPlanetKind::SupportedResourceRequest]
    );
}

#[test]
fn an_empty_combination_set_is_a_real_answer_and_is_not_asked_again() {
    let (mut eco, _from_eco, _keep, responder) = start(planet_without_combinations);
    for _ in 0..4 {
        eco.ai_step();
    }

    assert_eq!(eco.world.combos.get(&100), Some(&HashSet::new()));

    drop(eco);
    assert_eq!(responder.join().unwrap().len(), 2); // each question asked exactly once
}