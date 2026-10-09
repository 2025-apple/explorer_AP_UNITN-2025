#![allow(dead_code)] // each test crate uses only part of this toolkit

use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use common_game::components::planet::{
    DummyPlanetState, Planet, PlanetAI, PlanetState, PlanetType,
};
use common_game::components::resource::{
    BasicResourceType, Combinator, ComplexResourceType, Generator,
};
use common_game::components::rocket::Rocket;
use common_game::components::sunray::Sunray;
use common_game::protocols::orchestrator_explorer::{ExplorerToOrchestrator, OrchestratorToExplorer};
use common_game::protocols::orchestrator_planet::{OrchestratorToPlanet, PlanetToOrchestrator};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use common_game::utils::ID;
use crossbeam_channel::{unbounded, Receiver, Sender};
use explorer_eco::{create_explorer, BagContent};

/// How long the test side waits for a planet to answer the "orchestrator".
const PLANET_TIMEOUT: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// A minimal planet brain. The real Planet runtime does the protocol work
// (registering explorers, replying Stopped, ...); this only answers questions.
// ---------------------------------------------------------------------------
struct TestAI;

impl PlanetAI for TestAI {
    fn handle_sunray(&mut self, state: &mut PlanetState, _g: &Generator, _c: &Combinator, sunray: Sunray) {
        let _ = state.charge_cell(sunray);
    }

    fn handle_asteroid(&mut self, _s: &mut PlanetState, _g: &Generator, _c: &Combinator) -> Option<Rocket> {
        None
    }

    fn handle_internal_state_req(&mut self, state: &mut PlanetState, _g: &Generator, _c: &Combinator) -> DummyPlanetState {
        state.to_dummy()
    }

