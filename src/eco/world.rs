//! What Eco has learned about the planet graph so far. Populated lazily
//! from `NeighborsResponse` / resource / combo query results — never
//! assumed known up front.

use std::collections::{HashMap, HashSet, VecDeque};

use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use common_game::utils::ID;

#[derive(Debug, Default)]
pub struct WorldModel {
    pub neighbors: HashMap<ID, Vec<ID>>,
    pub resources: HashMap<ID, HashSet<BasicResourceType>>,
    pub combos: HashMap<ID, HashSet<ComplexResourceType>>,
    pub visited: HashSet<ID>,
}

impl WorldModel {
    pub fn record_neighbors(&mut self, planet: ID, neighbors: Vec<ID>) {
        self.neighbors.insert(planet, neighbors);
    }

    pub fn record_resources(&mut self, planet: ID, resources: HashSet<BasicResourceType>) {
        self.resources.insert(planet, resources);
    }

    pub fn record_combos(&mut self, planet: ID, combos: HashSet<ComplexResourceType>) {
        self.combos.insert(planet, combos);
    }

    /// Shortest known hop path (inclusive of destination) to the nearest
    /// planet known to support `resource`. `None` if no known planet has
    /// it yet — caller should fall back to exploration.
    pub fn nearest_known_source(&self, from: ID, resource: BasicResourceType) -> Option<Vec<ID>> {
        self.bfs_to(from, |p| self.resources.get(&p).is_some_and(|s| s.contains(&resource)))
    }

    pub fn nearest_known_combo_planet(&self, from: ID, combo: ComplexResourceType) -> Option<Vec<ID>> {
        self.bfs_to(from, |p| self.combos.get(&p).is_some_and(|s| s.contains(&combo)))
    }

    /// Nearest planet we haven't visited yet — used when the shopping
    /// list needs something no known planet has, so Eco has to explore.
    pub fn nearest_unvisited(&self, from: ID) -> Option<Vec<ID>> {
        self.bfs_to(from, |p| p != from && !self.visited.contains(&p))
    }

    fn bfs_to(&self, from: ID, goal: impl Fn(ID) -> bool) -> Option<Vec<ID>> {
        if goal(from) {
            return Some(vec![from]);
        }
        let mut queue = VecDeque::new();
        let mut came_from: HashMap<ID, ID> = HashMap::new();
        let mut seen = HashSet::new();
        queue.push_back(from);
        seen.insert(from);

        while let Some(cur) = queue.pop_front() {
            let Some(neighbors) = self.neighbors.get(&cur) else { continue };
            for &n in neighbors {
                if seen.insert(n) {
                    came_from.insert(n, cur);
                    if goal(n) {
                        let mut path = vec![n];
                        let mut walk = n;
                        while let Some(&p) = came_from.get(&walk) {
                            path.push(p);
                            walk = p;
                        }
                        path.reverse();
                        return Some(path);
                    }
                    queue.push_back(n);
                }
            }
        }
        None
    }
}