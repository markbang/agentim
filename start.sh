#!/usr/bin/env bash
set -euo pipefail

AGENTIM_HOME="${AGENTIM_HOME:-.}"
BINARY="${AGENTIM_HOME}/target/release/agentim"
AGENT="${AGENTIM_AGENT:-claude}"
ADDR="${AGENTIM_ADDR:-127.0.0.1:8080}"
DRY_RUN="${AGENTIM_DRY_RUN:-0}"

args=(--agent "$AGENT" --addr "$ADDR")

[[ -n "${TELEGRAM_AGENT:-}" ]] && args+=(--telegram-agent "$TELEGRAM_AGENT")
[[ -n "${DISCORD_AGENT:-}" ]] && args+=(--discord-agent "$DISCORD_AGENT")
[[ -n "${FEISHU_AGENT:-}" ]] && args+=(--feishu-agent "$FEISHU_AGENT")
[[ -n "${QQ_AGENT:-}" ]] && args+=(--qq-agent "$QQ_AGENT")
[[ -n "${AGENTIM_STATE_FILE:-}" ]] && args+=(--state-file "$AGENTIM_STATE_FILE")

if [[ -n "${TELEGRAM_TOKEN:-}" ]]; then
  args+=(--telegram-token "$TELEGRAM_TOKEN")
fi

if [[ -n "${DISCORD_TOKEN:-}" ]]; then
  args+=(--discord-token "$DISCORD_TOKEN")
fi

if [[ -n "${FEISHU_APP_ID:-}" || -n "${FEISHU_APP_SECRET:-}" ]]; then
  : "${FEISHU_APP_ID:?FEISHU_APP_ID must be set with FEISHU_APP_SECRET}"
  : "${FEISHU_APP_SECRET:?FEISHU_APP_SECRET must be set with FEISHU_APP_ID}"
  args+=(--feishu-app-id "$FEISHU_APP_ID" --feishu-app-secret "$FEISHU_APP_SECRET")
elif [[ -n "${FEISHU_TOKEN:-}" ]]; then
  args+=(--feishu-token "$FEISHU_TOKEN")
fi

if [[ -n "${QQ_BOT_ID:-}" || -n "${QQ_BOT_TOKEN:-}" ]]; then
  : "${QQ_BOT_ID:?QQ_BOT_ID must be set with QQ_BOT_TOKEN}"
  : "${QQ_BOT_TOKEN:?QQ_BOT_TOKEN must be set with QQ_BOT_ID}"
  args+=(--qq-bot-id "$QQ_BOT_ID" --qq-bot-token "$QQ_BOT_TOKEN")
elif [[ -n "${QQ_TOKEN:-}" ]]; then
  args+=(--qq-token "$QQ_TOKEN")
fi

echo "╔════════════════════════════════════════════════════════════╗"
echo "║          AgentIM - Multi-Channel AI Agent Manager        ║"
echo "║               Environment-driven startup                 ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo

echo "Agent:   $AGENT"
echo "Address: $ADDR"
[[ -n "${TELEGRAM_TOKEN:-}" ]] && echo "Telegram: enabled"
[[ -n "${DISCORD_TOKEN:-}" ]] && echo "Discord:  enabled"
[[ -n "${FEISHU_APP_ID:-}${FEISHU_TOKEN:-}" ]] && echo "Feishu:   enabled"
[[ -n "${QQ_BOT_ID:-}${QQ_TOKEN:-}" ]] && echo "QQ:       enabled"
echo

echo "Command:"
printf '  %q' "$BINARY" "${args[@]}" "$@"
printf '\n\n'

if [[ "$DRY_RUN" == "1" ]]; then
  echo "AGENTIM_DRY_RUN=1 -> validated startup configuration without executing the server."
  exit 0
fi

if [[ ! -x "$BINARY" ]]; then
  echo "🔨 Release binary not found. Building..."
  cargo build --release
  echo
fi

exec "$BINARY" "${args[@]}" "$@"
