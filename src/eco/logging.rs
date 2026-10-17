//! Every log line Eco emits, defined once here, so wording stays
//! consistent and the call sites in `explorer.rs` stay short.

use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::utils::ID;

use super::planner::Action;
use super::regime::EconomyRegime;

use common_game::logging::{ActorType, Channel, EventType, LogEvent, Participant, Payload};
use common_game::protocols::orchestrator_explorer::{
    ExplorerToOrchestrator, OrchestratorToExplorer,
};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};

use super::explorer::BagContent;

/// The orchestrator has no id of its own; the common crate suggests 0.
const ORCHESTRATOR_ID: ID = 0;

/// Emits one message event in the format the working group standardised
/// (spec §4.2): who sent it, who received it, what kind, and the message text.
fn log_message(from: Participant, to: Participant, kind: EventType, msg: &dyn std::fmt::Debug) {
    let mut payload = Payload::new();
    payload.insert("message".to_string(), format!("{msg:?}"));
    LogEvent::new(Some(from), Some(to), kind, Channel::Trace, payload).emit();
}

pub(crate) fn received_from_orchestrator(id: ID, msg: &OrchestratorToExplorer) {
    log_message(
        Participant::new(ActorType::Orchestrator, ORCHESTRATOR_ID),
        Participant::new(ActorType::Explorer, id),
        EventType::MessageOrchestratorToExplorer,
        msg,
    );
}

pub(crate) fn sent_to_orchestrator(id: ID, msg: &ExplorerToOrchestrator<BagContent>) {
    log_message(
        Participant::new(ActorType::Explorer, id),
        Participant::new(ActorType::Orchestrator, ORCHESTRATOR_ID),
        EventType::MessageExplorerToOrchestrator,
        msg,
    );
}

pub(crate) fn sent_to_planet(id: ID, planet_id: ID, msg: &ExplorerToPlanet) {
    log_message(
        Participant::new(ActorType::Explorer, id),
        Participant::new(ActorType::Planet, planet_id),
        EventType::MessageExplorerToPlanet,
        msg,
    );
}

pub(crate) fn received_from_planet(id: ID, planet_id: ID, msg: &PlanetToExplorer) {
    log_message(
        Participant::new(ActorType::Planet, planet_id),
        Participant::new(ActorType::Explorer, id),
        EventType::MessagePlanetToExplorer,
        msg,
    );
}

// ---- Protocol / lifecycle ----

pub(crate) fn orchestrator_channel_closed(id: ID) {
    log::warn!("Explorer {id}: orchestrator channel closed");
}

pub(crate) fn shutting_down(id: ID) {
    log::info!("Explorer {id} shutting down");
}

pub(crate) fn kill_received(id: ID) {
    log::info!("Explorer {id} received KillExplorer");
}

pub(crate) fn moved_to_planet(id: ID, planet_id: ID) {
    log::info!("Explorer {id}: moved to planet {planet_id}");
}

pub(crate) fn move_rejected(id: ID, planet_id: ID) {
    log::warn!("Explorer {id}: move to planet {planet_id} was rejected");
}

pub(crate) fn no_planet_link(id: ID) {
    log::debug!("Explorer {id}: no planet link yet");
}

pub(crate) fn planet_channel_gone(id: ID) {
    log::warn!("Explorer {id}: planet channel gone");
}

pub(crate) fn planet_stopped(id: ID) {
    log::debug!("Explorer {id}: planet is stopped");
}

// ---- Economy ----
pub(crate) fn task_abandoned(id: ID, old: ComplexResourceType, new: ComplexResourceType) {
    log::info!("Explorer {id}: task {old:?} can no longer be delivered, switching to {new:?}");
}
pub(crate) fn regime_ended(id: ID, ended: EconomyRegime, days: u32, new_regime: EconomyRegime) {
    log::info!("Explorer {id}: regime {ended:?} ended after {days} days, entering {new_regime:?}");
}

pub(crate) fn income_credited(id: ID, day: u32, amount: u32, total: i64) {
    log::info!("Explorer {id}: day {day} income +{amount} -> {total} coins");
}

pub(crate) fn task_assigned(id: ID, target: ComplexResourceType) {
    log::info!("Explorer {id}: assigned new task {target:?}");
}

pub(crate) fn task_completed(id: ID, target: ComplexResourceType, total: i64) {
    log::info!("Explorer {id}: completed task {target:?} ({total} coins)");
}

pub(crate) fn in_debt(id: ID, coins: i64) {
    log::warn!("Explorer {id}: in debt, {coins} coins");
}

pub(crate) fn mine_failed(id: ID, resource: BasicResourceType, reason: &str) {
    log::debug!("Explorer {id}: mine {resource:?} failed: {reason}");
}

pub(crate) fn combine_failed(id: ID, target: ComplexResourceType, reason: &str) {
    log::debug!("Explorer {id}: combine {target:?} failed: {reason}");
}

pub(crate) fn action_taken(id: ID, planet_id: ID, action: &Action, coins: i64) {
    log::info!("Explorer {id}: on planet {planet_id} -> {action:?} ({coins} coins)");
}

pub(crate) fn waiting_for_income(id: ID, action: &Action, coins: i64) {
    log::info!("Explorer {id}: cannot afford {action:?} with {coins} coins, waiting for income");
}
