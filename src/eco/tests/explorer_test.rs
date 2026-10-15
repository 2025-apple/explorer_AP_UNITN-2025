use common_game::components::resource::ComplexResourceType;
use common_game::protocols::orchestrator_explorer::{ExplorerToOrchestrator, OrchestratorToExplorer};
use common_game::protocols::planet_explorer::{ExplorerToPlanet, PlanetToExplorer};
use crossbeam_channel::{unbounded, Receiver, Sender};

use crate::eco::explorer::{BagContent, Explorer};
use crate::eco::planner::Action;

use crate::eco::explorer::STARTING_COINS;
use crate::eco::regime::EconomyRegime;
use crate::eco::time::CYCLES_PER_DAY;
use common_game::components::resource::BasicResourceType as B;
use std::collections::HashSet;


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
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS);
    assert_eq!(r.explorer.clock.cycle_in_day, 1); // the step still ticked once
}

#[test]
fn five_steps_make_one_day_and_credit_income() {
    let mut r = rig();
    for _ in 0..CYCLES_PER_DAY {
        r.explorer.ai_step();
    }
    let n = EconomyRegime::Neutral.costs();
    // With an empty world the planner only asks for neighbors (free),
    // so the only wallet change is one Neutral day of income.
    assert_eq!(r.explorer.clock.day, 2);

    assert_eq!(r.explorer.wallet.coins, STARTING_COINS + i64::from(n.daily_income));
}

#[test]
fn stay_charges_current_price_and_neighbor_request_is_free() {
    let mut r = rig();
    r.explorer.execute(Action::Stay);

    let n = EconomyRegime::Neutral.costs();
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS - i64::from(n.stay));

    r.explorer.execute(Action::RequestNeighbors);
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS - i64::from(n.stay));
    assert!(matches!(
        r.from_explorer.try_recv(),
        Ok(ExplorerToOrchestrator::NeighborsRequest { .. })
    ));
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
    for _ in 0..(2*CYCLES_PER_DAY) {
        r.explorer.ai_step();
    }
    let n = EconomyRegime::Neutral.costs();
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS + 2 * i64::from(n.daily_income));
    assert_eq!(r.explorer.clock.day, 3);
    assert_eq!(r.explorer.clock.cycle_in_day, 0);
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
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS); // a failed mine costs nothing
}

fn accept_move(r: &mut Rig, to: u32) {
    let (tx, _rx) = unbounded::<ExplorerToPlanet>();
    r.explorer.handle_orchestrator_message(OrchestratorToExplorer::MoveToPlanet {
        sender_to_new_planet: Some(tx),
        planet_id: to,
    });
}

fn refuse_move(r: &mut Rig, to: u32) {
    r.explorer.handle_orchestrator_message(OrchestratorToExplorer::MoveToPlanet {
        sender_to_new_planet: None,
        planet_id: to,
    });
}

#[test]
fn requesting_a_move_is_free() {
    let mut r = rig();
    r.explorer.execute(Action::Move(7));
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS);
    assert!(matches!(
        r.from_explorer.try_recv(),
        Ok(ExplorerToOrchestrator::TravelToPlanetRequest { dst_planet_id: 7, .. })
    ));
}

#[test]
fn an_accepted_move_is_charged_once_at_the_current_price() {
    let n = EconomyRegime::Neutral.costs();
    let mut r = rig();
    r.explorer.execute(Action::Move(7));
    accept_move(&mut r, 7);
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS - i64::from(n.mv)); // Neutral move price
    assert_eq!(r.explorer.current_planet_id, 7);
    accept_move(&mut r, 7); // "moving" to where we already are
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS - i64::from(n.mv)); // not charged again
}

#[test]
fn a_move_is_paid_at_the_price_in_force_when_it_is_accepted() {
    let rec = EconomyRegime::Recession.costs();
    let mut r = rig();
    r.explorer.execute(Action::Move(7)); // requested during Neutral
    r.explorer.clock.regime = EconomyRegime::Recession;
    accept_move(&mut r, 7);
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS - i64::from(rec.mv)); // Recession move price
}

