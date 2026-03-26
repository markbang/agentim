# Autoresearch: AgentIM usable multi-agent IM bridge

## Objective
Turn this repository into a genuinely usable IM bridge for multiple AI agents and multiple IM platforms. The target is not just "code exists"; the bridge must be runnable, internally coherent, and easy enough to evaluate and extend. The user specifically wants progress to be reviewed/evaluated continuously, so the loop must track both bridge functionality and reviewer/eval coverage.

## Metrics
- **Primary**: `completion_score` (unitless, higher is better) — weighted acceptance score for "usable multi-agent IM bridge".
- **Secondary**:
  - `dynamic_score` — points from commands/tests that execute successfully
  - `routing_score` — points for webhook/routing/session bridging coverage
  - `review_score` — points for built-in review/eval coverage
  - `cargo_test_ok` — 1 if `cargo test --quiet` succeeds, else 0
  - `help_ok` — 1 if `cargo run -- --help` succeeds, else 0
  - `startup_ok` — 1 if `AGENTIM_DRY_RUN=1 ./start.sh` succeeds, else 0
  - `route_count` — number of supported webhook routes detected

## How to Run
`./autoresearch.sh`

The script must always print structured `METRIC ...` lines, even if the code is incomplete, so the loop can measure progress instead of crashing on every gap.

## Files in Scope
- `src/main.rs` — binary entrypoint and startup flow
- `src/cli.rs` — CLI surface and ergonomics
- `src/bot_server.rs` — webhook server / route wiring
- `src/manager.rs` — agent/channel/session orchestration
- `src/agent.rs` — agent abstractions and built-in adapters
- `src/channel.rs` — channel abstractions and mock/local channels
- `src/bots/*.rs` — concrete IM platform adapters and webhook handling
- `src/session.rs` — session history/context model
- `src/persistence.rs` — config/persistence support
- `src/lib.rs` — exported public API
- `examples/*.rs` — usage examples
- `README.md`, `QUICK_START.md`, `SETUP.md`, `BOT_INTEGRATION.md`, `CLI_GUIDE.md` — user-facing docs that must match reality
- `Cargo.toml` — dependency/config changes only if clearly justified

## Off Limits
- Do not fake platform support purely by editing docs.
- Do not overfit to regex-only checks; prefer executable behavior and integration tests as soon as the project compiles.
- Do not add heavyweight infrastructure or external services just to satisfy the benchmark.
- Do not silently remove core multi-agent / multi-channel goals to make scoring easier.

## Constraints
- Prefer deterministic local tests over network-dependent checks.
- Keep the benchmark honest: dynamic execution should dominate static/source-inspection scoring over time.
- Preserve a simple path for a user to start the bridge locally.
- Avoid breaking the existing library API unless the usability improvement is clearly worth it.
- No benchmark cheating: score increases should correspond to real bridge capability.

## Review Rubric
The project is only "done" when most of the score comes from executable checks, not source inspection.

1. **Functionality reviewer**
   - The crate builds and tests pass.
   - Incoming messages can be routed through the manager to an agent and back to a channel.
   - Multiple IM adapters are wired coherently.
2. **Usability reviewer**
   - CLI/startup path is coherent.
   - Docs match the actual commands.
   - A newcomer can understand how to run the bridge.
3. **Readiness reviewer**
   - Bridge state/session behavior is sane.
   - Progress can be evaluated repeatedly with minimal manual interpretation.

## Current Understanding
- The repo contains the right conceptual pieces, but the current binary/docs appear inconsistent.
- `src/main.rs` is single-command bot-server startup, while several docs still describe a larger CRUD/interactive CLI.
- `src/bot_server.rs` only wires Telegram right now.
- There are concrete bot modules for Discord/Feishu/QQ, but the server/entrypoint does not yet expose them coherently.
- `src/interactive.rs` appears to depend on CLI helpers that may no longer exist.
- The first likely win is restoring buildability and creating an honest acceptance loop.

## What's Been Tried
- Session initialized by reading the repo, checking docs/code drift, and identifying likely compile/runtime gaps before the first benchmark.
- Established a mixed dynamic/static benchmark in `autoresearch.sh` so progress is measurable even when features are incomplete.
- Added a shared incoming-message bridge path in `AgentIM` that auto-creates sessions, stores per-session `reply_target`, routes to the selected agent, and sends the response back through the correct channel target.
- Rewired `src/bot_server.rs` to expose Telegram, Discord, Feishu, and QQ webhook routes instead of Telegram-only routing.
- Added executable reviewer coverage in `tests/review_bridge.rs` to validate multi-platform webhook routing, reply-target behavior, and session reuse.
- Extended startup wiring so Discord/Feishu/QQ channels can be initialized coherently, with explicit credential flags plus backward-compatible compound-token fallbacks for Feishu/QQ.
- Replaced the broken legacy `start.sh` flow with an environment-driven startup wrapper plus `AGENTIM_DRY_RUN=1` validation path.
- Rewrote the main user docs (`README.md`, `QUICK_START.md`, `SETUP.md`, `BOT_INTEGRATION.md`) so they describe the real single-command runtime, current webhook routes, and the built-in review/eval loop.
- Added per-platform agent routing for the binary via `BotServerConfig` and new CLI/startup overrides (`--telegram-agent`, `--discord-agent`, `--feishu-agent`, `--qq-agent`).
- Added executable reviewer coverage proving that different webhook routes can be mapped to different registered agents and return different responses.
