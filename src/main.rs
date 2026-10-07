use explorer_eco::create_explorer;

use std::thread;
use std::time::Duration;

use crossbeam_channel::unbounded;

use common_game::protocols::orchestrator_explorer::OrchestratorToExplorer;
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use common_game::utils::ID;

fn main() {
    env_logger::init();

    // Channels connecting "the orchestrator" (this main function, standing in
    // for the real one) to Eco.
    let (tx_to_explorer, rx_from_orchestrator) = unbounded::<OrchestratorToExplorer>();
    let (tx_to_orchestrator, rx_from_explorer) = unbounded();

    let explorer_id: ID = 1;
    let starting_planet_id: ID = 100;

    let (tx_planet, _rx_planet_side) = unbounded::<ExplorerToPlanet>();
    let (_tx_planet_reply, rx_planet) = unbounded::<PlanetToExplorer>();

    let explorer = explorer_eco::create_explorer(
        explorer_id,
        starting_planet_id,
        rx_from_orchestrator,
        tx_to_orchestrator,
        rx_planet,
        tx_planet,
    );

    // Eco runs its own loop on its own thread, exactly like the real
    // orchestrator would spawn it.
    let handle = thread::spawn(move || explorer.run());

    // Drain whatever Eco sends back, just so we can see it happening.
    let listener = thread::spawn(move || {
        while let Ok(msg) = rx_from_explorer.recv() {
            println!("Eco -> orchestrator: {msg:?}");
        }
    });

    // Kick the AI on.
    tx_to_explorer.send(OrchestratorToExplorer::StartExplorerAI).unwrap();

    // Let it run for a bit so you can watch the economy tick in the logs
    // (run with RUST_LOG=info to see them).
    thread::sleep(Duration::from_secs(5));

    tx_to_explorer.send(OrchestratorToExplorer::KillExplorer).unwrap();

    handle.join().unwrap();
    drop(tx_to_explorer);
    let _ = listener.join();
}

//to check git function