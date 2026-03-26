#!/usr/bin/env bash
set -euo pipefail

score=0
dynamic_score=0
routing_score=0
review_score=0
cargo_test_ok=0
help_ok=0
startup_ok=0
route_count=0
handler_count=0
autocreate_ok=0
review_artifacts=0

# Dynamic checks: prefer execution, but do not crash the whole benchmark when code is incomplete.
set +e
cargo test --quiet >/tmp/agentim-cargo-test.out 2>/tmp/agentim-cargo-test.err
cargo_test_status=$?
set -e
if [ "$cargo_test_status" -eq 0 ]; then
  cargo_test_ok=1
  dynamic_score=$((dynamic_score + 40))
fi

set +e
cargo run --quiet -- --help >/tmp/agentim-help.out 2>/tmp/agentim-help.err
help_status=$?
set -e
if [ "$help_status" -eq 0 ]; then
  help_ok=1
  required_flags=0
  for flag in --telegram-token --telegram-webhook-secret-token --discord-token --feishu-token --feishu-app-id --feishu-app-secret --qq-token --qq-bot-id --qq-bot-token --agent --telegram-agent --discord-agent --feishu-agent --qq-agent --config-file --dry-run --state-file --state-backup-count --max-session-messages --webhook-secret --webhook-signing-secret --webhook-max-skew-seconds --addr; do
    if grep -q -- "$flag" /tmp/agentim-help.out; then
      required_flags=$((required_flags + 1))
    fi
  done
  dynamic_score=$((dynamic_score + required_flags))
fi

set +e
AGENTIM_DRY_RUN=1 ./start.sh >/tmp/agentim-start.out 2>/tmp/agentim-start.err
start_status=$?
set -e
if [ "$start_status" -eq 0 ]; then
  startup_ok=1
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge functionality_reviewer_routes_channels_to_configured_agents \
  >/tmp/agentim-multi-agent-test.out 2>/tmp/agentim-multi-agent-test.err
