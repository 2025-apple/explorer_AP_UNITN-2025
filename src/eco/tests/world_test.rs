use std::collections::HashSet;

use common_game::components::resource::BasicResourceType;

use crate::eco::world::WorldModel;

/// 1 - 2 - 3, with Carbon on planet 3; planets 1 and 2 visited.
fn line_world() -> WorldModel {
    let mut w = WorldModel::default();
    w.record_neighbors(1, vec![2]);
    w.record_neighbors(2, vec![1, 3]);
    w.record_neighbors(3, vec![2]);
    w.record_resources(3, HashSet::from([BasicResourceType::Carbon]));
    w.visited.extend([1, 2]);
    w
}

#[test]
fn forgetting_a_planet_removes_it_from_every_table() {
    let mut w = line_world();
    w.forget_planet(3);
    assert!(!w.neighbors.contains_key(&3));
    assert_eq!(w.neighbors.get(&2), Some(&vec![1]));
    assert!(!w.resources.contains_key(&3));
}

#[test]
fn a_refresh_missing_a_planet_means_it_was_destroyed() {
    let mut w = line_world();
    w.record_neighbors(2, vec![1]); // 3 is no longer listed
    assert!(!w.neighbors.contains_key(&3));
    assert!(!w.resources.contains_key(&3));
    assert_eq!(w.neighbors.get(&2), Some(&vec![1]));
}

#[test]
fn a_refresh_after_invalidation_is_still_compared_with_the_old_list() {
    let mut w = line_world();
    w.invalidate_neighbors(2);
    assert!(!w.neighbors.contains_key(&2)); // planner will see "unknown" and ask
    w.record_neighbors(2, vec![1]);
    assert!(!w.resources.contains_key(&3));
    assert!(w.stale_neighbors.is_empty());
}

#[test]
fn a_refresh_that_only_adds_planets_forgets_nothing() {
    let mut w = line_world();
    w.record_neighbors(2, vec![1, 3, 4]);
    assert!(w.resources.contains_key(&3));
    assert_eq!(w.neighbors.get(&2), Some(&vec![1, 3, 4]));
}

#[test]
fn paths_never_go_through_a_forgotten_planet() {
    let mut w = line_world();
    assert_eq!(w.nearest_known_source(1, BasicResourceType::Carbon), Some(vec![1, 2, 3]));
    w.forget_planet(2);
    assert_eq!(w.nearest_known_source(1, BasicResourceType::Carbon), None);
}

#[test]
fn a_visited_planet_whose_exits_were_never_asked_is_still_worth_exploring() {
    let mut w = WorldModel::default();
    w.record_neighbors(1, vec![2]); // planet 2 was visited, but its exits are unknown
    w.visited.extend([1, 2]);
    assert_eq!(w.nearest_unexplored(1), Some(vec![1, 2]));

    w.record_neighbors(2, vec![1]); // now everything is known and visited
    assert_eq!(w.nearest_unexplored(1), None);
}