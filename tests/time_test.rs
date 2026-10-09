use explorer_eco::eco::time::{EconomyClock, EconomyRegime};
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

    // The fifth tick rolls over to the next day.
    assert!(c.tick(&mut rng).is_none());

    assert_eq!(c.day, 2);
    assert_eq!(c.cycle_in_day, 0);

    assert!(c.take_income_due());
    assert!(!c.take_income_due());
}

#[test]
fn neutral_lasts_five_days_then_flourish() {
    let mut rng = StdRng::seed_from_u64(2);
    let mut c = EconomyClock::new();

    for tick in 1..=24 {
        assert!(
            c.tick(&mut rng).is_none(),
            "unexpected transition at tick {tick}"
        );
    }

    assert_eq!(c.regime, EconomyRegime::Neutral);

    // Tick 25 ends the five-day Neutral regime.
    assert_eq!(
        c.tick(&mut rng),
        Some((EconomyRegime::Neutral, 5))
    );

    assert_eq!(c.regime, EconomyRegime::Flourish);

    // Flourish duration: Eco's chosen range, not a spec requirement.
    assert!((3..=8).contains(&c.days_left_in_regime));
}

#[test]
fn regimes_cycle_in_order_with_lengths_in_range() {
    for seed in 0..20 {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut c = EconomyClock::new();
        let mut ended = Vec::new();

        for _ in 0..1000 {
            if let Some(transition) = c.tick(&mut rng) {
                ended.push(transition);
            }

            if ended.len() == 4 {
                break;
            }
        }

        assert_eq!(ended.len(), 4, "seed {seed}");

        // Neutral -> Flourish -> Recession -> Neutral
        assert_eq!(ended[0], (EconomyRegime::Neutral, 5));

        assert_eq!(ended[1].0, EconomyRegime::Flourish);
        assert!(
            (3..=8).contains(&ended[1].1),
            "invalid Flourish duration for seed {seed}: {}",
            ended[1].1
        );

        assert_eq!(ended[2].0, EconomyRegime::Recession);
        assert!(
            (2..=6).contains(&ended[2].1),
            "invalid Recession duration for seed {seed}: {}",
            ended[2].1
        );

        assert_eq!(ended[3], (EconomyRegime::Neutral, 5));
    }
}