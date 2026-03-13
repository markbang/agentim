# AgentIM Quick Reference

## Installation & Setup

### Build
```bash
cargo build --release
```

### Quick Start (Interactive)
```bash
./target/release/agentim interactive
```

### Production Setup
```bash
cp agentim.json.example agentim.json
# Edit agentim.json
./start.sh
```

## CLI Commands

### Agents
```bash
agentim agent list                                    # List all agents
agentim agent register --id <id> --agent-type <type> # Register agent
agentim agent health --id <id>                        # Health check
```

Agent types: `claude`, `codex`, `pi`

### Channels
```bash
agentim channel list                                      # List all channels
agentim channel register --id <id> --channel-type <type> # Register channel
agentim channel health --id <id>                         # Health check
```

Channel types: `telegram`, `discord`, `feishu`, `qq`

### Sessions
```bash
agentim session list                                                    # List sessions
agentim session create --agent-id <a> --channel-id <c> --user-id <u>  # Create session
agentim session get --id <id>                                          # Get details
agentim session send --session-id <id> --message "<msg>"              # Send message
agentim session delete --id <id>                                       # Delete session
```

### System
```bash
agentim status      # View system status
agentim interactive # Interactive mode
```

## Configuration File (agentim.json)

```json
{
  "agents": [
    {
      "id": "claude-main",
      "agent_type": "claude",
      "model": "claude-3-5-sonnet-20241022"
    }
  ],
  "channels": [
    {
      "id": "telegram-main",
      "channel_type": "telegram"
    }
  ],
  "sessions": [
    {
      "agent_id": "claude-main",
      "channel_id": "telegram-main",
      "user_id": "user123"
    }
  ]
}
```

## Message Flow

```
User Message
    ↓
./agentim session send --session-id <id> --message "..."
    ↓
Agent processes message with context
    ↓
Agent generates response
    ↓
Response sent to channel
    ↓
User receives response
```

## Example Workflow

```bash
# 1. Build
cargo build --release

# 2. Register agent
./target/release/agentim agent register --id claude-1 --agent-type claude

# 3. Register channel
./target/release/agentim channel register --id tg-1 --channel-type telegram

# 4. Create session
SESSION=$(./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id tg-1 \
  --user-id user123 | grep -oP 'Session created: \K\S+')

# 5. Send message
./target/release/agentim session send \
  --session-id $SESSION \
  --message "What is 2+2?"

# 6. Check status
./target/release/agentim status
```

## Documentation

- `README.md` - Project overview
- `SETUP.md` - Detailed setup guide
- `ARCHITECTURE.md` - Architecture details
- `CLI_GUIDE.md` - CLI reference
- `IMPLEMENTATION_SUMMARY.md` - What was implemented

## Key Features

✅ Multi-agent support (Claude, Codex, Pi)
✅ Multi-channel support (Telegram, Discord, Feishu, QQ)
✅ Session-based message history
✅ Interactive setup mode
✅ Configuration file support
✅ Health checks
✅ Concurrent session handling
✅ Production-ready

## Troubleshooting

### Build fails
```bash
cargo clean
cargo build --release
```

### Agent/Channel not found
- Ensure you registered the agent/channel first
- Check the ID matches exactly

### Session creation fails
- Verify agent and channel are registered
- Check IDs are correct

### Message not sent
- Verify session exists
- Check agent and channel are healthy
- View session details: `agentim session get --id <id>`
