use crate::eco::regime::EconomyRegime;
use crate::eco::time::CYCLES_PER_DAY;

fn row(r: EconomyRegime) -> (u32, u32, u32, u32, u32) {
    let c = r.costs();
    (c.mv, c.stay, c.mine, c.combine, c.daily_income)
}


#[test]
fn regimes_cycle_neutral_flourish_recession() {
    assert_eq!(EconomyRegime::Neutral.next(), EconomyRegime::Flourish);
    assert_eq!(EconomyRegime::Flourish.next(), EconomyRegime::Recession);
    assert_eq!(EconomyRegime::Recession.next(), EconomyRegime::Neutral);
}

#[test]
fn mining_and_combining_are_cheapest_in_flourish_and_dearest_in_recession() {
    let n = EconomyRegime::Neutral.costs();
    let f = EconomyRegime::Flourish.costs();
    let r = EconomyRegime::Recession.costs();
    assert!(f.mine < n.mine && n.mine < r.mine);
    assert!(f.combine < n.combine && n.combine < r.combine);
}

#[test]
fn price_tables_match_design() {
    assert_eq!(row(EconomyRegime::Neutral), (4, 4, 4, 4, 12));
    assert_eq!(row(EconomyRegime::Flourish), (5, 2, 1, 3, 20));
    assert_eq!(row(EconomyRegime::Recession), (9, 7, 5, 6, 16));
}

