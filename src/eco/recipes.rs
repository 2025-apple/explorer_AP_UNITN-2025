//! Static knowledge of how complex resources are built. Mirrors
//! `Explorer::build_combine_request` exactly — if that ever changes,
//! update `recipe_of` to match.

use common_game::components::resource::{BasicResourceType, ComplexResourceType};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ingredient {
    Basic(BasicResourceType),
    Complex(ComplexResourceType),
}

pub fn recipe_of(target: ComplexResourceType) -> (Ingredient, Ingredient) {
    use BasicResourceType::*;
    use ComplexResourceType::*;
    match target {
        Water => (Ingredient::Basic(Hydrogen), Ingredient::Basic(Oxygen)),
        Diamond => (Ingredient::Basic(Carbon), Ingredient::Basic(Carbon)),
        Life => (Ingredient::Complex(Water), Ingredient::Basic(Carbon)),
        Robot => (Ingredient::Basic(Silicon), Ingredient::Complex(Life)),
        Dolphin => (Ingredient::Complex(Water), Ingredient::Complex(Life)),
        AIPartner => (Ingredient::Complex(Robot), Ingredient::Complex(Diamond)),
    }
}

/// What is still missing to produce `target`, given the bag.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Remaining {
    /// Basics still to mine (one entry per unit).
    pub basics_to_mine: Vec<BasicResourceType>,
    /// Combines still to run, in dependency order.
    pub steps_to_run: Vec<ComplexResourceType>,
}

pub fn remaining_work(
    target: ComplexResourceType,
    bag_basics: &HashMap<BasicResourceType, u32>,
    bag_complex: &HashMap<ComplexResourceType, u32>,
) -> Remaining {
    let mut basics = bag_basics.clone();
    let mut complex = bag_complex.clone();
    let mut out = Remaining::default();
    allocate(target, &mut basics, &mut complex, &mut out);
    out
}

/// Top-down: use a held unit if there is one, otherwise plan to build it.
fn allocate(
    target: ComplexResourceType,
    basics: &mut HashMap<BasicResourceType, u32>,
    complex: &mut HashMap<ComplexResourceType, u32>,
    out: &mut Remaining,
) {
    if let Some(n) = complex.get_mut(&target) {
        if *n > 0 {
            *n -= 1;
            return;
        }
    }
    let (a, b) = recipe_of(target);
    for ingredient in [a, b] {
        match ingredient {
            Ingredient::Basic(bt) => match basics.get_mut(&bt) {
                Some(n) if *n > 0 => *n -= 1,
                _ => out.basics_to_mine.push(bt),
            },
            Ingredient::Complex(ct) => allocate(ct, basics, complex, out),
        }
    }
    out.steps_to_run.push(target);
}

/// True if the bag holds every ingredient of `step`, counting duplicates
/// (Diamond needs two Carbon).
pub fn is_step_ready(
    step: ComplexResourceType,
    bag_basics: &HashMap<BasicResourceType, u32>,
    bag_complex: &HashMap<ComplexResourceType, u32>,
) -> bool {
    let (a, b) = recipe_of(step);
    let mut need_basic: HashMap<BasicResourceType, u32> = HashMap::new();
    let mut need_complex: HashMap<ComplexResourceType, u32> = HashMap::new();
    for ingredient in [a, b] {
        match ingredient {
            Ingredient::Basic(bt) => *need_basic.entry(bt).or_insert(0) += 1,
            Ingredient::Complex(ct) => *need_complex.entry(ct).or_insert(0) += 1,
        }
    }
    need_basic
        .iter()
        .all(|(bt, n)| bag_basics.get(bt).copied().unwrap_or(0) >= *n)
        && need_complex
            .iter()
            .all(|(ct, n)| bag_complex.get(ct).copied().unwrap_or(0) >= *n)
}

/// The six buildable complex resources — used for picking a random task.
pub const ALL_COMPLEX_RESOURCES: [ComplexResourceType; 6] = [
    ComplexResourceType::Water,
    ComplexResourceType::Diamond,
    ComplexResourceType::Life,
    ComplexResourceType::Robot,
    ComplexResourceType::Dolphin,
    ComplexResourceType::AIPartner,
];

/// Tasks Eco may be given next: the types it does not already hold.
/// If it holds all six, any type is allowed.
pub fn task_pool(held: &HashMap<ComplexResourceType, u32>) -> Vec<ComplexResourceType> {
    let pool: Vec<_> = ALL_COMPLEX_RESOURCES
        .iter()
        .copied()
        .filter(|t| held.get(t).copied().unwrap_or(0) == 0)
        .collect();
    if pool.is_empty() {
        ALL_COMPLEX_RESOURCES.to_vec()
    } else {
        pool
    }
}
