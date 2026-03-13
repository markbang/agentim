#!/bin/bash

# AgentIM Startup Script
# Simple installation and configuration

set -e

AGENTIM_HOME="${AGENTIM_HOME:-.}"
CONFIG_FILE="${AGENTIM_HOME}/agentim.json"
BINARY="${AGENTIM_HOME}/target/release/agentim"

echo "╔════════════════════════════════════════════════════════════╗"
echo "║          AgentIM - Multi-Channel AI Agent Manager          ║"
echo "║                    Startup Script                          ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Check if binary exists
if [ ! -f "$BINARY" ]; then
    echo "🔨 Building AgentIM..."
    cargo build --release
    echo "✅ Build complete"
    echo ""
fi

# Check if config exists
if [ ! -f "$CONFIG_FILE" ]; then
    echo "📝 No configuration found. Starting interactive setup..."
    echo ""
    "$BINARY" interactive
else
    echo "📋 Loading configuration from $CONFIG_FILE"
    echo ""

    # Load agents from config
    echo "📦 Registering agents..."
    agents=$(jq -r '.agents[]' "$CONFIG_FILE" 2>/dev/null || echo "")
    if [ -n "$agents" ]; then
        echo "$agents" | while read -r agent; do
            id=$(echo "$agent" | jq -r '.id')
            type=$(echo "$agent" | jq -r '.agent_type')
            model=$(echo "$agent" | jq -r '.model // empty')

            if [ -n "$model" ]; then
                "$BINARY" agent register --id "$id" --agent-type "$type" --model "$model"
            else
                "$BINARY" agent register --id "$id" --agent-type "$type"
            fi
        done
    fi

    # Load channels from config
    echo "📦 Registering channels..."
    channels=$(jq -r '.channels[]' "$CONFIG_FILE" 2>/dev/null || echo "")
    if [ -n "$channels" ]; then
        echo "$channels" | while read -r channel; do
            id=$(echo "$channel" | jq -r '.id')
            type=$(echo "$channel" | jq -r '.channel_type')
            "$BINARY" channel register --id "$id" --channel-type "$type"
        done
    fi

    # Load sessions from config
    echo "📦 Creating sessions..."
    sessions=$(jq -r '.sessions[]' "$CONFIG_FILE" 2>/dev/null || echo "")
    if [ -n "$sessions" ]; then
        echo "$sessions" | while read -r session; do
            agent=$(echo "$session" | jq -r '.agent_id')
            channel=$(echo "$session" | jq -r '.channel_id')
            user=$(echo "$session" | jq -r '.user_id')
            "$BINARY" session create --agent-id "$agent" --channel-id "$channel" --user-id "$user"
        done
    fi
fi

echo ""
echo "📊 System Status:"
"$BINARY" status

echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                   Setup Complete! 🎉                       ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""
echo "Next steps:"
echo "  • Interactive mode:  $BINARY interactive"
echo "  • View agents:       $BINARY agent list"
echo "  • View channels:     $BINARY channel list"
echo "  • View sessions:     $BINARY session list"
echo "  • Send message:      $BINARY session send --session-id <id> --message '<msg>'"
echo ""
