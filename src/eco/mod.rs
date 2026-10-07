//! Eco: an autonomous `Explorer` that owns its own private economy
//! simulation, layered directly on top of the orchestrator/planet
//! protocol. Nothing here adds new protocol messages — the economy
//! (coins, days, cycles, regimes) is entirely Eco's own bookkeeping.
//!
//! Module map:
//!   time      - day/cycle clock, regime-duration RNG draws
//!               ("how time works")
//!   regime    - price tables + Neutral/Flourish/Recession transition
//!               order, plus the blind-mode duration estimator
//!               ("how the economy switches")
//!   wallet    - coin balance
//!   recipes   - static recipe-tree knowledge (what builds what)
//!   world     - learned planet graph (neighbors/resources/combos)
//!   bag       - Eco's resource inventory
//!   planner   - turns (task, bag, world, economy) into one action
//!   logging   - every log line Eco emits, in one place
//!   explorer  - the `Explorer` struct itself + protocol wiring
//!
//! VERIFY against the real `common_game` crate before compiling:
//!   - `BasicResourceType` / `ComplexResourceType` are `Copy + Eq + Hash`.
//!   - `BasicResource::get_type()` / `ComplexResource::get_type()` exist.
//!   - The six recipes in `recipes::recipe_of` match your current
//!     `build_combine_request` exactly.

mod bag;
mod explorer;
mod logging;
mod planner;
mod recipes;
mod regime;
mod time;
mod wallet;
mod world;

pub use explorer::{create_explorer, create_explorer_oracle, Explorer};