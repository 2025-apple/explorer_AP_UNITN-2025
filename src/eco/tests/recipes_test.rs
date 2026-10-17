use std::collections::HashMap;

use common_game::components::resource::{BasicResourceType as B, ComplexResourceType as C};

use crate::eco::recipes::{
    ALL_COMPLEX_RESOURCES, Remaining, is_step_ready, remaining_work, task_pool,
};

fn count<T: PartialEq>(v: &[T], x: T) -> usize {
    v.iter().filter(|e| **e == x).count()
}

#[test]
fn dolphin_from_nothing_needs_two_waters_worth_of_basics() {
    let r = remaining_work(C::Dolphin, &HashMap::new(), &HashMap::new());
    assert_eq!(count(&r.basics_to_mine, B::Hydrogen), 2);
    assert_eq!(count(&r.basics_to_mine, B::Oxygen), 2);
    assert_eq!(count(&r.basics_to_mine, B::Carbon), 1);
    assert_eq!(
        r.steps_to_run,
        vec![C::Water, C::Water, C::Life, C::Dolphin]
    );
}

#[test]
fn a_held_water_is_used_not_rebuilt() {
    let r = remaining_work(C::Dolphin, &HashMap::new(), &HashMap::from([(C::Water, 1)]));
    assert_eq!(count(&r.basics_to_mine, B::Hydrogen), 1);
    assert_eq!(count(&r.basics_to_mine, B::Oxygen), 1);
    assert_eq!(r.steps_to_run, vec![C::Water, C::Life, C::Dolphin]);
}

#[test]
fn after_building_water_only_carbon_is_missing_for_life() {
    let r = remaining_work(C::Life, &HashMap::new(), &HashMap::from([(C::Water, 1)]));
    assert_eq!(r.basics_to_mine, vec![B::Carbon]);
    assert_eq!(r.steps_to_run, vec![C::Life]);
}

#[test]
fn diamond_needs_two_carbon() {
    let none = remaining_work(C::Diamond, &HashMap::new(), &HashMap::new());
    assert_eq!(none.basics_to_mine, vec![B::Carbon, B::Carbon]);
    let one = remaining_work(
        C::Diamond,
        &HashMap::from([(B::Carbon, 1)]),
        &HashMap::new(),
    );
    assert_eq!(one.basics_to_mine, vec![B::Carbon]);
}

#[test]
fn a_held_target_leaves_nothing_to_do() {
    let r = remaining_work(C::Water, &HashMap::new(), &HashMap::from([(C::Water, 1)]));
    assert_eq!(r, Remaining::default());
}

#[test]
fn readiness_counts_duplicates() {
    let one = HashMap::from([(B::Carbon, 1)]);
    let two = HashMap::from([(B::Carbon, 2)]);
    assert!(!is_step_ready(C::Diamond, &one, &HashMap::new()));
    assert!(is_step_ready(C::Diamond, &two, &HashMap::new()));
}

#[test]
fn readiness_needs_complex_ingredients_too() {
    let carbon = HashMap::from([(B::Carbon, 1)]);
    assert!(!is_step_ready(C::Life, &carbon, &HashMap::new()));
    assert!(is_step_ready(
        C::Life,
        &carbon,
        &HashMap::from([(C::Water, 1)])
    ));
}

#[test]
fn a_new_task_is_never_a_type_already_held() {
    let held = HashMap::from([(C::Water, 1), (C::Diamond, 2)]);
    let pool = task_pool(&held);
    assert!(!pool.contains(&C::Water));
    assert!(!pool.contains(&C::Diamond));
    assert_eq!(pool.len(), 4);
}

#[test]
fn with_an_empty_bag_every_type_is_a_possible_task() {
    assert_eq!(task_pool(&HashMap::new()).len(), 6);
}

#[test]
fn if_everything_is_held_any_type_is_allowed_again() {
    let held: HashMap<C, u32> = ALL_COMPLEX_RESOURCES.iter().map(|&t| (t, 1)).collect();
    assert_eq!(task_pool(&held).len(), 6);
}
