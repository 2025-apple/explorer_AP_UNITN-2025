use common_game::components::resource::ComplexResourceType;
use common_game::protocols::orchestrator_explorer::{ExplorerToOrchestrator, OrchestratorToExplorer};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::eco::explorer::{BagContent, Explorer};
use crate::eco::planner::Action;

struct Rig {
    explorer: Explorer,
    from_explorer: Receiver<ExplorerToOrchestrator<BagContent>>,
    _to_explorer: Sender<OrchestratorToExplorer>,
    _planet_rx: Receiver<ExplorerToPlanet>,
    _planet_tx: Sender<PlanetToExplorer>,
}

fn rig() -> Rig {
    let (to_explorer, rx_orch) = unbounded::<OrchestratorToExplorer>();
    let (tx_orch, from_explorer) = unbounded::<ExplorerToOrchestrator<BagContent>>();
    let (tx_planet, planet_rx) = unbounded::<ExplorerToPlanet>();
    let (planet_tx, rx_planet) = unbounded::<PlanetToExplorer>();
    let explorer = Explorer::new(1, 100, rx_orch, tx_orch, rx_planet, tx_planet, true);
    Rig {
        explorer,
        from_explorer,
        _to_explorer: to_explorer,
        _planet_rx: planet_rx,
        _planet_tx: planet_tx,
    }
}

#[test]
fn first_step_assigns_task_without_charging() {
    let mut r = rig();
    r.explorer.ai_step();
    assert!(r.explorer.task.is_some());
    assert_eq!(r.explorer.wallet.coins, 120);
    assert_eq!(r.explorer.clock.cycle_in_day, 1); // the step still ticked once
}

#[test]
fn five_steps_make_one_day_and_credit_income() {
    let mut r = rig();
    for _ in 0..5 {
        r.explorer.ai_step();
    }
    // With an empty world the planner only asks for neighbors (free),
    // so the only wallet change is one Neutral day of income.
    assert_eq!(r.explorer.clock.day, 2);
    assert_eq!(r.explorer.wallet.coins, 120 + 120);
}

#[test]
fn stay_charges_current_price_and_neighbor_request_is_free() {
    let mut r = rig();
    r.explorer.execute(Action::Stay);
    assert_eq!(r.explorer.wallet.coins, 120 - 4); // Neutral stay
    r.explorer.execute(Action::RequestNeighbors);
    assert_eq!(r.explorer.wallet.coins, 120 - 4);
    assert!(matches!(
        r.from_explorer.try_recv(),
        Ok(ExplorerToOrchestrator::NeighborsRequest { .. })
    ));
}

#[test]
fn completing_a_task_pays_half_the_balance() {
    let mut r = rig();
    r.explorer.wallet.coins = 101;
    r.explorer.task = Some(ComplexResourceType::Water);
    r.explorer.complete_task(ComplexResourceType::Water);
    assert_eq!(r.explorer.wallet.coins, 101 + 50); // integer half
    assert!(r.explorer.task.is_none());
}

#[test]
fn completing_a_task_in_debt_pays_nothing() {
    let mut r = rig();
    r.explorer.wallet.coins = -10;
    r.explorer.complete_task(ComplexResourceType::Water);
    assert_eq!(r.explorer.wallet.coins, -10);
}

#[test]
fn a_silent_orchestrator_gets_one_neighbors_request_not_one_per_step() {
    let mut r = rig();
    for _ in 0..10 {
        r.explorer.ai_step();
    }
    let sent = r
        .from_explorer
        .try_iter()
        .filter(|m| matches!(m, ExplorerToOrchestrator::NeighborsRequest { .. }))
        .count();
    assert_eq!(sent, 1);
}

#[test]
fn waiting_for_a_reply_changes_neither_the_clock_nor_the_wallet() {
    // Same arithmetic as the 5-step test, over two days while a request is pending.
    let mut r = rig();
    for _ in 0..10 {
        r.explorer.ai_step();
    }
    assert_eq!(r.explorer.clock.day, 3);
    assert_eq!(r.explorer.clock.cycle_in_day, 0);
    assert_eq!(r.explorer.wallet.coins, 120 + 120 + 120); // two Neutral incomes, no charges
}

#[test]
fn a_neighbors_response_clears_the_pending_request() {
    let mut r = rig();
    r.explorer.execute(Action::RequestNeighbors);
    assert!(matches!(r.explorer.comms.pending, crate::eco::comms::Pending::Neighbors { .. }));
    r.explorer
        .handle_orchestrator_message(OrchestratorToExplorer::NeighborsResponse { neighbors: vec![] });
    assert_eq!(r.explorer.comms.pending, crate::eco::comms::Pending::Idle);
}

#[test]
fn a_move_to_planet_clears_a_pending_travel() {
    let mut r = rig();
    r.explorer.execute(Action::Move(7));
    assert!(matches!(r.explorer.comms.pending, crate::eco::comms::Pending::Travel { .. }));
    let (tx, _rx) = unbounded::<ExplorerToPlanet>();
    r.explorer.handle_orchestrator_message(OrchestratorToExplorer::MoveToPlanet {
        sender_to_new_planet: Some(tx),
        planet_id: 7,
    });
    assert_eq!(r.explorer.comms.pending, crate::eco::comms::Pending::Idle);
    assert_eq!(r.explorer.current_planet_id, 7);
}

#[test]
fn a_failed_mine_starts_a_cooldown() {
    // The rig's planet never answers, so the mine fails after the 2 s reply timeout.
    let mut r = rig();
    r.explorer
        .execute(Action::Mine(common_game::components::resource::BasicResourceType::Carbon));
    assert!(!r.explorer.comms.may_act(std::time::Instant::now()));
}