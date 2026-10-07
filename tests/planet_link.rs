use std::collections::HashSet;
use std::thread;
use std::time::Duration;
use crossbeam_channel::unbounded;
use common_game::components::resource::{BasicResourceType, GenericResource};
use common_game::protocols::orchestrator_explorer::{ExplorerToOrchestrator, OrchestratorToExplorer};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use explorer_eco::{create_explorer, BagContent};
#[test]
fn supported_resource_request_reaches_planet_and_back() {
    let (tx_orch, rx_orch) = unbounded::<OrchestratorToExplorer>();
    let (tx_to_orch, rx_from_explorer) = unbounded::<ExplorerToOrchestrator<BagContent>>();    let (tx_planet, rx_planet_side) = unbounded::<ExplorerToPlanet>();
    let (tx_reply, rx_planet) = unbounded::<PlanetToExplorer>();

    let planet = thread::spawn(move || {
        if let Ok(ExplorerToPlanet::SupportedResourceRequest { .. }) =
            rx_planet_side.recv_timeout(Duration::from_secs(2))
        {
            tx_reply.send(PlanetToExplorer::SupportedResourceResponse {
                resource_list: HashSet::from([BasicResourceType::Carbon]),
            }).unwrap();
        }
    });

    let explorer = create_explorer(1, 100, rx_orch, tx_to_orch, rx_planet, tx_planet);
    let handle = thread::spawn(move || explorer.run());

    tx_orch.send(OrchestratorToExplorer::SupportedResourceRequest).unwrap();
    let got = loop {
        match rx_from_explorer.recv_timeout(Duration::from_secs(3)).expect("no reply") {
            ExplorerToOrchestrator::SupportedResourceResult { supported_resources, .. } => break supported_resources,
            _ => continue,
        }
    };
    assert_eq!(got, HashSet::from([BasicResourceType::Carbon]));

    tx_orch.send(OrchestratorToExplorer::KillExplorer).unwrap();
    handle.join().unwrap();
    planet.join().unwrap();
}