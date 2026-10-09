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
}