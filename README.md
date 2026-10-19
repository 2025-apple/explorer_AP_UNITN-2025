# Eco: an economy-aware Explorer

Eco is an Explorer for the Advanced Programming galaxy project (crate `explorer_eco`). It runs inside any orchestrator that follows the common protocol, with no dependency on a particular orchestrator or GUI.

## Idea

Eco is given a target complex resource (a "task") and has to build it by mining basic resources and combining them across the planets of the galaxy. On top of that it lives in a small **economy of its own design**: every action costs coins, and the prices change with a cycle of regimes.

- **Time.** Eco keeps its own clock. One decision cycle runs every 100 ms, 50 cycles make a day (5 s). The clock does not depend on the orchestrator or on how slowly a planet answers; waiting for a reply never advances it.
- **Regimes.** Neutral (5 days), then Flourish, then Recession, repeating. Each regime has its own prices for move, stay, mine and combine, and its own daily income.
- **Wallet.** Eco starts with 18 coins. Mining and combining are charged only when they succeed; a move is charged when the orchestrator accepts it. Waiting and asking for information are free. There is no completion bonus.
- **Affordability.** Eco only starts a paid action it can afford. If it cannot, it waits for income instead of acting.

## How Eco decides (`src/eco/`)

| Module | Role |
|---|---|
| `explorer.rs` | the main loop (`run`, `ai_step`), message handling, charging |
| `planner.rs` | picks the next action: combine if ready, otherwise mine or walk to a planet that supplies what is missing; may choose to wait during a Recession when waiting is cheaper |
| `recipes.rs` | which basic and complex resources a task still needs |
| `world.rs` | what Eco has learned: neighbours, resources, combinations, visited planets; paths are found by breadth-first search |
| `comms.rs` | pending requests, timeouts and cooldowns, so a silent orchestrator or planet never blocks Eco |
| `regime.rs`, `time.rs`, `wallet.rs` | price tables, the clock, and the coin balance |
| `status.rs` | a read-only snapshot of Eco's state for an observer (see below) |
| `logging.rs` | logs every message to and from the orchestrator and planets as `LogEvent`s, as the course spec requires |

If the current task becomes impossible (for example no reachable planet supplies a needed resource), Eco swaps it for one the known galaxy can deliver.

## Observing Eco

`create_explorer(...)` builds the Explorer. `Explorer::with_status_sink` accepts a shared cell that Eco fills with an `EcoStatus` (day, regime, coins, earned, spent, task, last decision and its cost). A GUI can read this without Eco knowing anything about the GUI.

## Tests

```
cargo test
```

Unit tests cover the economy, planner, recipes, world model, communication and the explorer loop. Integration tests in `tests/` run Eco against real `Planet` instances, including building a resource across three planets, and check that messages are logged in the standard format.

## Using it

Used from an orchestrator by adding it as a dependency and building it with the four channels of the protocol:

```toml
explorer_eco = { git = "https://github.com/<org>/explorer_eco.git", branch = "master" }
```

The GUI demo runs Eco inside `Bipolar-Orchestrator_eco_accommodated`, a copy of the Bipolar orchestrator and GUI adapted to show Eco's status panel.
