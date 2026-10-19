#![allow(clippy::missing_panics_doc, clippy::must_use_candidate)]
#![allow(dead_code)] // each test crate uses only part of this toolkit

use std::collections::HashMap;
use std::sync::OnceLock;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use common_game::components::forge::Forge;
use common_game::components::planet::{
    DummyPlanetState, Planet, PlanetAI, PlanetState, PlanetType,
};
use common_game::components::resource::{
    BasicResourceType, Combinator, ComplexResourceType, Generator, ResourceType,
};
use common_game::components::rocket::Rocket;
use common_game::components::sunray::Sunray;
use common_game::protocols::orchestrator_explorer::{
    ExplorerToOrchestrator, OrchestratorToExplorer,
};
use common_game::protocols::orchestrator_planet::{OrchestratorToPlanet, PlanetToOrchestrator};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use common_game::utils::ID;
use crossbeam_channel::{Receiver, Sender, unbounded};
use explorer_eco::{BagContent, create_explorer};

/// How long the test side waits for a planet to answer the "orchestrator".
const PLANET_TIMEOUT: Duration = Duration::from_secs(2);

/// Sunrays can only be made by a `Forge`, and only one may exist per process.
fn forge() -> &'static Forge {
    static FORGE: OnceLock<Forge> = OnceLock::new();
    FORGE.get_or_init(|| Forge::new().expect("only one Forge per test process"))
}

// ---------------------------------------------------------------------------
// A minimal planet brain. The real Planet runtime does the protocol work
// (registering explorers, replying Stopped, ...); this only answers questions
// and spends charged energy cells on generating/combining.
// ---------------------------------------------------------------------------
struct TestAI;

fn charged_cell_index(state: &PlanetState) -> usize {
    (0..state.cells_count())
        .find(|&i| state.cell(i).is_charged())
        .unwrap_or(0)
}

impl PlanetAI for TestAI {
    fn handle_sunray(
        &mut self,
        state: &mut PlanetState,
        _g: &Generator,
        _c: &Combinator,
        sunray: Sunray,
    ) {
        let _ = state.charge_cell(sunray);
    }

    fn handle_asteroid(
        &mut self,
        _s: &mut PlanetState,
        _g: &Generator,
        _c: &Combinator,
    ) -> Option<Rocket> {
        None
    }

