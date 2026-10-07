//! Static knowledge of how complex resources are built. Mirrors
//! `Explorer::build_combine_request` exactly — if that ever changes,
//! update `recipe_of` to match.

use common_game::components::resource::{BasicResourceType, ComplexResourceType};

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

/// Flattened build plan for a target: the basic resources needed
/// (leaves) and the combine steps to run, in dependency order (each
/// step's ingredients are guaranteed ready by the time it's reached).
#[derive(Debug, Clone)]
pub struct BuildPlan {
    pub basics_needed: Vec<BasicResourceType>,
    pub combine_steps: Vec<ComplexResourceType>,
}

pub fn build_plan_for(target: ComplexResourceType) -> BuildPlan {
    let mut basics = Vec::new();
    let mut steps = Vec::new();
    flatten(target, &mut basics, &mut steps);
    BuildPlan { basics_needed: basics, combine_steps: steps }
}

fn flatten(
    target: ComplexResourceType,
    basics: &mut Vec<BasicResourceType>,
    steps: &mut Vec<ComplexResourceType>,
) {
    let (a, b) = recipe_of(target);
    for ingredient in [a, b] {
        match ingredient {
            Ingredient::Basic(bt) => basics.push(bt),
            Ingredient::Complex(ct) => flatten(ct, basics, steps),
        }
    }
    steps.push(target);
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