#[test]
fn a_rejected_move_is_free_and_starts_a_cooldown() {
    let mut r = rig();
    r.explorer.execute(Action::Move(7));
    refuse_move(&mut r, 7);
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS);
    assert_eq!(r.explorer.current_planet_id, 100);
    assert_eq!(r.explorer.comms.pending, crate::eco::comms::Pending::Idle);
    assert!(!r.explorer.comms.may_act(std::time::Instant::now()));
}

#[test]
fn a_refusal_about_another_planet_leaves_the_pending_request_alone() {
    let mut r = rig();
    r.explorer.execute(Action::Move(7));
    refuse_move(&mut r, 9);
    assert!(matches!(r.explorer.comms.pending, crate::eco::comms::Pending::Travel { to: 7, .. }));
}

#[test]
fn a_move_started_by_the_orchestrator_is_charged_too() {
    let n = EconomyRegime::Neutral.costs();
    let mut r = rig();
    accept_move(&mut r, 9);
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS - i64::from(n.mv));
    assert_eq!(r.explorer.current_planet_id, 9);
    assert!(r.explorer.world.visited.contains(&9));
}

#[test]
fn neighbors_are_recorded_for_the_planet_that_was_asked_not_the_one_we_are_on() {
    let mut r = rig();
    r.explorer.execute(Action::RequestNeighbors); // asked while on planet 100
    accept_move(&mut r, 7); // the orchestrator moved us before it answered
    r.explorer
        .handle_orchestrator_message(OrchestratorToExplorer::NeighborsResponse { neighbors: vec![1, 2] });
    assert_eq!(r.explorer.world.neighbors.get(&100), Some(&vec![1, 2]));
    assert!(!r.explorer.world.neighbors.contains_key(&7));
}

#[test]
fn an_unsolicited_neighbors_response_is_ignored() {
    let mut r = rig();
    r.explorer
        .handle_orchestrator_message(OrchestratorToExplorer::NeighborsResponse { neighbors: vec![1, 2] });
    assert!(r.explorer.world.neighbors.is_empty());
}

#[test]
fn a_refused_move_makes_eco_ask_for_fresh_neighbors() {
    let mut r = rig();
    r.explorer.world.record_neighbors(100, vec![7]);
    r.explorer.execute(Action::Move(7));
    refuse_move(&mut r, 7);
    assert!(!r.explorer.world.neighbors.contains_key(&100));
    assert!(r.explorer.world.stale_neighbors.contains_key(&100));
}

#[test]
fn a_destination_missing_from_the_refreshed_list_is_forgotten() {
    let mut r = rig();
    r.explorer.world.record_neighbors(100, vec![7, 8]);
    r.explorer.world.record_resources(
        7,
        std::collections::HashSet::from([common_game::components::resource::BasicResourceType::Carbon]),
    );
    r.explorer.execute(Action::Move(7));
    refuse_move(&mut r, 7);
    r.explorer.execute(Action::RequestNeighbors); // exits unknown again, so this asks
    r.explorer
        .handle_orchestrator_message(OrchestratorToExplorer::NeighborsResponse { neighbors: vec![8] });
    assert_eq!(r.explorer.world.neighbors.get(&100), Some(&vec![8]));
    assert!(!r.explorer.world.resources.contains_key(&7));
}

#[test]
fn a_travel_request_that_times_out_invalidates_the_exits() {
    let mut r = rig();
    r.explorer.task = Some(ComplexResourceType::Water);
    r.explorer.world.record_neighbors(100, vec![7]);
    let long_ago = std::time::Instant::now()
        .checked_sub(crate::eco::comms::PENDING_TIMEOUT + std::time::Duration::from_secs(1))
        .unwrap();
    r.explorer.comms.pending = crate::eco::comms::Pending::Travel { to: 7, since: long_ago };
    r.explorer.ai_step();
    assert!(!r.explorer.world.neighbors.contains_key(&100));
    assert_eq!(r.explorer.comms.pending, crate::eco::comms::Pending::Idle);
}

