use rand::rngs::StdRng;
use rand::SeedableRng;

#[test]
fn new_clock_starts_day_one_neutral() {
    let c = EconomyClock::new();
    assert_eq!(c.day, 1);
    assert_eq!(c.cycle_in_day, 0);
    assert_eq!(c.regime, EconomyRegime::Neutral);
    assert_eq!(c.days_left_in_regime, 5);
}

#[test]
fn five_ticks_make_a_day_and_income_fires_once() {
    let mut rng = StdRng::seed_from_u64(1);
    let mut c = EconomyClock::new();
    for _ in 0..4 {
        assert!(c.tick(&mut rng).is_none());
        assert!(!c.take_income_due());
    }
    assert_eq!(c.day, 1);
    assert!(c.tick(&mut rng).is_none()); // 5th tick: day rolls over
    assert_eq!(c.day, 2);
    assert_eq!(c.cycle_in_day, 0);
    assert!(c.take_income_due());
    assert!(!c.take_income_due()); // consumed
}

#[test]
fn neutral_lasts_five_days_then_flourish() {
    let mut rng = StdRng::seed_from_u64(2);
    let mut c = EconomyClock::new();
    for tick in 1..=24 {
        assert!(c.tick(&mut rng).is_none(), "unexpected transition at tick {tick}");
    }
    assert_eq!(c.regime, EconomyRegime::Neutral);
    assert_eq!(c.tick(&mut rng), Some((EconomyRegime::Neutral, 5))); // tick 25
    assert_eq!(c.regime, EconomyRegime::Flourish);
    assert!((FLOURISH_DAYS_RANGE.0..=FLOURISH_DAYS_RANGE.1).contains(&c.days_left_in_regime));
}

#[test]
fn regimes_cycle_in_order_with_lengths_in_range() {
    for seed in 0..20 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut c = EconomyClock::new();
        let mut ended = Vec::new();
        for _ in 0..1000 {
            if let Some(e) = c.tick(&mut rng) {
                ended.push(e);
            }
            if ended.len() == 4 {
                break;
            }
        }
        assert_eq!(ended.len(), 4, "seed {seed}");
        assert_eq!(ended[0], (EconomyRegime::Neutral, 5));
        assert_eq!(ended[1].0, EconomyRegime::Flourish);
        assert!((FLOURISH_DAYS_RANGE.0..=FLOURISH_DAYS_RANGE.1).contains(&ended[1].1));
        assert_eq!(ended[2].0, EconomyRegime::Recession);
        assert!((RECESSION_DAYS_RANGE.0..=RECESSION_DAYS_RANGE.1).contains(&ended[2].1));
        assert_eq!(ended[3], (EconomyRegime::Neutral, 5));
    }
}