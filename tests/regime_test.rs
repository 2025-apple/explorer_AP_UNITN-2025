use explorer_eco::eco::regime::EconomyRegime;
use explorer_eco::eco::time::CYCLES_PER_DAY;

fn row(r: EconomyRegime) -> (u32, u32, u32, u32, u32) {
    let c = r.costs();
    (c.mv, c.stay, c.mine, c.combine, c.daily_income)
}

#[test]
fn price_tables_match_design() {
    // (move, stay, mine, combine, daily income)
    assert_eq!(row(EconomyRegime::Neutral), (10, 4, 2, 4, 120));
    assert_eq!(row(EconomyRegime::Flourish), (8, 2, 1, 3, 200));
    assert_eq!(row(EconomyRegime::Recession), (12, 7, 5, 6, 50));
}

#[test]
fn regimes_cycle_neutral_flourish_recession() {
    assert_eq!(EconomyRegime::Neutral.next(), EconomyRegime::Flourish);
    assert_eq!(EconomyRegime::Flourish.next(), EconomyRegime::Recession);
    assert_eq!(EconomyRegime::Recession.next(), EconomyRegime::Neutral);
}

#[test]
fn a_day_of_cheapest_actions_is_always_affordable() {
    // Five cheapest actions cost less than one day's income.
    for r in [
        EconomyRegime::Neutral,
        EconomyRegime::Flourish,
        EconomyRegime::Recession,
    ] {
        let c = r.costs();
        let cheapest = c.mv.min(c.stay).min(c.mine).min(c.combine);

        assert!(
            cheapest * u32::from(CYCLES_PER_DAY) < c.daily_income,
            "{r:?}: cheapest day costs more than income"
        );
    }
}