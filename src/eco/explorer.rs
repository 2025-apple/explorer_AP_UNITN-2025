//! The `Explorer` struct itself: protocol handling (unchanged from the
//! skeleton) plus the wiring that drives Eco's private economy and
//! planner every `ai_step`.

use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use rand::rngs::StdRng;
use rand::{Rng, RngExt, SeedableRng};

use common_game::components::resource::{
    BasicResourceType, ComplexResourceRequest, ComplexResourceType, GenericResource,
};
use common_game::protocols::orchestrator_explorer::{
    ExplorerToOrchestrator, OrchestratorToExplorer,
};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use common_game::utils::ID;

use super::bag::Bag;
use super::logging;
use super::planner::{Action, Planner};
use super::recipes::ALL_COMPLEX_RESOURCES;
use super::regime::{ForecastMode, RegimeEstimator};
use super::time::EconomyClock;
use super::wallet::Wallet;
use super::world::WorldModel;

#[must_use]
pub fn create_explorer(
    explorer_id: ID,
    starting_planet_id: ID,
    rx_orchestrator: Receiver<OrchestratorToExplorer>,
    tx_orchestrator: Sender<ExplorerToOrchestrator<GenericResource>>,
) -> Explorer {
    // Blind mode by default — the realistic, harder AI behavior. Use
    // `create_explorer_oracle` for a baseline/testing build.
    Explorer::new(explorer_id, starting_planet_id, rx_orchestrator, tx_orchestrator, true)
}

