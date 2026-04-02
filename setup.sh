#!/usr/bin/env bash
set -euo pipefail

echo "╔════════════════════════════════════════════════════════════╗"
echo "║          AgentIM - ACP-First IM Agent Bridge             ║"
echo "║             Setup wrapper for current runtime            ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo

if [[ -f .env ]]; then
  echo "📦 Loading environment variables from .env..."
  set -a
  source .env
  set +a
else
  echo "ℹ️  .env file not found. Using current shell environment."
fi

# Backward-compatible env names from older docs/scripts.
if [[ -z "${TELEGRAM_TOKEN:-}" && -n "${TELEGRAM_BOT_TOKEN:-}" ]]; then
  export TELEGRAM_TOKEN="$TELEGRAM_BOT_TOKEN"
fi

if [[ -z "${DISCORD_TOKEN:-}" && -n "${DISCORD_BOT_TOKEN:-}" ]]; then
  export DISCORD_TOKEN="$DISCORD_BOT_TOKEN"
fi

if [[ -z "${AGENTIM_CONFIG_FILE:-}" && -f agentim.json ]]; then
  export AGENTIM_CONFIG_FILE=agentim.json
fi

if [[ -z "${AGENTIM_CONFIG_FILE:-}" && -n "${AGENTIM_CONFIG:-}" ]]; then
  export AGENTIM_CONFIG_FILE="${AGENTIM_CONFIG}"
fi

if [[ -z "${AGENTIM_STATE_FILE:-}" && -n "${AGENTIM_STATE:-}" ]]; then
  export AGENTIM_STATE_FILE="${AGENTIM_STATE}"
fi

if [[ -z "${AGENTIM_AGENT:-}" && -n "${AGENTIM_ACP_COMMAND:-}" ]]; then
  export AGENTIM_AGENT=acp
fi

if [[ -z "${AGENTIM_ADDR:-}" ]]; then
  export AGENTIM_ADDR=127.0.0.1:8080
fi

enabled_channels=()
[[ -n "${TELEGRAM_TOKEN:-}" ]] && enabled_channels+=(telegram)
[[ -n "${DISCORD_TOKEN:-}" ]] && enabled_channels+=(discord)
[[ -n "${FEISHU_APP_ID:-}${FEISHU_TOKEN:-}" ]] && enabled_channels+=(feishu)
[[ -n "${QQ_BOT_ID:-}${QQ_TOKEN:-}" ]] && enabled_channels+=(qq)

echo "Agent:   ${AGENTIM_AGENT:-from config or binary default}"
echo "Address: ${AGENTIM_ADDR}"
if [[ -n "${AGENTIM_ACP_COMMAND:-}" ]]; then
  echo "ACP:     ${AGENTIM_ACP_COMMAND}"
fi
if [[ -n "${AGENTIM_ACP_ARGS:-}" ]]; then
  echo "ACP args: ${AGENTIM_ACP_ARGS}"
fi
if [[ -n "${AGENTIM_ACP_ENV:-}" ]]; then
  echo "ACP env:  ${AGENTIM_ACP_ENV}"
fi
if [[ -n "${AGENTIM_CONFIG_FILE:-}" ]]; then
  echo "Config:  ${AGENTIM_CONFIG_FILE}"
fi
if [[ -n "${AGENTIM_STATE_FILE:-}" ]]; then
  echo "State:   ${AGENTIM_STATE_FILE}"
fi
if (( ${#enabled_channels[@]} > 0 )); then
  echo "Channels: ${enabled_channels[*]}"
else
  echo "Channels: none (the server can still start, but no platform webhook will be active)"
fi
echo

echo "Recommended preflight: AGENTIM_DRY_RUN=1 ./setup.sh"
echo "Delegating to ./start.sh ..."
echo

exec ./start.sh "$@"
