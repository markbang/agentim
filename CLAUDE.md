# AgentIM Development Guide

## Build & Test

```bash
cargo build              # debug build
cargo build --release    # release build
cargo test               # all tests (unit + integration)
cargo clippy             # lint
cargo fmt --check        # format check
```

## Dry Run

Validate startup config without connecting to any external services:

```bash
cargo run -- --dry-run --agent acp --acp-command acp
```

## Architecture

- `src/acp.rs` - `AcpAgent` ACP session transport
- `src/agent.rs` - `Agent` trait definition
- `src/channel.rs` - `Channel` trait definition
- `src/bots/*.rs` - Real platform channel implementations (Telegram, Discord, Feishu, QQ, Slack, DingTalk, WeChat Work, LINE)
- `src/manager.rs` - `AgentIM` orchestrator: agent/channel/session registry, message routing
- `src/bot_server.rs` - Axum webhook handlers, routing rules, security verification, health endpoints
- `src/session.rs` - Session struct with message history, context windowing, history summarization
- `src/config.rs` - Type definitions (AgentType, ChannelType)
- `src/cli.rs` - Clap argument parsing + colored output helpers
- `src/error.rs` - Error types via thiserror

## Key Conventions

- All concurrency through DashMap (no Mutex on hot paths)
- Session history uses VecDeque with configurable trim
- Every webhook handler follows: authorize -> parse -> route -> respond -> persist
- Tests use in-process Axum via `tower::ServiceExt::oneshot`
- `main.rs` uses the library crate (`use agentim::*`), not `mod` declarations
- New backend integrations should go through ACP compatibility rather than adding runtime-specific transport paths
