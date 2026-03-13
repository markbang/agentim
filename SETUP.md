# AgentIM Setup Guide

## Quick Start (5 minutes)

### Option 1: Interactive Setup (Recommended)

```bash
# Build the project
cargo build --release

# Start interactive setup
./target/release/agentim interactive
```

This will guide you through:
1. Registering AI agents (Claude, Codex, Pi)
2. Registering communication channels (Telegram, Discord, Feishu, QQ)
3. Creating sessions between agents and channels
4. Testing message flow

### Option 2: Configuration File

1. Copy the example configuration:
```bash
cp agentim.json.example agentim.json
```

2. Edit `agentim.json` with your agents and channels:
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

3. Run the startup script:
```bash
./start.sh
```

## Agent Types

### Claude
- **Type**: `claude`
- **Model**: Optional (default: `claude-3-5-sonnet-20241022`)
- **Example**:
  ```json
  {
    "id": "claude-main",
    "agent_type": "claude",
    "model": "claude-3-5-sonnet-20241022"
  }
  ```

### Codex
- **Type**: `codex`
- **Model**: Optional (default: `code-davinci-002`)
- **Example**:
  ```json
  {
    "id": "codex-main",
    "agent_type": "codex",
    "model": "code-davinci-002"
  }
  ```

### Pi
- **Type**: `pi`
- **Model**: Not applicable
- **Example**:
  ```json
  {
    "id": "pi-main",
    "agent_type": "pi"
  }
  ```

## Channel Types

### Telegram
- **Type**: `telegram`
- **Example**:
  ```json
  {
    "id": "telegram-main",
    "channel_type": "telegram"
  }
  ```

### Discord
- **Type**: `discord`
- **Example**:
  ```json
  {
    "id": "discord-main",
    "channel_type": "discord"
  }
  ```

### Feishu (Lark)
- **Type**: `feishu`
- **Example**:
  ```json
  {
    "id": "feishu-main",
    "channel_type": "feishu"
  }
  ```

### QQ
- **Type**: `qq`
- **Example**:
  ```json
  {
    "id": "qq-main",
    "channel_type": "qq"
  }
  ```

## CLI Commands

### Agent Management
```bash
# List all agents
./target/release/agentim agent list

# Register a new agent
./target/release/agentim agent register --id claude-1 --agent-type claude

# Health check
./target/release/agentim agent health --id claude-1
```

### Channel Management
```bash
# List all channels
./target/release/agentim channel list

# Register a new channel
./target/release/agentim channel register --id tg-1 --channel-type telegram

# Health check
./target/release/agentim channel health --id tg-1
```

### Session Management
```bash
# List all sessions
./target/release/agentim session list

# Create a new session
./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id tg-1 \
  --user-id user123

# Get session details
./target/release/agentim session get --id <session-id>

# Send a message
./target/release/agentim session send \
  --session-id <session-id> \
  --message "Hello, agent!"

# Delete a session
./target/release/agentim session delete --id <session-id>
```

### System Status
```bash
# View system status
./target/release/agentim status

# Interactive mode
./target/release/agentim interactive
```

## Message Flow

1. **User sends message** → Channel receives it
2. **Message routed to Agent** → Agent processes the message
3. **Agent generates response** → Response sent back to Channel
4. **User receives response** → Channel delivers it

Example:
```bash
# Create a session
SESSION_ID=$(./target/release/agentim session create \
  --agent-id claude-1 \
  --channel-id tg-1 \
  --user-id user123 | grep -oP 'Session created: \K\S+')

# Send a message
./target/release/agentim session send \
  --session-id $SESSION_ID \
  --message "What is 2+2?"
```

## Configuration File Format

The `agentim.json` file contains three main sections:

### Agents Section
```json
"agents": [
  {
    "id": "unique-agent-id",
    "agent_type": "claude|codex|pi",
    "model": "optional-model-name"
  }
]
```

### Channels Section
```json
"channels": [
  {
    "id": "unique-channel-id",
    "channel_type": "telegram|discord|feishu|qq"
  }
]
```

### Sessions Section
```json
"sessions": [
  {
    "agent_id": "must-match-agent-id",
    "channel_id": "must-match-channel-id",
    "user_id": "unique-user-identifier"
  }
]
```

## Production Deployment

### 1. Build Release Binary
```bash
cargo build --release
```

The binary will be at `./target/release/agentim`

### 2. Prepare Configuration
```bash
cp agentim.json.example agentim.json
# Edit agentim.json with your setup
```

### 3. Run Startup Script
```bash
./start.sh
```

### 4. Verify Status
```bash
./target/release/agentim status
```

## Troubleshooting

### Build Issues
```bash
# Clean and rebuild
cargo clean
cargo build --release
```

### Configuration Issues
- Ensure `agentim.json` is valid JSON
- Check that agent IDs in sessions match registered agents
- Check that channel IDs in sessions match registered channels

### Runtime Issues
```bash
# Check system status
./target/release/agentim status

# Check specific agent
./target/release/agentim agent health --id <agent-id>

# Check specific channel
./target/release/agentim channel health --id <channel-id>
```

## Next Steps

1. **Integrate Real APIs**: Replace mock implementations with actual API calls
2. **Add Persistence**: Store sessions and messages in a database
3. **Web Interface**: Add a REST API for remote management
4. **Monitoring**: Add metrics and logging for production use

See `ARCHITECTURE.md` for detailed design information.
