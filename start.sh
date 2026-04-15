#!/usr/bin/env bash
set -euo pipefail

AGENTIM_HOME="${AGENTIM_HOME:-.}"
BINARY="${AGENTIM_HOME}/target/release/agentim"
AGENT="${AGENTIM_AGENT:-}"
ADDR="${AGENTIM_ADDR:-}"
DRY_RUN="${AGENTIM_DRY_RUN:-0}"

args=()
[[ -n "$AGENT" ]] && args+=(--agent "$AGENT")
[[ -n "$ADDR" ]] && args+=(--addr "$ADDR")

[[ -n "${AGENTIM_CONFIG_FILE:-}" ]] && args+=(--config-file "$AGENTIM_CONFIG_FILE")
[[ -n "${TELEGRAM_AGENT:-}" ]] && args+=(--telegram-agent "$TELEGRAM_AGENT")
[[ -n "${DISCORD_AGENT:-}" ]] && args+=(--discord-agent "$DISCORD_AGENT")
[[ -n "${FEISHU_AGENT:-}" ]] && args+=(--feishu-agent "$FEISHU_AGENT")
[[ -n "${QQ_AGENT:-}" ]] && args+=(--qq-agent "$QQ_AGENT")
[[ -n "${LINE_AGENT:-}" ]] && args+=(--line-agent "$LINE_AGENT")
[[ -n "${WECHATWORK_AGENT:-}" ]] && args+=(--wechatwork-agent "$WECHATWORK_AGENT")
[[ -n "${ACP_COMMAND:-}" ]] && args+=(--acp-command "$ACP_COMMAND")
[[ -n "${ACP_CWD:-}" ]] && args+=(--acp-cwd "$ACP_CWD")
if [[ -n "${ACP_ARGS:-}" ]]; then
  # shellcheck disable=SC2206
  acp_args=($ACP_ARGS)
  for arg in "${acp_args[@]}"; do
    args+=(--acp-arg "$arg")
  done
fi
if [[ -n "${ACP_ENV_VARS:-}" ]]; then
  # shellcheck disable=SC2206
  acp_env_vars=($ACP_ENV_VARS)
  for item in "${acp_env_vars[@]}"; do
    args+=(--acp-env "$item")
  done
fi
[[ -n "${AGENTIM_STATE_FILE:-}" ]] && args+=(--state-file "$AGENTIM_STATE_FILE")
[[ -n "${AGENTIM_STATE_BACKUP_COUNT:-}" ]] && args+=(--state-backup-count "$AGENTIM_STATE_BACKUP_COUNT")
[[ -n "${AGENTIM_MAX_SESSION_MESSAGES:-}" ]] && args+=(--max-session-messages "$AGENTIM_MAX_SESSION_MESSAGES")
[[ -n "${AGENTIM_CONTEXT_MESSAGE_LIMIT:-}" ]] && args+=(--context-message-limit "$AGENTIM_CONTEXT_MESSAGE_LIMIT")
[[ -n "${AGENTIM_AGENT_TIMEOUT_MS:-}" ]] && args+=(--agent-timeout-ms "$AGENTIM_AGENT_TIMEOUT_MS")
[[ -n "${AGENTIM_WEBHOOK_SECRET:-}" ]] && args+=(--webhook-secret "$AGENTIM_WEBHOOK_SECRET")
[[ -n "${AGENTIM_WEBHOOK_SIGNING_SECRET:-}" ]] && args+=(--webhook-signing-secret "$AGENTIM_WEBHOOK_SIGNING_SECRET")
[[ -n "${AGENTIM_WEBHOOK_MAX_SKEW_SECONDS:-}" ]] && args+=(--webhook-max-skew-seconds "$AGENTIM_WEBHOOK_MAX_SKEW_SECONDS")
[[ -n "${FEISHU_WEBHOOK_VERIFICATION_TOKEN:-}" ]] && args+=(--feishu-verification-token "$FEISHU_WEBHOOK_VERIFICATION_TOKEN")

if [[ -n "${TELEGRAM_TOKEN:-}" ]]; then
  args+=(--telegram-token "$TELEGRAM_TOKEN")
fi

if [[ -n "${DISCORD_TOKEN:-}" ]]; then
  args+=(--discord-token "$DISCORD_TOKEN")
fi

if [[ -n "${DISCORD_INTERACTION_PUBLIC_KEY:-}" ]]; then
  args+=(--discord-interaction-public-key "$DISCORD_INTERACTION_PUBLIC_KEY")
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

if [[ -n "${LINE_CHANNEL_TOKEN:-}" ]]; then
  args+=(--line-channel-token "$LINE_CHANNEL_TOKEN")
fi
if [[ -n "${LINE_CHANNEL_SECRET:-}" ]]; then
  args+=(--line-channel-secret "$LINE_CHANNEL_SECRET")
