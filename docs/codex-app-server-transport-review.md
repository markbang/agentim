# Codex Transport Review: app-server JSON-RPC vs ACP session/*

## Summary

The current `src/acp.rs` client speaks an ACP session transport built around:

- `initialize`
- `session/new`
- `session/load`
- `session/prompt`
- `session/update`

That transport does **not** match the Codex CLI transport exposed today.

## Evidence

### 1. Codex app-server schema exposes thread/turn JSON-RPC methods, not ACP session methods

Local verification:

```bash
tmpdir=$(mktemp -d)
codex app-server generate-json-schema --out "$tmpdir"
rg -n 'session/new|session/load|session/prompt|thread/start|turn/start|mcpServerStatus/list' "$tmpdir"
```

Observed matches:

- `thread/start`
- `turn/start`
- `mcpServerStatus/list`

Observed non-matches:

- no `session/new`
- no `session/load`
- no `session/prompt`

### 2. Codex CLI surfaces app-server and mcp-server as distinct products

Local verification:

```bash
codex app-server --help
codex mcp-server --help
```

The CLI advertises:

- `codex app-server` for the Codex app-server protocol
- `codex mcp-server` for Codex as an MCP server

Neither help surface claims ACP `session/*` compatibility.

### 3. `src/acp.rs` is hard-coded to the ACP session protocol

`src/acp.rs` currently depends on:

- `ACP_NEW_SESSION_METHOD: "session/new"`
- `ACP_LOAD_SESSION_METHOD: "session/load"`
- `ACP_PROMPT_METHOD: "session/prompt"`
- `ACP_SESSION_UPDATE_NOTIFICATION: "session/update"`

That is a protocol-level mismatch, not a naming-only issue.

## Implication

The repository should **not** present current Codex integration as “ACP-compatible” unless a real Codex transport that implements `session/new|load|prompt` is verified.

The evidence supports this correction:

- **Codex bridge target:** `codex app-server` JSON-RPC
- **Not validated as Codex target:** `src/acp.rs` ACP session transport

## Current implementation status

AgentIM now ships the local Codex bridge through `src/codex.rs`, which speaks the verified
app-server thread/turn JSON-RPC surface:

- `initialize`
- `thread/start`
- `thread/resume`
- `turn/start`
- streamed `item/agentMessage/delta`
- `turn/completed`

The legacy `src/acp.rs` module remains in the tree for ACP experiments/tests only and should not be
described as the real Codex path.

## Naming / UX fallout

The bridge now uses Codex-oriented user-facing naming:

- `--codex-command`
- `--codex-arg`
- `--codex-cwd`
- `--codex-env`

If compatibility with non-Codex ACP agents is still desired in the future, keep ACP and Codex as
separate transports rather than overloading the Codex app-server path with ACP semantics.

## Verified product direction

- The default startup path should assume a local Codex backend.
- Telegram-only onboarding should not require `OPENAI_API_KEY`.
- Any future non-Codex ACP support should remain a separate transport instead of being conflated with
  the Codex app-server bridge.