    fn handle_explorer_msg(
        &mut self,
        state: &mut PlanetState,
        generator: &Generator,
        combinator: &Combinator,
        msg: ExplorerToPlanet,
    ) -> Option<PlanetToExplorer> {
        match msg {
            ExplorerToPlanet::SupportedResourceRequest { .. } => {
                Some(PlanetToExplorer::SupportedResourceResponse {
                    resource_list: generator.all_available_recipes(),
                })
            }
            ExplorerToPlanet::SupportedCombinationRequest { .. } => {
                Some(PlanetToExplorer::SupportedCombinationResponse {
                    combination_list: combinator.all_available_recipes(),
                })
            }
            ExplorerToPlanet::AvailableEnergyCellRequest { .. } => {
                let charged = state.cells_iter().filter(|c| c.is_charged()).count();
                Some(PlanetToExplorer::AvailableEnergyCellResponse {
                    available_cells: charged as ID,
                })
            }
            // Generate/Combine are added in the capstone step.
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// A real Planet running on its own thread, plus the "orchestrator side" of it.
// ---------------------------------------------------------------------------
pub struct TestPlanet {
    pub id: ID,
    /// The sender Eco must use to talk to this planet (what MoveToPlanet carries).
    pub to_planet: Sender<ExplorerToPlanet>,
    to_planet_orch: Sender<OrchestratorToPlanet>,
    from_planet_orch: Receiver<PlanetToOrchestrator>,
    thread: Option<JoinHandle<()>>,
}

impl TestPlanet {
    /// Spawns a real `Planet`. It starts STOPPED (as the real runtime does):
    /// call `start()` to make it answer explorers.
    pub fn spawn(
        id: ID,
        planet_type: PlanetType,
        gen_rules: Vec<BasicResourceType>,
        comb_rules: Vec<ComplexResourceType>,
    ) -> Self {
        let (to_planet_orch, rx_orch) = unbounded::<OrchestratorToPlanet>();
        let (tx_orch, from_planet_orch) = unbounded::<PlanetToOrchestrator>();
        let (to_planet, rx_expl) = unbounded::<ExplorerToPlanet>();

        let mut planet = Planet::new(
            id,
            planet_type,
            Box::new(TestAI),
            gen_rules,
            comb_rules,
            (rx_orch, tx_orch),
            rx_expl,
        )
            .expect("test planet rules must be valid for the planet type");

        let thread = thread::spawn(move || {
            let _ = planet.run();
        });

        TestPlanet { id, to_planet, to_planet_orch, from_planet_orch, thread: Some(thread) }
    }

    fn expect_reply(&self, what: &str) -> PlanetToOrchestrator {
        self.from_planet_orch
            .recv_timeout(PLANET_TIMEOUT)
            .unwrap_or_else(|_| panic!("planet {} gave no reply to {what}", self.id))
    }

    pub fn start(&self) {
        self.to_planet_orch.send(OrchestratorToPlanet::StartPlanetAI).unwrap();
        match self.expect_reply("StartPlanetAI") {
            PlanetToOrchestrator::StartPlanetAIResult { .. } => {}
            other => panic!("expected StartPlanetAIResult, got {other:?}"),
        }
    }

    pub fn stop(&self) {
        self.to_planet_orch.send(OrchestratorToPlanet::StopPlanetAI).unwrap();
        match self.expect_reply("StopPlanetAI") {
            PlanetToOrchestrator::StopPlanetAIResult { .. } => {}
            other => panic!("expected StopPlanetAIResult, got {other:?}"),
        }
    }

    /// What the orchestrator does to let an explorer onto this planet
    /// (Incoming request); returns the channel on which the planet will
    /// answer that explorer. Only valid while the planet is started.
    pub fn admit(&self, explorer_id: ID) -> Receiver<PlanetToExplorer> {
        let (tx, rx) = unbounded::<PlanetToExplorer>();
        self.to_planet_orch
            .send(OrchestratorToPlanet::IncomingExplorerRequest { explorer_id, new_sender: tx })
            .unwrap();
        match self.expect_reply("IncomingExplorerRequest") {
            PlanetToOrchestrator::IncomingExplorerResponse { res: Ok(()), .. } => rx,
            other => panic!("expected a successful IncomingExplorerResponse, got {other:?}"),
        }
    }

    /// What the orchestrator does when an explorer leaves (Outgoing request).
    pub fn release(&self, explorer_id: ID) {
        self.to_planet_orch
            .send(OrchestratorToPlanet::OutgoingExplorerRequest { explorer_id })
            .unwrap();
        match self.expect_reply("OutgoingExplorerRequest") {
            PlanetToOrchestrator::OutgoingExplorerResponse { res: Ok(()), .. } => {}
            other => panic!("expected a successful OutgoingExplorerResponse, got {other:?}"),
        }
    }
}

impl Drop for TestPlanet {
    fn drop(&mut self) {
        // KillPlanet works whether the planet is started or stopped.
        let _ = self.to_planet_orch.send(OrchestratorToPlanet::KillPlanet);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

// ---------------------------------------------------------------------------
// A real Eco running on its own thread, plus the orchestrator side of it.
// ---------------------------------------------------------------------------
pub struct RunningExplorer {
    pub explorer_id: ID,
    to_explorer: Sender<OrchestratorToExplorer>,
    from_explorer: Receiver<ExplorerToOrchestrator<BagContent>>,
    thread: Option<JoinHandle<()>>,
}

/// Admits explorer `explorer_id` on `planet` (as the orchestrator would) and
/// starts Eco on its own thread. Eco is in manual mode until `send(StartExplorerAI)`.
pub fn spawn_explorer(explorer_id: ID, planet: &TestPlanet) -> RunningExplorer {
    let rx_planet = planet.admit(explorer_id);
    let (to_explorer, rx_orch) = unbounded::<OrchestratorToExplorer>();
    let (tx_orch, from_explorer) = unbounded::<ExplorerToOrchestrator<BagContent>>();

    let explorer = create_explorer(
        explorer_id,
        planet.id,
        rx_orch,
        tx_orch,
        rx_planet,
        planet.to_planet.clone(),
    );
    let thread = thread::spawn(move || explorer.run());

    RunningExplorer { explorer_id, to_explorer, from_explorer, thread: Some(thread) }
}

impl RunningExplorer {
    pub fn send(&self, msg: OrchestratorToExplorer) {
        self.to_explorer.send(msg).unwrap();
    }

    /// Reads messages Eco sends to the orchestrator until `pick` returns
    /// `Some`, or `timeout` passes. Non-matching messages are skipped
    /// (e.g. the CurrentPlanetResult Eco sends when it starts).
    pub fn wait_for<T>(
        &self,
        timeout: Duration,
        mut pick: impl FnMut(ExplorerToOrchestrator<BagContent>) -> Option<T>,
    ) -> Option<T> {
        let deadline = Instant::now() + timeout;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.from_explorer.recv_timeout(left) {
                Ok(msg) => {
                    if let Some(found) = pick(msg) {
                        return Some(found);
                    }
                }
                Err(_) => return None,
            }
        }
    }
}

impl Drop for RunningExplorer {
    fn drop(&mut self) {
        let _ = self.to_explorer.send(OrchestratorToExplorer::KillExplorer);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}