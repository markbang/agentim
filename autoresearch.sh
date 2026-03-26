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
  for flag in --telegram-token --discord-token --feishu-token --feishu-app-id --feishu-app-secret --qq-token --qq-bot-id --qq-bot-token --agent --addr; do
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
