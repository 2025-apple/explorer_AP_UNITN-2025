//! Every log line Eco emits, defined once here, so wording stays
//! consistent and the call sites in `explorer.rs` stay short.

use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::utils::ID;

use super::regime::EconomyRegime;

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

pub(crate) fn unknown_message(id: ID) {
    log::error!("Explorer {id}: error unknown received message");
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

pub(crate) fn regime_ended(id: ID, ended: EconomyRegime, days: u32, new_regime: EconomyRegime) {
    log::info!("Explorer {id}: regime {ended:?} ended after {days} days, entering {new_regime:?}");
}

pub(crate) fn income_credited(id: ID, day: u32, amount: u32, total: i64) {
    log::info!("Explorer {id}: day {day} income +{amount} -> {total} coins");
}

pub(crate) fn task_assigned(id: ID, target: ComplexResourceType) {
    log::info!("Explorer {id}: assigned new task {target:?}");
}

pub(crate) fn task_completed(id: ID, target: ComplexResourceType, bonus: u32, total: i64) {
    log::info!("Explorer {id}: completed task {target:?}, earned {bonus} bonus coins ({total} total)");
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