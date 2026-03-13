#!/bin/bash

# AgentIM Setup Script
# This script sets up AgentIM with multiple agents and channels

set -e

echo "╔════════════════════════════════════════════════════════════╗"
echo "║          AgentIM - Multi-Channel AI Agent Manager          ║"
echo "║                    Setup Script                            ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# Load environment variables
if [ -f .env ]; then
    echo "📦 Loading environment variables from .env..."
    source .env
else
    echo "⚠️  .env file not found. Using environment variables."
fi

# Build the project
echo ""
echo "🔨 Building AgentIM..."
cargo build --release 2>&1 | tail -5

AGENTIM="./target/release/agentim"

# Check if build was successful
if [ ! -f "$AGENTIM" ]; then
    echo "❌ Build failed!"
    exit 1
fi

echo "✅ Build successful!"
echo ""

# Register Agents
echo "📋 Registering Agents..."
echo ""

if [ -n "$ANTHROPIC_API_KEY" ]; then
    echo "  → Registering Claude Agent..."
    $AGENTIM agent register \
        --id claude-main \
        --agent-type claude \
        --api-key "$ANTHROPIC_API_KEY" \
        --model claude-3-5-sonnet-20241022
    echo "    ✅ Claude Agent registered"
else
    echo "    ⚠️  ANTHROPIC_API_KEY not set, skipping Claude"
fi

if [ -n "$OPENAI_API_KEY" ]; then
    echo "  → Registering Codex Agent..."
    $AGENTIM agent register \
        --id codex-main \
        --agent-type codex \
        --api-key "$OPENAI_API_KEY"
    echo "    ✅ Codex Agent registered"
else
    echo "    ⚠️  OPENAI_API_KEY not set, skipping Codex"
fi

if [ -n "$PI_API_KEY" ]; then
    echo "  → Registering Pi Agent..."
    $AGENTIM agent register \
        --id pi-main \
        --agent-type pi \
        --api-key "$PI_API_KEY"
    echo "    ✅ Pi Agent registered"
else
    echo "    ⚠️  PI_API_KEY not set, skipping Pi"
fi

echo ""

# Register Channels
echo "📋 Registering Channels..."
echo ""

if [ -n "$TELEGRAM_BOT_TOKEN" ]; then
    echo "  → Registering Telegram Channel..."
    $AGENTIM channel register \
        --id tg-main \
        --channel-type telegram \
        --credentials "{\"token\":\"$TELEGRAM_BOT_TOKEN\"}"
    echo "    ✅ Telegram Channel registered"
else
    echo "    ⚠️  TELEGRAM_BOT_TOKEN not set, skipping Telegram"
fi

if [ -n "$DISCORD_BOT_TOKEN" ]; then
    echo "  → Registering Discord Channel..."
    $AGENTIM channel register \
        --id discord-main \
        --channel-type discord \
        --credentials "{\"token\":\"$DISCORD_BOT_TOKEN\"}"
    echo "    ✅ Discord Channel registered"
else
    echo "    ⚠️  DISCORD_BOT_TOKEN not set, skipping Discord"
fi

if [ -n "$FEISHU_APP_ID" ] && [ -n "$FEISHU_APP_SECRET" ]; then
    echo "  → Registering Feishu Channel..."
    $AGENTIM channel register \
        --id feishu-main \
        --channel-type feishu \
        --credentials "{\"app_id\":\"$FEISHU_APP_ID\",\"app_secret\":\"$FEISHU_APP_SECRET\"}"
    echo "    ✅ Feishu Channel registered"
else
    echo "    ⚠️  FEISHU credentials not set, skipping Feishu"
fi

if [ -n "$QQ_BOT_ID" ] && [ -n "$QQ_BOT_TOKEN" ]; then
    echo "  → Registering QQ Channel..."
    $AGENTIM channel register \
        --id qq-main \
        --channel-type qq \
        --credentials "{\"bot_id\":\"$QQ_BOT_ID\",\"bot_token\":\"$QQ_BOT_TOKEN\"}"
    echo "    ✅ QQ Channel registered"
else
    echo "    ⚠️  QQ credentials not set, skipping QQ"
fi

echo ""

# Show system status
echo "📊 System Status:"
echo ""
$AGENTIM status

echo ""
echo "╔════════════════════════════════════════════════════════════╗"
echo "║                   Setup Complete! 🎉                       ║"
echo "╚══��═════════════════════════════════════════════════════════╝"
echo ""
echo "Next steps:"
echo "  1. View all agents:   $AGENTIM agent list"
echo "  2. View all channels: $AGENTIM channel list"
echo "  3. View all sessions: $AGENTIM session list"
echo "  4. Create a session:  $AGENTIM session create --agent-id claude-main --channel-id tg-main --user-id user123"
echo ""