multi_agent_test_status=$?
set -e
if [ "$multi_agent_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge routing_reviewer_overrides_platform_route_for_matching_user \
  >/tmp/agentim-routing-rule-test.out 2>/tmp/agentim-routing-rule-test.err
routing_rule_test_status=$?
set -e
if [ "$routing_rule_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge routing_reviewer_overrides_platform_route_for_matching_reply_target \
  >/tmp/agentim-reply-target-rule-test.out 2>/tmp/agentim-reply-target-rule-test.err
reply_target_rule_test_status=$?
set -e
if [ "$reply_target_rule_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge routing_reviewer_matches_reply_target_prefix \
  >/tmp/agentim-routing-prefix-test.out 2>/tmp/agentim-routing-prefix-test.err
routing_prefix_test_status=$?
set -e
if [ "$routing_prefix_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge routing_reviewer_prefers_higher_priority_rule_when_multiple_match \
  >/tmp/agentim-routing-priority-test.out 2>/tmp/agentim-routing-priority-test.err
routing_priority_test_status=$?
set -e
if [ "$routing_priority_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge readiness_reviewer_enforces_max_session_messages \
  >/tmp/agentim-max-history-test.out 2>/tmp/agentim-max-history-test.err
max_history_test_status=$?
set -e
if [ "$max_history_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge readiness_reviewer_preserves_system_messages_when_trimming_history \
  >/tmp/agentim-system-trim-test.out 2>/tmp/agentim-system-trim-test.err
system_trim_test_status=$?
set -e
if [ "$system_trim_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge readiness_reviewer_exposes_history_summary_after_trimming \
  >/tmp/agentim-summary-trim-test.out 2>/tmp/agentim-summary-trim-test.err
summary_trim_test_status=$?
set -e
if [ "$summary_trim_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge readiness_reviewer_compacts_trimmed_turns_into_turn_pairs \
  >/tmp/agentim-summary-pair-test.out 2>/tmp/agentim-summary-pair-test.err
summary_pair_test_status=$?
set -e
if [ "$summary_pair_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge readiness_reviewer_persists_sessions_between_restarts \
  >/tmp/agentim-persistence-test.out 2>/tmp/agentim-persistence-test.err
persistence_test_status=$?
set -e
if [ "$persistence_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge persistence_reviewer_writes_clean_snapshot_without_temp_artifacts \
  >/tmp/agentim-persistence-clean-test.out 2>/tmp/agentim-persistence-clean-test.err
persistence_clean_test_status=$?
set -e
if [ "$persistence_clean_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge persistence_reviewer_rotates_snapshot_backups \
  >/tmp/agentim-persistence-rotation-test.out 2>/tmp/agentim-persistence-rotation-test.err
persistence_rotation_test_status=$?
set -e
if [ "$persistence_rotation_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge persistence_reviewer_recovers_from_latest_valid_backup \
  >/tmp/agentim-persistence-fallback-test.out 2>/tmp/agentim-persistence-fallback-test.err
persistence_fallback_test_status=$?
set -e
if [ "$persistence_fallback_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge security_reviewer_rejects_missing_secret_and_accepts_valid_secret \
  >/tmp/agentim-security-test.out 2>/tmp/agentim-security-test.err
security_test_status=$?
set -e
if [ "$security_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge security_reviewer_rejects_invalid_signed_webhooks_and_replay \
  >/tmp/agentim-signed-security-test.out 2>/tmp/agentim-signed-security-test.err
signed_security_test_status=$?
set -e
if [ "$signed_security_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge security_reviewer_accepts_telegram_secret_token_only_when_header_matches \
  >/tmp/agentim-telegram-secret-test.out 2>/tmp/agentim-telegram-secret-test.err
telegram_secret_test_status=$?
set -e
if [ "$telegram_secret_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge ops_reviewer_reports_runtime_status_and_review_config \
  >/tmp/agentim-ops-test.out 2>/tmp/agentim-ops-test.err
ops_test_status=$?
set -e
if [ "$ops_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge usability_reviewer_binary_dry_run_exits_cleanly \
  >/tmp/agentim-dry-run-test.out 2>/tmp/agentim-dry-run-test.err
dry_run_test_status=$?
set -e
if [ "$dry_run_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

set +e
cargo test --quiet --test review_bridge usability_reviewer_loads_runtime_config_file \
  >/tmp/agentim-config-test.out 2>/tmp/agentim-config-test.err
config_test_status=$?
set -e
if [ "$config_test_status" -eq 0 ]; then
  dynamic_score=$((dynamic_score + 8))
fi

route_count=$(python3 - <<'PY'
from pathlib import Path
text = Path('src/bot_server.rs').read_text() if Path('src/bot_server.rs').exists() else ''
need = ['/telegram', '/discord', '/feishu', '/qq']
print(sum(1 for item in need if item in text))
PY
)
routing_score=$((routing_score + route_count * 4))

handler_count=$(python3 - <<'PY'
from pathlib import Path
checks = [
    ('src/bots/telegram.rs', 'telegram_webhook_handler'),
    ('src/bots/discord.rs', 'discord_webhook_handler'),
    ('src/bots/feishu.rs', 'feishu_webhook_handler'),
    ('src/bots/qq.rs', 'qq_webhook_handler'),
]
count = 0
for path, needle in checks:
    p = Path(path)
    if p.exists() and needle in p.read_text():
        count += 1
print(count)
PY
)
routing_score=$((routing_score + handler_count * 3))

autocreate_ok=$(python3 - <<'PY'
from pathlib import Path
needles = [
    'find_or_create_session(',
    'agentim.find_or_create_session(',
]
found = 0
for path in Path('src').rglob('*.rs'):
    text = path.read_text()
    if any(needle in text for needle in needles):
        found = 1
        break
print(found)
PY
)
routing_score=$((routing_score + autocreate_ok * 10))

review_artifacts=$(python3 - <<'PY'
from pathlib import Path
count = 0
for path in [Path('autoresearch.md'), Path('README.md'), Path('BOT_INTEGRATION.md')]:
    if path.exists() and 'review' in path.read_text().lower():
        count += 1
print(count)
PY
)
review_score=$((review_score + review_artifacts * 4))

if [ -e tests/review_bridge.rs ] || [ -e src/review.rs ]; then
  review_score=$((review_score + 8))
fi

score=$((dynamic_score + routing_score + review_score))

printf 'METRIC completion_score=%s\n' "$score"
printf 'METRIC dynamic_score=%s\n' "$dynamic_score"
printf 'METRIC routing_score=%s\n' "$routing_score"
printf 'METRIC review_score=%s\n' "$review_score"
printf 'METRIC cargo_test_ok=%s\n' "$cargo_test_ok"
printf 'METRIC help_ok=%s\n' "$help_ok"
printf 'METRIC startup_ok=%s\n' "$startup_ok"
printf 'METRIC route_count=%s\n' "$route_count"
printf 'METRIC handler_count=%s\n' "$handler_count"
printf 'METRIC autocreate_ok=%s\n' "$autocreate_ok"

if [ "$cargo_test_ok" -ne 1 ]; then
  echo '--- cargo test tail ---'
  tail -20 /tmp/agentim-cargo-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-cargo-test.out 2>/dev/null || true
fi

if [ "$help_ok" -ne 1 ]; then
  echo '--- cargo run -- --help tail ---'
  tail -20 /tmp/agentim-help.err 2>/dev/null || true
  tail -20 /tmp/agentim-help.out 2>/dev/null || true
fi

if [ "$startup_ok" -ne 1 ]; then
  echo '--- start.sh dry-run tail ---'
  tail -20 /tmp/agentim-start.err 2>/dev/null || true
  tail -20 /tmp/agentim-start.out 2>/dev/null || true
fi

if [ "$multi_agent_test_status" -ne 0 ]; then
  echo '--- multi-agent review test tail ---'
  tail -20 /tmp/agentim-multi-agent-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-multi-agent-test.out 2>/dev/null || true
fi

if [ "$routing_rule_test_status" -ne 0 ]; then
  echo '--- routing-rule review test tail ---'
  tail -20 /tmp/agentim-routing-rule-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-routing-rule-test.out 2>/dev/null || true
fi

if [ "$reply_target_rule_test_status" -ne 0 ]; then
  echo '--- reply-target routing review test tail ---'
  tail -20 /tmp/agentim-reply-target-rule-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-reply-target-rule-test.out 2>/dev/null || true
fi

if [ "$routing_prefix_test_status" -ne 0 ]; then
  echo '--- routing-prefix review test tail ---'
  tail -20 /tmp/agentim-routing-prefix-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-routing-prefix-test.out 2>/dev/null || true
fi

if [ "$routing_priority_test_status" -ne 0 ]; then
  echo '--- routing-priority review test tail ---'
  tail -20 /tmp/agentim-routing-priority-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-routing-priority-test.out 2>/dev/null || true
fi

if [ "$max_history_test_status" -ne 0 ]; then
  echo '--- max-history review test tail ---'
  tail -20 /tmp/agentim-max-history-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-max-history-test.out 2>/dev/null || true
fi

if [ "$system_trim_test_status" -ne 0 ]; then
  echo '--- system-trim review test tail ---'
  tail -20 /tmp/agentim-system-trim-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-system-trim-test.out 2>/dev/null || true
fi

if [ "$summary_trim_test_status" -ne 0 ]; then
  echo '--- summary-trim review test tail ---'
  tail -20 /tmp/agentim-summary-trim-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-summary-trim-test.out 2>/dev/null || true
fi

if [ "$summary_pair_test_status" -ne 0 ]; then
  echo '--- summary-pair review test tail ---'
  tail -20 /tmp/agentim-summary-pair-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-summary-pair-test.out 2>/dev/null || true
fi

if [ "$persistence_test_status" -ne 0 ]; then
  echo '--- persistence review test tail ---'
  tail -20 /tmp/agentim-persistence-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-persistence-test.out 2>/dev/null || true
fi

if [ "$persistence_clean_test_status" -ne 0 ]; then
  echo '--- persistence clean-snapshot review test tail ---'
  tail -20 /tmp/agentim-persistence-clean-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-persistence-clean-test.out 2>/dev/null || true
fi

if [ "$persistence_rotation_test_status" -ne 0 ]; then
  echo '--- persistence rotation review test tail ---'
  tail -20 /tmp/agentim-persistence-rotation-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-persistence-rotation-test.out 2>/dev/null || true
fi

if [ "$persistence_fallback_test_status" -ne 0 ]; then
  echo '--- persistence fallback review test tail ---'
  tail -20 /tmp/agentim-persistence-fallback-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-persistence-fallback-test.out 2>/dev/null || true
fi

if [ "$security_test_status" -ne 0 ]; then
  echo '--- security review test tail ---'
  tail -20 /tmp/agentim-security-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-security-test.out 2>/dev/null || true
fi

if [ "$signed_security_test_status" -ne 0 ]; then
  echo '--- signed security review test tail ---'
  tail -20 /tmp/agentim-signed-security-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-signed-security-test.out 2>/dev/null || true
fi

if [ "$telegram_secret_test_status" -ne 0 ]; then
  echo '--- telegram native secret review test tail ---'
  tail -20 /tmp/agentim-telegram-secret-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-telegram-secret-test.out 2>/dev/null || true
fi

if [ "$ops_test_status" -ne 0 ]; then
  echo '--- ops review test tail ---'
  tail -20 /tmp/agentim-ops-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-ops-test.out 2>/dev/null || true
fi

if [ "$dry_run_test_status" -ne 0 ]; then
  echo '--- binary dry-run review test tail ---'
  tail -20 /tmp/agentim-dry-run-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-dry-run-test.out 2>/dev/null || true
fi

if [ "$config_test_status" -ne 0 ]; then
  echo '--- runtime config review test tail ---'
  tail -20 /tmp/agentim-config-test.err 2>/dev/null || true
  tail -20 /tmp/agentim-config-test.out 2>/dev/null || true
fi
