# AgentIM Production Readiness

## 当前结论

当前主干已经达到“可作为单实例 webhook bridge 投入生产”的基线，但前提很明确：

- 真实流量必须使用 `openai` 或 `acp` agent
- webhook 入口必须至少启用一层鉴权
- 当前定位是单实例进程 + 本地快照持久化，不是分布式控制面

内置 `claude` / `codex` / `pi` 仍然是开发 stub，只用于 dry-run 和本地验证；生产 bot-server 进程会直接拒绝这些配置。

## 已落地的生产保障

- 同一 `(agent, channel, user)` 的 session 现在通过原子索引复用，不会并发重复建会话
- 同一 session 的消息处理已经串行化，避免并发覆盖历史或丢回复
- webhook 支持共享密钥、全局 HMAC 签名、防 replay，以及 Telegram / Discord / Feishu / Slack 的平台原生校验
- Slack 在配置签名密钥后，缺失签名头会直接拒绝，不再默许漏 header
- agent 调用默认有 `30000ms` 超时，避免 webhook 长时间悬挂
- 请求体默认限制为 `256 KiB`
- session 快照改为后台异步落盘，不再在 webhook 请求路径里直接做同步 IO
- 状态文件支持 `.bak.N` 轮转，并可在主文件损坏时从最近有效备份恢复

## 仍然成立的边界

- 目前是单实例内存态 session 管理；如果要多实例部署，需要把 session / routing / replay cache 外置
- 当前默认持久化仍是本地 JSON 快照；如果要更强 durability，建议换数据库或对象存储
- 监控和指标还没有 Prometheus 级别的集成，现阶段主要依赖日志和 `/healthz`、`/reviewz`
- TLS 终止仍建议放在反向代理或 LB 前面

## 建议的生产启动方式

```bash
cargo run -- \
  --agent openai \
  --openai-api-key "$OPENAI_API_KEY" \
  --openai-base-url "${OPENAI_BASE_URL:-https://api.openai.com/v1}" \
  --openai-model "${OPENAI_MODEL:-gpt-4o-mini}" \
  --openai-max-retries 1 \
  --telegram-token "$TELEGRAM_TOKEN" \
  --webhook-secret "change-me" \
  --state-file .agentim/sessions.json \
  --state-backup-count 2 \
  --addr 127.0.0.1:8080
```

如果要接 ACP：

```bash
cargo run -- \
  --agent acp \
  --acp-command /path/to/acp-agent \
  --telegram-token "$TELEGRAM_TOKEN" \
  --webhook-signing-secret "change-me-signing" \
  --state-file .agentim/sessions.json
```

## 上线前检查

- `cargo test`
- `cargo run -- --dry-run ...`
- 确认至少一层 webhook 鉴权已开启
- 确认使用的是 `openai` 或 `acp`
- 确认 `state_file` 和备份目录可写
- 确认反向代理/LB 已启用 HTTPS

## 下一阶段增强

- 外部 session 存储和多实例协调
- Prometheus / OpenTelemetry 指标
- 更细粒度的限流和 backpressure
- 优雅停机时的最终 flush / drain