#[test]
fn an_exhausted_frontier_idles_for_free_instead_of_asking_again() {
    let n = EconomyRegime::Neutral.costs();
    let mut r = rig();
    r.explorer.task = Some(ComplexResourceType::Water);
    // Everything known about planet 100, and nowhere left to explore.
    r.explorer.world.record_resources(100, std::collections::HashSet::new());
    r.explorer.world.record_combos(100, std::collections::HashSet::new());
    r.explorer.world.record_neighbors(100, vec![]);
    for _ in 0..CYCLES_PER_DAY {
        r.explorer.ai_step();
    }
    assert!(r.from_explorer.try_recv().is_err()); // nothing was sent
    assert_eq!(r.explorer.wallet.coins, STARTING_COINS + i64::from(n.daily_income)); // one day of income, no charges
    assert_eq!(r.explorer.comms.pending, crate::eco::comms::Pending::Idle);
}

#[test]
fn reset_forgets_the_galaxy_but_keeps_ecos_own_life() {
    let mut r = rig();
    r.explorer.task = Some(ComplexResourceType::Water);
    r.explorer.wallet.coins = 77;
    r.explorer.clock.day = 4;
    r.explorer.world.record_neighbors(100, vec![7]);
    r.explorer.world.record_resources(100, std::collections::HashSet::new());
    r.explorer.execute(Action::Move(7)); // leaves a travel request pending

    r.explorer.handle_orchestrator_message(OrchestratorToExplorer::ResetExplorerAI);

    assert!(r.explorer.ai_active);
    assert_eq!(r.explorer.task, Some(ComplexResourceType::Water));
    assert_eq!(r.explorer.wallet.coins, 77);
    assert_eq!(r.explorer.clock.day, 4);
    assert!(r.explorer.world.neighbors.is_empty());
    assert!(r.explorer.world.resources.is_empty());
    assert_eq!(r.explorer.world.visited, std::collections::HashSet::from([100]));
    assert_eq!(r.explorer.comms.pending, crate::eco::comms::Pending::Idle);
    assert!(r.explorer.comms.may_act(std::time::Instant::now()));
    assert!(r
        .from_explorer
        .try_iter()
        .any(|m| matches!(m, ExplorerToOrchestrator::ResetExplorerAIResult { .. })));
}

#[test]
fn reset_restarts_a_stopped_ai_and_leaves_a_running_one_running() {
    let mut r = rig();
    assert!(!r.explorer.ai_active); // manual mode at start
    r.explorer.handle_orchestrator_message(OrchestratorToExplorer::ResetExplorerAI);
    assert!(r.explorer.ai_active);
    r.explorer.handle_orchestrator_message(OrchestratorToExplorer::ResetExplorerAI);
    assert!(r.explorer.ai_active);
}

#[test]
fn stop_enters_manual_mode_and_drops_pending_requests() {
    let mut r = rig();
    r.explorer.handle_orchestrator_message(OrchestratorToExplorer::StartExplorerAI);
    assert!(r.explorer.ai_active);
    r.explorer.execute(Action::RequestNeighbors); // pending neighbors request
    r.explorer.handle_orchestrator_message(OrchestratorToExplorer::StopExplorerAI);
    assert!(!r.explorer.ai_active);
    assert_eq!(r.explorer.comms.pending, crate::eco::comms::Pending::Idle);
    assert!(r
        .from_explorer
        .try_iter()
        .any(|m| matches!(m, ExplorerToOrchestrator::StopExplorerAIResult { .. })));
}

#[test]
fn an_impossible_task_is_swapped_for_one_the_galaxy_can_deliver() {
    use common_game::components::resource::BasicResourceType as B;
    use std::collections::HashSet;
    let mut r = rig();
    r.explorer.task = Some(ComplexResourceType::Robot); // needs Silicon: nobody supplies it
    // The whole known galaxy is planet 100: Carbon, and it can make Diamond.
    r.explorer.world.record_resources(100, HashSet::from([B::Carbon]));
    r.explorer.world.record_combos(100, HashSet::from([ComplexResourceType::Diamond]));
    r.explorer.world.record_neighbors(100, vec![]);
    r.explorer.ai_step();
    assert_eq!(r.explorer.task, Some(ComplexResourceType::Diamond));
}

