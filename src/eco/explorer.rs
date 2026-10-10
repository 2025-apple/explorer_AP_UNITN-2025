//! The `Explorer` struct itself: protocol handling (unchanged from the
//! skeleton) plus the wiring that drives Eco's private economy and
//! planner every `ai_step`.

use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use common_game::components::resource::{
    BasicResourceType, ComplexResourceRequest, ComplexResourceType, GenericResource, ResourceType,
};
use common_game::protocols::orchestrator_explorer::{
    ExplorerToOrchestrator, OrchestratorToExplorer,
};
use common_game::protocols::planet_explorer::{
    ExplorerToPlanet, ExplorerToPlanetKind, PlanetToExplorer, PlanetToExplorerKind,
};

use common_game::utils::ID;

use super::bag::Bag;
use super::logging;
use super::planner::{Action, Planner};
use super::recipes::ALL_COMPLEX_RESOURCES;
use super::regime::{ForecastMode, RegimeEstimator};
use super::time::EconomyClock;
use super::wallet::Wallet;
use super::world::WorldModel;
use super::comms::Comms;

/// Eco's bag report: resource type and how many.
pub type BagContent = Vec<(ResourceType, usize)>;

#[must_use]
pub fn create_explorer(
    explorer_id: ID,
    starting_planet_id: ID,
    rx_orchestrator: Receiver<OrchestratorToExplorer>,
    tx_orchestrator: Sender<ExplorerToOrchestrator<BagContent>>,
    rx_planet: Receiver<PlanetToExplorer>,
    tx_planet: Sender<ExplorerToPlanet>,
) -> Explorer {
    // Blind mode by default — the realistic, harder AI behavior. Use
    // `create_explorer_oracle` for a baseline/testing build.
    Explorer::new(explorer_id, starting_planet_id, rx_orchestrator, tx_orchestrator, rx_planet, tx_planet, true)
}

#[must_use]
pub fn create_explorer_oracle(
    explorer_id: ID,
    starting_planet_id: ID,
    rx_orchestrator: Receiver<OrchestratorToExplorer>,
    tx_orchestrator: Sender<ExplorerToOrchestrator<BagContent>>,
    rx_planet: Receiver<PlanetToExplorer>,
    tx_planet: Sender<ExplorerToPlanet>,
) -> Explorer {
    Explorer::new(explorer_id, starting_planet_id, rx_orchestrator, tx_orchestrator, rx_planet, tx_planet, false)
}

/// Eco's link to whichever planet it's currently on. `None` until the
/// first successful `MoveToPlanet` — see module docs on why the
/// starting planet has no link.
#[derive(Default)]
struct PlanetLink {
    to_planet: Option<Sender<ExplorerToPlanet>>,
    from_planet: Option<Receiver<PlanetToExplorer>>,
}

impl PlanetLink {
    fn is_connected(&self) -> bool {
        self.to_planet.is_some()
    }
}

/// How long Eco waits for one planet reply.
const PLANET_REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// Why a planet request produced no usable answer. All of these mean
/// "we learned nothing": callers must NOT record any knowledge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlanetError {
    NoLink,  // no channel to a planet yet
    Gone,    // channel closed
    NoReply, // timed out
    Stopped, // planet answered `Stopped`
}

impl PlanetError {
    fn describe(self) -> &'static str {
        match self {
            PlanetError::NoLink => "not currently linked to a planet",
            PlanetError::Gone => "planet unreachable",
            PlanetError::NoReply => "no response from planet",
            PlanetError::Stopped => "planet is stopped",
        }
    }
}

/// Which reply kind answers which request kind.
fn reply_kind(request: ExplorerToPlanetKind) -> PlanetToExplorerKind {
    match request {
        ExplorerToPlanetKind::SupportedResourceRequest => PlanetToExplorerKind::SupportedResourceResponse,
        ExplorerToPlanetKind::SupportedCombinationRequest => PlanetToExplorerKind::SupportedCombinationResponse,
        ExplorerToPlanetKind::GenerateResourceRequest => PlanetToExplorerKind::GenerateResourceResponse,
        ExplorerToPlanetKind::CombineResourceRequest => PlanetToExplorerKind::CombineResourceResponse,
        ExplorerToPlanetKind::AvailableEnergyCellRequest => PlanetToExplorerKind::AvailableEnergyCellResponse,
    }
}