    fn handle_internal_state_req(
        &mut self,
        state: &mut PlanetState,
        _g: &Generator,
        _c: &Combinator,
    ) -> DummyPlanetState {
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
            ExplorerToPlanet::GenerateResourceRequest { resource, .. } => {
                let i = charged_cell_index(state);
                let made = generator.try_make(resource, state.cell_mut(i)).ok();
                Some(PlanetToExplorer::GenerateResourceResponse { resource: made })
            }
            ExplorerToPlanet::CombineResourceRequest { msg, .. } => {
                // With no charged cell, `try_make` fails and hands the
                // ingredients back, which is exactly what the protocol wants.
                let i = charged_cell_index(state);
                Some(PlanetToExplorer::CombineResourceResponse {
                    complex_response: combinator.try_make(msg, state.cell_mut(i)),
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A real Planet running on its own thread, plus the "orchestrator side" of it.
// ---------------------------------------------------------------------------
pub struct TestPlanet {
    pub id: ID,
    /// The sender Eco must use to talk to this planet (what `MoveToPlanet` carries).
    pub to_planet: Sender<ExplorerToPlanet>,
    to_planet_orch: Sender<OrchestratorToPlanet>,
    from_planet_orch: Receiver<PlanetToOrchestrator>,
    thread: Option<JoinHandle<()>>,
}

impl TestPlanet {
    /// Spawns a real `Planet`. It starts STOPPED (as the real runtime does):
    /// call `start()` to make it answer explorers.
    #[must_use]
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

        TestPlanet {
            id,
            to_planet,
            to_planet_orch,
            from_planet_orch,
            thread: Some(thread),
        }
    }

    fn expect_reply(&self, what: &str) -> PlanetToOrchestrator {
        self.from_planet_orch
            .recv_timeout(PLANET_TIMEOUT)
            .unwrap_or_else(|_| panic!("planet {} gave no reply to {what}", self.id))
    }

    pub fn start(&self) {
        self.to_planet_orch
            .send(OrchestratorToPlanet::StartPlanetAI)
            .unwrap();
        match self.expect_reply("StartPlanetAI") {
            PlanetToOrchestrator::StartPlanetAIResult { .. } => {}
            other => panic!("expected StartPlanetAIResult, got {other:?}"),
        }
    }

    pub fn stop(&self) {
        self.to_planet_orch
            .send(OrchestratorToPlanet::StopPlanetAI)
            .unwrap();
        match self.expect_reply("StopPlanetAI") {
            PlanetToOrchestrator::StopPlanetAIResult { .. } => {}
            other => panic!("expected StopPlanetAIResult, got {other:?}"),
        }
    }

    /// Charges one energy cell (the orchestrator sends the planet a sunray).
    pub fn sunray(&self) {
        self.to_planet_orch
            .send(OrchestratorToPlanet::Sunray(forge().generate_sunray()))
            .unwrap();
        match self.expect_reply("Sunray") {
            PlanetToOrchestrator::SunrayAck { .. } => {}
            other => panic!("expected SunrayAck, got {other:?}"),
        }
    }

    /// What the orchestrator does to let an explorer onto this planet
    /// (Incoming request), with a fresh reply channel; returns its receiving end.
    #[must_use]
    pub fn admit(&self, explorer_id: ID) -> Receiver<PlanetToExplorer> {
        let (tx, rx) = unbounded::<PlanetToExplorer>();
        self.admit_via(explorer_id, tx);
        rx
    }

    /// Same, but the planet will answer through `new_sender`. Eco's receiving
    /// end is fixed for its whole life, so every planet it visits must answer
    /// into the same channel.
    pub fn admit_via(&self, explorer_id: ID, new_sender: Sender<PlanetToExplorer>) {
        self.to_planet_orch
            .send(OrchestratorToPlanet::IncomingExplorerRequest {
                explorer_id,
                new_sender,
            })
            .unwrap();
        match self.expect_reply("IncomingExplorerRequest") {
            PlanetToOrchestrator::IncomingExplorerResponse { res: Ok(()), .. } => {}
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
    /// Every planet Eco visits must answer into (a clone of) this sender.
    pub reply_tx: Sender<PlanetToExplorer>,
    to_explorer: Sender<OrchestratorToExplorer>,
    from_explorer: Receiver<ExplorerToOrchestrator<BagContent>>,
    thread: Option<JoinHandle<()>>,
}

/// Admits explorer `explorer_id` on `planet` (as the orchestrator would) and
/// starts Eco on its own thread. Eco is in manual mode until `send(StartExplorerAI)`.
#[must_use]
pub fn spawn_explorer(explorer_id: ID, planet: &TestPlanet) -> RunningExplorer {
    spawn_with(explorer_id, planet, None)
}

/// Same, but Eco starts with the given task instead of a random one.
#[must_use]
pub fn spawn_explorer_with_task(
    explorer_id: ID,
    planet: &TestPlanet,
    task: ComplexResourceType,
) -> RunningExplorer {
    spawn_with(explorer_id, planet, Some(task))
}

fn spawn_with(
    explorer_id: ID,
    planet: &TestPlanet,
    task: Option<ComplexResourceType>,
) -> RunningExplorer {
    let (reply_tx, rx_planet) = unbounded::<PlanetToExplorer>();
    planet.admit_via(explorer_id, reply_tx.clone());
    let (to_explorer, rx_orch) = unbounded::<OrchestratorToExplorer>();
    let (tx_orch, from_explorer) = unbounded::<ExplorerToOrchestrator<BagContent>>();

    let mut explorer = create_explorer(
        explorer_id,
        planet.id,
        rx_orch,
        tx_orch,
        rx_planet,
        planet.to_planet.clone(),
    );
    if let Some(task) = task {
        explorer = explorer.with_task(task);
    }
    let thread = thread::spawn(move || explorer.run());

    RunningExplorer {
        explorer_id,
        reply_tx,
        to_explorer,
        from_explorer,
        thread: Some(thread),
    }
}

impl RunningExplorer {
    pub fn send(&self, msg: OrchestratorToExplorer) {
        self.to_explorer.send(msg).unwrap();
    }

    /// The next message Eco sends to the orchestrator, if one arrives in time.
    #[must_use]
    pub fn next_message(&self, timeout: Duration) -> Option<ExplorerToOrchestrator<BagContent>> {
        self.from_explorer.recv_timeout(timeout).ok()
    }

    /// Reads messages Eco sends to the orchestrator until `pick` returns
    /// `Some`, or `timeout` passes. Non-matching messages are skipped
    /// (e.g. the `CurrentPlanetResult` Eco sends when it starts).
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

// ---------------------------------------------------------------------------
// A small galaxy plus a scripted orchestrator that serves Eco's requests.
// ---------------------------------------------------------------------------
pub struct Galaxy {
    planets: HashMap<ID, TestPlanet>,
    edges: HashMap<ID, Vec<ID>>,
    /// Orchestrators differ: the diagram does Incoming then Outgoing, Bipolar
    /// does Outgoing then Incoming. Eco must not care.
    outgoing_first: bool,
}

impl Galaxy {
    #[must_use]
    pub fn new(outgoing_first: bool) -> Self {
        Galaxy {
            planets: HashMap::new(),
            edges: HashMap::new(),
            outgoing_first,
        }
    }

    pub fn add_planet(&mut self, planet: TestPlanet) {
        self.edges.entry(planet.id).or_default();
        self.planets.insert(planet.id, planet);
    }

    pub fn connect(&mut self, a: ID, b: ID) {
        self.edges.entry(a).or_default().push(b);
        self.edges.entry(b).or_default().push(a);
    }

    #[must_use]
    pub fn planet(&self, id: ID) -> &TestPlanet {
        &self.planets[&id]
    }

    pub fn start_all(&self) {
        for planet in self.planets.values() {
            planet.start();
        }
    }

    fn neighbors(&self, id: ID) -> Vec<ID> {
        self.edges.get(&id).cloned().unwrap_or_default()
    }

    /// The orchestrator's move choreography for one accepted `TravelToPlanetRequest`.
    fn move_explorer(&self, eco: &RunningExplorer, from: ID, to: ID) {
        let (src, dst) = (&self.planets[&from], &self.planets[&to]);
        if self.outgoing_first {
            src.release(eco.explorer_id);
            dst.admit_via(eco.explorer_id, eco.reply_tx.clone());
        } else {
            dst.admit_via(eco.explorer_id, eco.reply_tx.clone());
            src.release(eco.explorer_id);
        }
        eco.send(OrchestratorToExplorer::MoveToPlanet {
            sender_to_new_planet: Some(dst.to_planet.clone()),
            planet_id: to,
        });
    }

    /// Plays the orchestrator for `eco` (answering neighbour and travel
    /// requests) while polling its bag, until it holds `want` or `timeout` passes.
    #[must_use]
    pub fn serve_until_bag_contains(
        &self,
        eco: &RunningExplorer,
        want: ResourceType,
        timeout: Duration,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        let mut last_poll = Instant::now();
        while Instant::now() < deadline {
            if last_poll.elapsed() >= Duration::from_millis(250) {
                eco.send(OrchestratorToExplorer::BagContentRequest);
                last_poll = Instant::now();
            }
            let Some(msg) = eco.next_message(Duration::from_millis(50)) else {
                continue;
            };
            match msg {
                ExplorerToOrchestrator::NeighborsRequest {
                    current_planet_id, ..
                } => {
                    eco.send(OrchestratorToExplorer::NeighborsResponse {
                        neighbors: self.neighbors(current_planet_id),
                    });
                }
                ExplorerToOrchestrator::TravelToPlanetRequest {
                    current_planet_id,
                    dst_planet_id,
                    ..
                } => {
                    self.move_explorer(eco, current_planet_id, dst_planet_id);
                }
                ExplorerToOrchestrator::BagContentResponse { bag_content, .. }
                    if bag_content
                        .iter()
                        .any(|(kind, count)| kind == &want && *count > 0) =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }
}
