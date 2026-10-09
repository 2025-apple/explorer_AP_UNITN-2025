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