pub struct Explorer {
    id: ID,

    from_orchestrator: Receiver<OrchestratorToExplorer>,
    to_orchestrator: Sender<ExplorerToOrchestrator<BagContent>>,

    planet_link: PlanetLink,
    pub(super) current_planet_id: ID,

    bag: Bag,
    ai_active: bool,
    should_stop: bool,

    // ---- Eco's own economy + planning state (no protocol involvement) ----
    pub(super) clock: EconomyClock,
    pub(super) wallet: Wallet,
    estimator: RegimeEstimator,
    pub(super) world: WorldModel,
    pub(super) task: Option<ComplexResourceType>,
    blind_mode: bool,
    rng: StdRng,
    pub(super) comms: Comms,
}

impl Explorer {
    pub(super) fn new(
        id: ID,
        starting_planet_id: ID,
        from_orchestrator: Receiver<OrchestratorToExplorer>,
        to_orchestrator: Sender<ExplorerToOrchestrator<BagContent>>,
        from_planet: Receiver<PlanetToExplorer>,
        to_planet: Sender<ExplorerToPlanet>,
        blind_mode: bool,
    ) -> Self {
        let mut world = WorldModel::default();
        world.visited.insert(starting_planet_id);

        Self {
            id,
            from_orchestrator,
            to_orchestrator,
            planet_link: PlanetLink {
                to_planet: Some(to_planet),
                from_planet: Some(from_planet),
            },
            current_planet_id: starting_planet_id,
            bag: Bag::default(),
            ai_active: false,
            should_stop: false,
            clock: EconomyClock::new(),
            wallet: Wallet::new(120), // spec: Eco starts day 1 with 120 coins
            estimator: RegimeEstimator::new(),
            world,
            comms: Comms::new(),
            task: None,
            blind_mode,
            rng: StdRng::from_rng(&mut rand::rng()),
        }
    }

    /// Main loop. Call this from the thread the orchestrator's `main.rs` spawns.
    pub fn run(mut self) {
        let _ = self.to_orchestrator.send(ExplorerToOrchestrator::CurrentPlanetResult {
            explorer_id: self.id,
            planet_id: self.current_planet_id,
        });

        while !self.should_stop {
            loop {
                match self.from_orchestrator.try_recv() {
                    Ok(msg) => self.handle_orchestrator_message(msg),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        logging::orchestrator_channel_closed(self.id);
                        self.should_stop = true;
                        break;
                    }
                }
            }

            if self.should_stop {
                break;
            }

            if self.ai_active {
                self.ai_step();
            }

            std::thread::sleep(Duration::from_millis(100));
        }

