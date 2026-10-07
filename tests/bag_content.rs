use std::thread;
use std::time::Duration;
use crossbeam_channel::unbounded;
use common_game::protocols::orchestrator_explorer::{ExplorerToOrchestrator, OrchestratorToExplorer};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use explorer_eco::{create_explorer, BagContent};

#[test]
fn bag_content_request_returns_empty_bag_for_new_explorer() {
    let (tx_orch, rx_orch) = unbounded::<OrchestratorToExplorer>();
    let (tx_to_orch, rx_from_explorer) = unbounded::<ExplorerToOrchestrator<BagContent>>();
    let (tx_planet, _rx_planet_side) = unbounded::<ExplorerToPlanet>();
    let (_tx_reply, rx_planet) = unbounded::<PlanetToExplorer>();

    let explorer = create_explorer(7, 100, rx_orch, tx_to_orch, rx_planet, tx_planet);
    let handle = thread::spawn(move || explorer.run());

    tx_orch.send(OrchestratorToExplorer::BagContentRequest).unwrap();
    let (id, bag) = loop {
        match rx_from_explorer.recv_timeout(Duration::from_secs(3)).expect("no reply") {
            ExplorerToOrchestrator::BagContentResponse { explorer_id, bag_content } => break (explorer_id, bag_content),
            _ => continue,
        }
    };
    assert_eq!(id, 7);
    assert!(bag.is_empty());

    tx_orch.send(OrchestratorToExplorer::KillExplorer).unwrap();
    handle.join().unwrap();
}