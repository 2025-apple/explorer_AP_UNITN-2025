//! A read-only snapshot of Eco's inner state, for a status panel.
//! It is a plain copy: reading it cannot change Eco.

use super::explorer::BagContent;
use super::regime::EconomyRegime;
use common_game::components::resource::ComplexResourceType;
use common_game::utils::ID;

#[derive(Debug, Clone, PartialEq)]
pub struct EcoStatus {
    pub planet: ID,
    pub day: u32,
    pub regime: EconomyRegime,
    pub coins: i64,
    /// Total income received so far.
    pub earned: i64,
    /// Total paid for actions so far.
    pub spent: i64,
    pub task: Option<ComplexResourceType>,
    pub bag: BagContent,
    /// True while Eco holds back an action it cannot pay for yet.
    pub waiting_for_income: bool,
}