        logging::shutting_down(self.id);
    }

    // ==================== Orchestrator -> Explorer ====================

    pub(super) fn handle_orchestrator_message(&mut self, msg: OrchestratorToExplorer) {
        match msg {
            OrchestratorToExplorer::KillExplorer => {
                logging::kill_received(self.id);
                self.should_stop = true;
                let _ = self.to_orchestrator.send(
                    ExplorerToOrchestrator::KillExplorerResult { explorer_id: self.id },
                );
            }

            OrchestratorToExplorer::StartExplorerAI => {
                self.ai_active = true;
                let _ = self.to_orchestrator.send(
                    ExplorerToOrchestrator::StartExplorerAIResult { explorer_id: self.id },
                );
            }

            OrchestratorToExplorer::StopExplorerAI => {
                self.ai_active = false;
                let _ = self.to_orchestrator.send(
                    ExplorerToOrchestrator::StopExplorerAIResult { explorer_id: self.id },
                );
            }

            OrchestratorToExplorer::ResetExplorerAI => {
                self.ai_active = false;
                self.world.visited = std::collections::HashSet::from([self.current_planet_id]);
                // Note: deliberately NOT resetting clock/wallet/task here —
                // those are Eco's persistent economic life, not AI-loop
                // state. Swap this comment out if a reset should mean
                // "start the economy over" too.
                let _ = self.to_orchestrator.send(
                    ExplorerToOrchestrator::ResetExplorerAIResult { explorer_id: self.id },
                );
            }

            OrchestratorToExplorer::MoveToPlanet { sender_to_new_planet, planet_id } => {
                match sender_to_new_planet {
                    Some(new_to_planet) => {
                        logging::moved_to_planet(self.id, planet_id);
                        self.planet_link.to_planet = Some(new_to_planet);
                        if let Some(rx) = &self.planet_link.from_planet {
                            while rx.try_recv().is_ok() {}
                        }
                        self.current_planet_id = planet_id;
                        self.world.visited.insert(planet_id);
                    }
                    None => {
                        logging::move_rejected(self.id, planet_id);
                    }
                }

                self.comms.resolve_travel();

                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::MovedToPlanetResult {
                    explorer_id: self.id,
                    planet_id: self.current_planet_id,
                });
            }

            OrchestratorToExplorer::CurrentPlanetRequest => {
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::CurrentPlanetResult {
                    explorer_id: self.id,
                    planet_id: self.current_planet_id,
                });
            }

            OrchestratorToExplorer::SupportedResourceRequest => {
                let supported_resources = self.query_supported_resources().unwrap_or_default();
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::SupportedResourceResult {
                    explorer_id: self.id,
                    supported_resources,
                });
            }

            OrchestratorToExplorer::SupportedCombinationRequest => {
                let combination_list = self.query_supported_combinations().unwrap_or_default();
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::SupportedCombinationResult {
                    explorer_id: self.id,
                    combination_list,
                });
            }

            OrchestratorToExplorer::GenerateResourceRequest { to_generate } => {
                let generated = self.request_generate_resource(to_generate);
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::GenerateResourceResponse {
                    explorer_id: self.id,
                    generated,
                });
            }

            OrchestratorToExplorer::CombineResourceRequest { to_generate } => {
                let generated = self.request_combine_resource(to_generate);
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::CombineResourceResponse {
                    explorer_id: self.id,
                    generated,
                });
            }

            OrchestratorToExplorer::NeighborsResponse { neighbors } => {
                self.world.record_neighbors(self.current_planet_id, neighbors);
                self.comms.resolve_neighbors();
            }


            OrchestratorToExplorer::BagContentRequest => {
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::BagContentResponse {
                    explorer_id: self.id,
                    bag_content: self.bag.contents(),
                });
            }

            _ => {
                logging::unknown_message(self.id);
            }
        }
    }

    // ==================== Explorer -> Planet ====================



    /// Builds the `ComplexResourceRequest` for a target type by pulling
    /// matching ingredient resources out of the bag. Puts back whatever
    /// it already took if the full recipe isn't available, so nothing is
    /// lost on failure. (Unchanged from the skeleton.)


    /// The ONE place Eco talks to a planet. Sends `msg`, then waits up to
    /// PLANET_REPLY_TIMEOUT for the reply that answers it.
    fn request_planet(&self, msg: ExplorerToPlanet) -> Result<PlanetToExplorer, PlanetError> {
        let (Some(tx), Some(rx)) = (&self.planet_link.to_planet, &self.planet_link.from_planet)
        else {
            logging::no_planet_link(self.id);
            return Err(PlanetError::NoLink);
        };

        // Anything already waiting is a stale reply to an older request.
        while rx.try_recv().is_ok() {}

        let wanted = reply_kind(ExplorerToPlanetKind::from(&msg));
        if tx.send(msg).is_err() {
            logging::planet_channel_gone(self.id);
            return Err(PlanetError::Gone);
        }

        let deadline = Instant::now() + PLANET_REPLY_TIMEOUT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(PlanetToExplorer::Stopped) => {
                    logging::planet_stopped(self.id);
                    return Err(PlanetError::Stopped);
                }
                Ok(reply) if PlanetToExplorerKind::from(&reply) == wanted => return Ok(reply),
                Ok(_) => log::debug!("explorer {}: discarded a reply that answers no pending request", self.id),
                Err(RecvTimeoutError::Timeout) => {
                    log::warn!("explorer {}: planet did not reply in time", self.id);
                    return Err(PlanetError::NoReply);
                }
                Err(RecvTimeoutError::Disconnected) => {
                    logging::planet_channel_gone(self.id);
                    return Err(PlanetError::Gone);
                }
            }
        }
    }

    /// Asks the current planet what it can generate. Records the answer
    /// (even an empty set: that is a real answer) only on success.
    pub(super) fn query_supported_resources(
        &mut self,
    ) -> Result<std::collections::HashSet<BasicResourceType>, PlanetError> {
        let reply = self.request_planet(ExplorerToPlanet::SupportedResourceRequest { explorer_id: self.id })?;
        let PlanetToExplorer::SupportedResourceResponse { resource_list } = reply else {
            return Err(PlanetError::NoReply);
        };
        self.world.record_resources(self.current_planet_id, resource_list.clone());
        Ok(resource_list)
    }

    pub(super) fn query_supported_combinations(
        &mut self,
    ) -> Result<std::collections::HashSet<ComplexResourceType>, PlanetError> {
        let reply = self.request_planet(ExplorerToPlanet::SupportedCombinationRequest { explorer_id: self.id })?;
        let PlanetToExplorer::SupportedCombinationResponse { combination_list } = reply else {
            return Err(PlanetError::NoReply);
        };
        self.world.record_combos(self.current_planet_id, combination_list.clone());
        Ok(combination_list)
    }

    fn request_generate_resource(&mut self, resource: BasicResourceType) -> Result<(), String> {
        let reply = self
            .request_planet(ExplorerToPlanet::GenerateResourceRequest { explorer_id: self.id, resource })
            .map_err(|e| e.describe().to_string())?;
        match reply {
            PlanetToExplorer::GenerateResourceResponse { resource: Some(r) } => {
                self.bag.put_back(GenericResource::BasicResources(r));
                Ok(())
            }
            PlanetToExplorer::GenerateResourceResponse { resource: None } => {
                Err("planet could not generate that resource".to_string())
            }
            _ => Err("unexpected reply from planet".to_string()),
        }
    }

    fn request_combine_resource(&mut self, to_generate: ComplexResourceType) -> Result<(), String> {
        if !self.planet_link.is_connected() {
            return Err(PlanetError::NoLink.describe().to_string());
        }

        let msg = self.build_combine_request(to_generate)?;

        let reply = self
            .request_planet(ExplorerToPlanet::CombineResourceRequest { explorer_id: self.id, msg })
            .map_err(|e| e.describe().to_string())?;
        match reply {
            PlanetToExplorer::CombineResourceResponse { complex_response: Ok(complex_resource) } => {
                self.bag.put_back(GenericResource::ComplexResources(complex_resource));
                Ok(())
            }
            PlanetToExplorer::CombineResourceResponse { complex_response: Err((reason, r1, r2)) } => {
                self.bag.put_back(r1);
                self.bag.put_back(r2);
                Err(reason)
            }
            _ => Err("unexpected reply from planet".to_string()),
        }
    }

    /// "Energy Cell Availability" — explorer-initiated only, no
    /// orchestrator involvement in the real protocol.
    pub fn query_available_energy_cells(&self) -> Option<ID> {
        match self.request_planet(ExplorerToPlanet::AvailableEnergyCellRequest { explorer_id: self.id }) {
            Ok(PlanetToExplorer::AvailableEnergyCellResponse { available_cells }) => Some(available_cells),
            _ => None,
        }
    }

    fn build_combine_request(&mut self, to_generate: ComplexResourceType) -> Result<ComplexResourceRequest, String> {
        let result = match to_generate {
            ComplexResourceType::Water => {
                match (self.bag.take_basic(BasicResourceType::Hydrogen), self.bag.take_basic(BasicResourceType::Oxygen)) {
                    (Some(h), Some(o)) => Ok((h, o)),
                    (h, o) => Err((h, o, "missing Hydrogen and/or Oxygen in bag".to_string())),
                }
                    .and_then(|(h, o)| {
                        let h = h.to_hydrogen().map_err(|e| (None, None, e))?;
                        let o = o.to_oxygen().map_err(|e| (None, None, e))?;
                        Ok(ComplexResourceRequest::Water(h, o))
                    })
            }
            ComplexResourceType::Diamond => {
                match (self.bag.take_basic(BasicResourceType::Carbon), self.bag.take_basic(BasicResourceType::Carbon)) {
                    (Some(c1), Some(c2)) => Ok((c1, c2)),
                    (c1, c2) => Err((c1, c2, "missing two Carbon in bag".to_string())),
                }
                    .and_then(|(c1, c2)| {
                        let c1 = c1.to_carbon().map_err(|e| (None, None, e))?;
                        let c2 = c2.to_carbon().map_err(|e| (None, None, e))?;
                        Ok(ComplexResourceRequest::Diamond(c1, c2))
                    })
            }
            ComplexResourceType::Life => {
                match (self.bag.take_complex(ComplexResourceType::Water), self.bag.take_basic(BasicResourceType::Carbon)) {
                    (Some(w), Some(c)) => Ok((w, c)),
                    (w, c) => Err((w, c, "missing Water and/or Carbon in bag".to_string())),
                }
                    .and_then(|(w, c)| {
                        let w = w.to_water().map_err(|e| (None, None, e))?;
                        let c = c.to_carbon().map_err(|e| (None, None, e))?;
                        Ok(ComplexResourceRequest::Life(w, c))
                    })
            }
            ComplexResourceType::Robot => {
                match (self.bag.take_basic(BasicResourceType::Silicon), self.bag.take_complex(ComplexResourceType::Life)) {
                    (Some(s), Some(l)) => Ok((s, l)),
                    (s, l) => Err((s, l, "missing Silicon and/or Life in bag".to_string())),
                }
                    .and_then(|(s, l)| {
                        let s = s.to_silicon().map_err(|e| (None, None, e))?;
                        let l = l.to_life().map_err(|e| (None, None, e))?;
                        Ok(ComplexResourceRequest::Robot(s, l))
                    })
            }
            ComplexResourceType::Dolphin => {
                match (self.bag.take_complex(ComplexResourceType::Water), self.bag.take_complex(ComplexResourceType::Life)) {
                    (Some(w), Some(l)) => Ok((w, l)),
                    (w, l) => Err((w, l, "missing Water and/or Life in bag".to_string())),
                }
                    .and_then(|(w, l)| {
                        let w = w.to_water().map_err(|e| (None, None, e))?;
                        let l = l.to_life().map_err(|e| (None, None, e))?;
                        Ok(ComplexResourceRequest::Dolphin(w, l))
                    })
            }
            ComplexResourceType::AIPartner => {
                match (self.bag.take_complex(ComplexResourceType::Robot), self.bag.take_complex(ComplexResourceType::Diamond)) {
                    (Some(r), Some(d)) => Ok((r, d)),
                    (r, d) => Err((r, d, "missing Robot and/or Diamond in bag".to_string())),
                }
                    .and_then(|(r, d)| {
                        let r = r.to_robot().map_err(|e| (None, None, e))?;
                        let d = d.to_diamond().map_err(|e| (None, None, e))?;
                        Ok(ComplexResourceRequest::AIPartner(r, d))
                    })
            }
        };

        result.map_err(|(a, b, msg): (Option<GenericResource>, Option<GenericResource>, String)| {
            if let Some(a) = a {
                self.bag.put_back(a);
            }
            if let Some(b) = b {
                self.bag.put_back(b);
            }
            msg
        })
    }

    /// Learns what the current planet supports, once per planet. Free: no
    /// charge, no clock tick, no orchestrator message. A failed probe records
    /// nothing and only delays the next probe; it never blocks other actions.
    fn discover_current_planet(&mut self) {
        let here = self.current_planet_id;
        let need_resources = !self.world.resources.contains_key(&here);
        let need_combos = !self.world.combos.contains_key(&here);
        if !(need_resources || need_combos) || !self.comms.may_probe(Instant::now()) {
            return;
        }

        if need_resources && self.query_supported_resources().is_err() {
            self.comms.probe_failed(Instant::now());
            return; // planet not answering: don't also ask about combinations
        }
        if need_combos && self.query_supported_combinations().is_err() {
            self.comms.probe_failed(Instant::now());
        }
    }


    // ==================== Autonomous AI: economy + planning ====================

    pub(super) fn ai_step(&mut self) {
        if let Some((ended_regime, len)) = self.clock.tick(&mut self.rng) {
            self.estimator.observe(ended_regime, len);
            logging::regime_ended(self.id, ended_regime, len, self.clock.regime);
        }

        if self.clock.take_income_due() {
            let income = self.clock.costs().daily_income;
            self.wallet.credit(income);
            logging::income_credited(self.id, self.clock.day, income, self.wallet.coins);
        }

        let Some(target) = self.task else {
            let new_task = ALL_COMPLEX_RESOURCES[self.rng.random_range(0..ALL_COMPLEX_RESOURCES.len())];
            logging::task_assigned(self.id, new_task);
            self.task = Some(new_task);
            return;
        };

        if self.bag.complex_set().contains(&target) {
            self.complete_task(target);
            return;
        }

        // Waiting on the orchestrator (or cooling down after a failure) only
        // blocks choosing a NEW action. The clock tick and income above
        // already ran, so nothing here touches the economy.
        if !self.comms.may_act(Instant::now()) {
            return;
        }

        self.discover_current_planet();

        let forecast = if self.blind_mode {
            ForecastMode::Blind {
                estimator: &self.estimator,
                days_spent_in_regime: self.clock.days_spent_in_regime(),
            }
        } else {
            ForecastMode::Oracle { days_left_in_regime: self.clock.days_left_in_regime }
        };

        let action = Planner::next_action(
            target,
            self.current_planet_id,
            &self.world,
            &self.bag.basic_counts(),
            &self.bag.complex_set(),
            self.clock.regime,
            self.clock.costs(),
            &forecast,
        );

        self.execute(action);
    }

    pub(super) fn execute(&mut self, action: Action) {
        let costs = self.clock.costs();
        match action {
            Action::Stay => {
                self.wallet.charge(costs.stay);
            }
            Action::Move(dst) | Action::ExploreTowards(dst) => {
                self.wallet.charge(costs.mv);   // moves in Step 6
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::TravelToPlanetRequest {
                    explorer_id: self.id,
                    current_planet_id: self.current_planet_id,
                    dst_planet_id: dst,
                });
                self.comms.start_travel(dst, Instant::now());
            }
            Action::Mine(resource) => {
                self.wallet.charge(costs.mine);
                if let Err(e) = self.request_generate_resource(resource) {
                    logging::mine_failed(self.id, resource, &e);
                    self.comms.cool_down(Instant::now());
                }
            }
            Action::Combine(target) => {
                self.wallet.charge(costs.combine);
                if let Err(e) = self.request_combine_resource(target) {
                    logging::combine_failed(self.id, target, &e);
                    self.comms.cool_down(Instant::now());
                }
            }
            Action::RequestNeighbors => {
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::NeighborsRequest {
                    explorer_id: self.id,
                    current_planet_id: self.current_planet_id,
                });
                self.comms.start_neighbors(self.current_planet_id, Instant::now());
            }
        }

        if self.wallet.is_in_debt() {
            logging::in_debt(self.id, self.wallet.coins);
            // Soft constraint only, per design: debt doesn't block acting.
            // A daily income top-up (or the completion bonus) will bring
            // the balance back up; refusing to act here would just stall
            // the task without helping either objective.
        }
    }


    pub(super) fn complete_task(&mut self, target: ComplexResourceType) {
        let bonus = (self.wallet.coins.max(0) / 2) as u32;
        self.wallet.credit(bonus);
        logging::task_completed(self.id, target, bonus, self.wallet.coins);
        self.task = None;
    }
}