//! Spec §4.2: Eco logs the messages it receives and sends, in the standard
//! `common_game::logging::LogEvent` format.

use std::sync::Mutex;
use std::time::Duration;

use common_game::protocols::orchestrator_explorer::{
    ExplorerToOrchestrator, OrchestratorToExplorer,
};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use crossbeam_channel::unbounded;
use log::{LevelFilter, Log, Metadata, Record};

/// A logger that just remembers every line.
struct Capture(Mutex<Vec<String>>);

impl Log for Capture {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        self.0.lock().unwrap().push(format!("{}", record.args()));
    }
    fn flush(&self) {}
}

static CAPTURE: Capture = Capture(Mutex::new(Vec::new()));

#[test]
fn messages_with_the_orchestrator_are_logged_as_log_events() {
    log::set_logger(&CAPTURE).unwrap();
    log::set_max_level(LevelFilter::Trace);

    let (to_explorer, rx_orchestrator) = unbounded::<OrchestratorToExplorer>();
    let (tx_orchestrator, from_explorer) =
        unbounded::<ExplorerToOrchestrator<explorer_eco::BagContent>>();
    let (tx_planet, _planet_rx) = unbounded::<ExplorerToPlanet>();
    let (_planet_tx, rx_planet) = unbounded::<PlanetToExplorer>();

    let explorer = explorer_eco::create_explorer(
        1,
        100,
        rx_orchestrator,
        tx_orchestrator,
        rx_planet,
        tx_planet,
    );
    let handle = std::thread::spawn(move || explorer.run());

    to_explorer
        .send(OrchestratorToExplorer::BagContentRequest)
        .unwrap();
    // CurrentPlanetResult (sent at start) and BagContentResponse
    from_explorer.recv_timeout(Duration::from_secs(2)).unwrap();
    from_explorer.recv_timeout(Duration::from_secs(2)).unwrap();
    to_explorer
        .send(OrchestratorToExplorer::KillExplorer)
        .unwrap();
    handle.join().unwrap();

    let lines = CAPTURE.0.lock().unwrap();
    assert!(lines.iter().any(|l| {
        l.contains("MessageOrchestratorToExplorer") && l.contains("BagContentRequest")
    }));
    assert!(lines.iter().any(|l| {
        l.contains("MessageExplorerToOrchestrator") && l.contains("BagContentResponse")
    }));
}