#[test]
fn a_task_is_kept_while_there_is_still_something_to_explore() {
    use common_game::components::resource::BasicResourceType as B;
    use std::collections::HashSet;
    let mut r = rig();
    r.explorer.task = Some(ComplexResourceType::Robot);
    r.explorer.world.record_resources(100, HashSet::from([B::Carbon]));
    r.explorer.world.record_combos(100, HashSet::from([ComplexResourceType::Diamond]));
    r.explorer.world.record_neighbors(100, vec![5]); // planet 5 was never visited
    r.explorer.ai_step();
    assert_eq!(r.explorer.task, Some(ComplexResourceType::Robot));
}



#[test]
fn can_afford_compares_the_balance_with_the_price_in_force() {
    let mut r = rig();
    let mine = i64::from(EconomyRegime::Neutral.costs().mine);
    r.explorer.wallet.coins = mine - 1;
    assert!(!r.explorer.can_afford(&Action::Mine(B::Carbon)));
    r.explorer.wallet.coins = mine;
    assert!(r.explorer.can_afford(&Action::Mine(B::Carbon)));
    r.explorer.wallet.coins = -50;
    assert!(r.explorer.can_afford(&Action::RequestNeighbors)); // asking is free
}

#[test]
fn a_poor_eco_waits_instead_of_acting() {
    let mut r = rig();
    r.explorer.task = Some(ComplexResourceType::Diamond);
    r.explorer.world.record_resources(100, HashSet::from([B::Carbon]));
    r.explorer.world.record_combos(100, HashSet::from([ComplexResourceType::Diamond]));
    r.explorer.world.record_neighbors(100, vec![]);
    r.explorer.wallet.coins = 0; // the planner wants Mine(Carbon), which costs 4
    r.explorer.ai_step();
    assert_eq!(r.explorer.wallet.coins, 0);
    assert!(r.explorer.waiting_for_income);
    assert!(r._planet_rx.try_recv().is_err()); // the planet was never asked
}

#[test]
fn completing_a_task_pays_nothing_and_frees_eco_for_the_next_one() {
    let mut r = rig();
    r.explorer.wallet.coins = 101;
    r.explorer.task = Some(ComplexResourceType::Water);
    r.explorer.complete_task(ComplexResourceType::Water);
    assert_eq!(r.explorer.wallet.coins, 101); // no reward
    assert!(r.explorer.task.is_none());
}

#[test]
fn status_copies_the_current_state() {
    let mut r = rig();
    r.explorer.task = Some(ComplexResourceType::Water);
    r.explorer.wallet.charge(4);
    r.explorer.wallet.credit(12);
    let s = r.explorer.status();
    assert_eq!(s.coins, STARTING_COINS + 12 - 4);
    assert_eq!(s.earned, 12);
    assert_eq!(s.spent, 4);
    assert_eq!(s.task, Some(ComplexResourceType::Water));
    assert_eq!(s.day, 1);
    assert_eq!(s.regime, EconomyRegime::Neutral);
    assert!(s.bag.is_empty());
    assert!(!s.waiting_for_income);
}

#[test]
fn status_shows_that_a_poor_eco_is_waiting() {
    let mut r = rig();
    r.explorer.task = Some(ComplexResourceType::Diamond);
    r.explorer.world.record_resources(100, HashSet::from([B::Carbon]));
    r.explorer.world.record_combos(100, HashSet::from([ComplexResourceType::Diamond]));
    r.explorer.world.record_neighbors(100, vec![]);
    r.explorer.wallet.coins = 0;
    r.explorer.ai_step();
    assert!(r.explorer.status().waiting_for_income);
}