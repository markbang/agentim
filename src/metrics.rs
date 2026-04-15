use once_cell::sync::Lazy;
use prometheus::{
    register_histogram_vec, register_int_counter, register_int_counter_vec, register_int_gauge,
    Encoder, HistogramVec, IntCounter, IntCounterVec, IntGauge, TextEncoder,
};

static WEBHOOK_REQUESTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "agentim_webhook_requests_total",
        "Total webhook requests received by channel",
        &["channel"]
    )
    .expect("register agentim_webhook_requests_total")
});

static WEBHOOK_FAILURES_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "agentim_webhook_failures_total",
        "Total webhook failures by channel and category",
        &["channel", "category"]
    )
    .expect("register agentim_webhook_failures_total")
});

static AGENT_LATENCY_MS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "agentim_agent_latency_ms",
        "Latency of agent requests in milliseconds",
        &["agent_id"],
        vec![5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0]
    )
    .expect("register agentim_agent_latency_ms")
});

static CHANNEL_SEND_LATENCY_MS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "agentim_channel_send_latency_ms",
        "Latency of outbound channel send operations in milliseconds",
        &["channel_id"],
        vec![5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0]
    )
    .expect("register agentim_channel_send_latency_ms")
});

static ACTIVE_SESSIONS: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "agentim_active_sessions",
        "Current number of active sessions"
    )
    .expect("register agentim_active_sessions")
});

static SESSION_CLEANUP_TOTAL: Lazy<IntCounter> = Lazy::new(|| {
    register_int_counter!(
        "agentim_session_cleanup_total",
        "Total number of sessions cleaned up"
    )
    .expect("register agentim_session_cleanup_total")
});

static AUTH_REJECT_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "agentim_auth_reject_total",
        "Total authentication rejections by reason",
        &["reason"]
    )
    .expect("register agentim_auth_reject_total")
});

pub fn inc_webhook_request(channel: &str) {
    WEBHOOK_REQUESTS_TOTAL.with_label_values(&[channel]).inc();
}

pub fn inc_webhook_failure(channel: &str, category: &str) {
    WEBHOOK_FAILURES_TOTAL
        .with_label_values(&[channel, category])
        .inc();
}

pub fn observe_agent_latency(agent_id: &str, ms: f64) {
    AGENT_LATENCY_MS.with_label_values(&[agent_id]).observe(ms);
}

pub fn observe_channel_send_latency(channel_id: &str, ms: f64) {
    CHANNEL_SEND_LATENCY_MS
        .with_label_values(&[channel_id])
        .observe(ms);
}

pub fn set_active_sessions(count: usize) {
    ACTIVE_SESSIONS.set(count as i64);
}

pub fn inc_session_cleanup(count: usize) {
    SESSION_CLEANUP_TOTAL.inc_by(count as u64);
}

pub fn inc_auth_reject(reason: &str) {
    AUTH_REJECT_TOTAL.with_label_values(&[reason]).inc();
}

pub fn gather_text() -> Result<String, prometheus::Error> {
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    TextEncoder::new().encode(&metric_families, &mut buffer)?;
    Ok(String::from_utf8_lossy(&buffer).into_owned())
}