#[must_use]
pub fn create_explorer_oracle(
    explorer_id: ID,
    starting_planet_id: ID,
    rx_orchestrator: Receiver<OrchestratorToExplorer>,
    tx_orchestrator: Sender<ExplorerToOrchestrator<GenericResource>>,
) -> Explorer {
    Explorer::new(explorer_id, starting_planet_id, rx_orchestrator, tx_orchestrator, false)
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

pub struct Explorer {
    id: ID,

    from_orchestrator: Receiver<OrchestratorToExplorer>,
    to_orchestrator: Sender<ExplorerToOrchestrator<GenericResource>>,

    planet_link: PlanetLink,
    current_planet_id: ID,

    bag: Bag,
    ai_active: bool,
    should_stop: bool,

    // ---- Eco's own economy + planning state (no protocol involvement) ----
    clock: EconomyClock,
    wallet: Wallet,
    estimator: RegimeEstimator,
    world: WorldModel,
    task: Option<ComplexResourceType>,
    blind_mode: bool,
    rng: StdRng,
}

impl Explorer {
    fn new(
        id: ID,
        starting_planet_id: ID,
        from_orchestrator: Receiver<OrchestratorToExplorer>,
        to_orchestrator: Sender<ExplorerToOrchestrator<GenericResource>>,
        blind_mode: bool,
    ) -> Self {
        let mut world = WorldModel::default();
        world.visited.insert(starting_planet_id);

        Self {
            id,
            from_orchestrator,
            to_orchestrator,
            planet_link: PlanetLink::default(),
            current_planet_id: starting_planet_id,
            bag: Bag::default(),
            ai_active: false,
            should_stop: false,
            clock: EconomyClock::new(),
            wallet: Wallet::new(120), // spec: Eco starts day 1 with 120 coins
            estimator: RegimeEstimator::new(),
            world,
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

    fn handle_orchestrator_message(&mut self, msg: OrchestratorToExplorer) {
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
                        self.planet_link = PlanetLink {
                            to_planet: Some(new_to_planet),
                            from_planet: None, // see module docs: orchestrator never delivers this
                        };
                        self.current_planet_id = planet_id;
                        self.world.visited.insert(planet_id);
                    }
                    None => {
                        logging::move_rejected(self.id, planet_id);
                    }
                }

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
                let supported_resources = self.query_supported_resources();
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::SupportedResourceResult {
                    explorer_id: self.id,
                    supported_resources,
                });
            }

            OrchestratorToExplorer::SupportedCombinationRequest => {
                let combination_list = self.query_supported_combinations();
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
            }

            _ => {
                logging::unknown_message(self.id);
            }
        }
    }

    // ==================== Explorer -> Planet ====================

    fn query_supported_resources(&mut self) -> std::collections::HashSet<BasicResourceType> {
        let Some(tx) = &self.planet_link.to_planet else {
            logging::no_planet_link(self.id);
            return std::collections::HashSet::new();
        };
        if tx.send(ExplorerToPlanet::SupportedResourceRequest { explorer_id: self.id }).is_err() {
            logging::planet_channel_gone(self.id);
            return std::collections::HashSet::new();
        }
        let result = match self.await_planet_response() {
            Some(PlanetToExplorer::SupportedResourceResponse { resource_list }) => resource_list,
            Some(PlanetToExplorer::Stopped) => {
                logging::planet_stopped(self.id);
                std::collections::HashSet::new()
            }
            _ => std::collections::HashSet::new(),
        };
        self.world.record_resources(self.current_planet_id, result.clone());
        result
    }

    fn query_supported_combinations(&mut self) -> std::collections::HashSet<ComplexResourceType> {
        let Some(tx) = &self.planet_link.to_planet else {
            return std::collections::HashSet::new();
        };
        if tx.send(ExplorerToPlanet::SupportedCombinationRequest { explorer_id: self.id }).is_err() {
            return std::collections::HashSet::new();
        }
        let result = match self.await_planet_response() {
            Some(PlanetToExplorer::SupportedCombinationResponse { combination_list }) => combination_list,
            _ => std::collections::HashSet::new(),
        };
        self.world.record_combos(self.current_planet_id, result.clone());
        result
    }

    fn request_generate_resource(&mut self, resource: BasicResourceType) -> Result<(), String> {
        let Some(tx) = &self.planet_link.to_planet else {
            return Err("not currently linked to a planet".to_string());
        };
        if tx
            .send(ExplorerToPlanet::GenerateResourceRequest { explorer_id: self.id, resource })
            .is_err()
        {
            return Err("planet unreachable".to_string());
        }

        match self.await_planet_response() {
            Some(PlanetToExplorer::GenerateResourceResponse { resource: Some(r) }) => {
                self.bag.put_back(GenericResource::BasicResources(r));
                Ok(())
            }
            Some(PlanetToExplorer::GenerateResourceResponse { resource: None }) => {
                Err("planet could not generate that resource".to_string())
            }
            Some(PlanetToExplorer::Stopped) => Err("planet is stopped".to_string()),
            _ => Err("no response from planet".to_string()),
        }
    }

    /// Builds the `ComplexResourceRequest` for a target type by pulling
    /// matching ingredient resources out of the bag. Puts back whatever
    /// it already took if the full recipe isn't available, so nothing is
    /// lost on failure. (Unchanged from the skeleton.)
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

    fn request_combine_resource(&mut self, to_generate: ComplexResourceType) -> Result<(), String> {
        if !self.planet_link.is_connected() {
            return Err("not currently linked to a planet".to_string());
        }

        let msg = self.build_combine_request(to_generate)?;

        let tx = self.planet_link.to_planet.as_ref().unwrap();
        if tx.send(ExplorerToPlanet::CombineResourceRequest { explorer_id: self.id, msg }).is_err() {
            return Err("planet unreachable".to_string());
        }

        match self.await_planet_response() {
            Some(PlanetToExplorer::CombineResourceResponse { complex_response: Ok(complex_resource) }) => {
                self.bag.put_back(GenericResource::ComplexResources(complex_resource));
                Ok(())
            }
            Some(PlanetToExplorer::CombineResourceResponse { complex_response: Err((reason, r1, r2)) }) => {
                self.bag.put_back(r1);
                self.bag.put_back(r2);
                Err(reason)
            }
            Some(PlanetToExplorer::Stopped) => Err("planet is stopped".to_string()),
            _ => Err("no response from planet".to_string()),
        }
    }

    /// "Energy Cell Availability" — explorer-initiated only, no
    /// orchestrator involvement in the real protocol.
    pub fn query_available_energy_cells(&self) -> Option<ID> {
        let tx = self.planet_link.to_planet.as_ref()?;
        if tx.send(ExplorerToPlanet::AvailableEnergyCellRequest { explorer_id: self.id }).is_err() {
            return None;
        }
        match self.await_planet_response() {
            Some(PlanetToExplorer::AvailableEnergyCellResponse { available_cells }) => Some(available_cells),
            _ => None,
        }
    }

    /// Blocks briefly for a single reply from the current planet.
    /// Returns `None` on timeout.
    fn await_planet_response(&self) -> Option<PlanetToExplorer> {
        let rx = self.planet_link.from_planet.as_ref()?;
        rx.recv_timeout(Duration::from_secs(2)).ok()
    }

    // ==================== Autonomous AI: economy + planning ====================

    fn ai_step(&mut self) {
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

    fn execute(&mut self, action: Action) {
        let costs = self.clock.costs();
        match action {
            Action::Stay => {
                self.wallet.charge(costs.stay);
            }
            Action::Move(dst) | Action::ExploreTowards(dst) => {
                self.wallet.charge(costs.mv);
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::TravelToPlanetRequest {
                    explorer_id: self.id,
                    current_planet_id: self.current_planet_id,
                    dst_planet_id: dst,
                });
            }
            Action::Mine(resource) => {
                self.wallet.charge(costs.mine);
                if let Err(e) = self.request_generate_resource(resource) {
                    logging::mine_failed(self.id, resource, &e);
                }
            }
            Action::Combine(target) => {
                self.wallet.charge(costs.combine);
                if let Err(e) = self.request_combine_resource(target) {
                    logging::combine_failed(self.id, target, &e);
                }
            }
            Action::RequestNeighbors => {
                let _ = self.to_orchestrator.send(ExplorerToOrchestrator::NeighborsRequest {
                    explorer_id: self.id,
                    current_planet_id: self.current_planet_id,
                });
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

    fn complete_task(&mut self, target: ComplexResourceType) {
        let bonus = (self.wallet.coins.max(0) / 2) as u32;
        self.wallet.credit(bonus);
        logging::task_completed(self.id, target, bonus, self.wallet.coins);
        self.task = None;
    }
}