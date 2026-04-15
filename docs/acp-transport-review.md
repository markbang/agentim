# ACP Transport Review

This project now uses ACP as the only supported runtime backend protocol.

## Current Position

- **Runtime backend target:** ACP `session/new|load|prompt`
- **Default startup path:** local ACP-compatible backend process
- **Main transport implementation:** `src/acp.rs`

## Implications

- AgentIM no longer exposes a Codex-specific runtime path.
- New agent integrations should be implemented behind ACP compatibility.
- Session lifecycle, prompt dispatch, and reconnect behavior are all handled through ACP.

## Operational Guidance

Use one of the following patterns:

```bash
agentim --acp-command your-agent --acp-arg serve --telegram-token YOUR_TELEGRAM_BOT_TOKEN
```

or via config:

```json
{
  "agent": "acp",
  "acp_command": "your-agent",
  "acp_args": ["serve"]
}
```

## Repository Guidance

- Prefer ACP for all backend integrations.
- Do not add new backend-specific runtime entrypoints unless there is a strong architectural reason.
- Keep transport semantics aligned with ACP session APIs.