fi

if [[ -n "${WECHATWORK_CORP_ID:-}" || -n "${WECHATWORK_AGENT_ID:-}" || -n "${WECHATWORK_SECRET:-}" ]]; then
  : "${WECHATWORK_CORP_ID:?WECHATWORK_CORP_ID must be set with WECHATWORK_AGENT_ID and WECHATWORK_SECRET}"
  : "${WECHATWORK_AGENT_ID:?WECHATWORK_AGENT_ID must be set with WECHATWORK_CORP_ID and WECHATWORK_SECRET}"
  : "${WECHATWORK_SECRET:?WECHATWORK_SECRET must be set with WECHATWORK_CORP_ID and WECHATWORK_AGENT_ID}"
  args+=(
    --wechatwork-corp-id "$WECHATWORK_CORP_ID"
    --wechatwork-agent-id "$WECHATWORK_AGENT_ID"
    --wechatwork-secret "$WECHATWORK_SECRET"
  )
fi

echo "╔════════════════════════════════════════════════════════════╗"
echo "║          AgentIM - Multi-Channel AI Agent Manager        ║"
echo "║               Environment-driven startup                 ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo

echo "Agent:   ${AGENT:-from config or binary default (acp)}"
echo "Address: ${ADDR:-from config or binary default (127.0.0.1:8080)}"
[[ -n "${ACP_COMMAND:-}" ]] && echo "ACP command: ${ACP_COMMAND}"
[[ -n "${ACP_CWD:-}" ]] && echo "ACP cwd: ${ACP_CWD}"
[[ -n "${ACP_ARGS:-}" ]] && echo "ACP args: ${ACP_ARGS}"
[[ -n "${AGENTIM_CONFIG_FILE:-}" ]] && echo "Config:  ${AGENTIM_CONFIG_FILE}"
[[ -n "${TELEGRAM_TOKEN:-}" ]] && echo "Telegram: enabled"
[[ -n "${DISCORD_TOKEN:-}" ]] && echo "Discord:  enabled"
[[ -n "${DISCORD_INTERACTION_PUBLIC_KEY:-}" ]] && echo "Discord native signature: enabled"
[[ -n "${FEISHU_APP_ID:-}${FEISHU_TOKEN:-}" ]] && echo "Feishu:   enabled"
[[ -n "${QQ_BOT_ID:-}${QQ_TOKEN:-}" ]] && echo "QQ:       enabled"
[[ -n "${LINE_CHANNEL_TOKEN:-}" ]] && echo "LINE:     enabled"
[[ -n "${WECHATWORK_CORP_ID:-}${WECHATWORK_AGENT_ID:-}${WECHATWORK_SECRET:-}" ]] && echo "WeChat Work: enabled"
[[ -n "${AGENTIM_WEBHOOK_SECRET:-}" ]] && echo "Shared auth: enabled"
[[ -n "${AGENTIM_WEBHOOK_SIGNING_SECRET:-}" ]] && echo "Signed auth: enabled"
[[ -n "${FEISHU_WEBHOOK_VERIFICATION_TOKEN:-}" ]] && echo "Feishu verification token: enabled"
[[ -n "${AGENTIM_STATE_BACKUP_COUNT:-}" ]] && echo "State backup rotation: ${AGENTIM_STATE_BACKUP_COUNT}"
[[ -n "${AGENTIM_CONTEXT_MESSAGE_LIMIT:-}" ]] && echo "Agent context window: ${AGENTIM_CONTEXT_MESSAGE_LIMIT}"
[[ -n "${AGENTIM_AGENT_TIMEOUT_MS:-}" ]] && echo "Agent timeout: ${AGENTIM_AGENT_TIMEOUT_MS}ms"
echo

echo "Command:"
printf '  %q' "$BINARY" "${args[@]}" "$@"
printf '\n\n'

needs_build=0
if [[ ! -x "$BINARY" ]]; then
  needs_build=1
elif [[ "$AGENTIM_HOME/Cargo.toml" -nt "$BINARY" || "$AGENTIM_HOME/Cargo.lock" -nt "$BINARY" ]]; then
  needs_build=1
elif find "$AGENTIM_HOME/src" -type f -newer "$BINARY" -print -quit | grep -q .; then
  needs_build=1
fi

if [[ "$needs_build" == "1" ]]; then
  echo "🔨 Release binary missing or stale. Building..."
  cargo build --release
  echo
fi

if [[ "$DRY_RUN" == "1" ]]; then
  echo "AGENTIM_DRY_RUN=1 -> running binary --dry-run for offline validation."
  exec "$BINARY" "${args[@]}" --dry-run "$@"
fi

exec "$BINARY" "${args[@]}" "$@"
