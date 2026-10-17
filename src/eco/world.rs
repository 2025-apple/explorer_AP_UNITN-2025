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
    /// Exit lists that were thrown away to force a refresh. Kept so the next
    /// fresh list can still be compared with the old one.
    pub stale_neighbors: HashMap<ID, Vec<ID>>,
}

impl WorldModel {
    /// Stores a fresh exit list. Any planet the previous list had and this one
    /// lacks was destroyed (the topology only ever loses planets), so it is
    /// forgotten everywhere.
    pub fn record_neighbors(&mut self, planet: ID, neighbors: Vec<ID>) {
        let old = self
            .neighbors
            .remove(&planet)
            .or_else(|| self.stale_neighbors.remove(&planet));
        self.stale_neighbors.remove(&planet);
        if let Some(old) = old {
            for gone in old {
                if !neighbors.contains(&gone) {
                    self.forget_planet(gone);
                }
            }
        }
        self.neighbors.insert(planet, neighbors);
    }

    /// Makes Eco ask for `planet`'s exits again, keeping the old list only as
    /// a baseline for spotting destroyed planets.
    pub fn invalidate_neighbors(&mut self, planet: ID) {
        if let Some(list) = self.neighbors.remove(&planet) {
            self.stale_neighbors.insert(planet, list);
        }
    }

    /// Removes a (destroyed) planet from every table and every exit list.
    pub fn forget_planet(&mut self, planet: ID) {
        self.neighbors.remove(&planet);
        self.stale_neighbors.remove(&planet);
        for list in self
            .neighbors
            .values_mut()
            .chain(self.stale_neighbors.values_mut())
        {
            list.retain(|&p| p != planet);
        }
        self.resources.remove(&planet);
        self.combos.remove(&planet);
        self.visited.remove(&planet);
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
        self.bfs_to(from, |p| {
            self.resources
                .get(&p)
                .is_some_and(|s| s.contains(&resource))
        })
    }

    pub fn nearest_known_combo_planet(
        &self,
        from: ID,
        combo: ComplexResourceType,
    ) -> Option<Vec<ID>> {
        self.bfs_to(from, |p| {
            self.combos.get(&p).is_some_and(|s| s.contains(&combo))
        })
    }

    /// Nearest planet worth going to when the shopping list needs something no
    /// known planet has: one we never visited, or one we visited but whose exits
    /// we never asked for (what lies beyond it is still unexplored).
    pub fn nearest_unexplored(&self, from: ID) -> Option<Vec<ID>> {
        self.bfs_to(from, |p| {
            p != from && (!self.visited.contains(&p) || !self.neighbors.contains_key(&p))
        })
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
            let Some(neighbors) = self.neighbors.get(&cur) else {
                continue;
            };
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
