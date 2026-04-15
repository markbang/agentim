use agentim::listener::{InboundListener, ListenerCheckpoint, ListenerRuntimeConfig};
use agentim::Result;
use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct CountingListener {
    count: Arc<AtomicUsize>,
}

#[async_trait]
impl InboundListener for CountingListener {
    fn id(&self) -> &str {
        "counting-listener"
    }

    async fn initialize(&self) -> Result<()> {
        Ok(())
    }

    async fn load_checkpoint(&self) -> Result<ListenerCheckpoint> {
        Ok(ListenerCheckpoint::default())
    }

    async fn run_once(&self, checkpoint: &mut ListenerCheckpoint) -> Result<bool> {
        let next = self.count.fetch_add(1, Ordering::SeqCst) + 1;
        checkpoint.cursor = Some(next.to_string());
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        Ok(true)
    }

    async fn persist_checkpoint(&self, _checkpoint: &ListenerCheckpoint) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
#[ignore]
async fn listener_supervisor_short_soak_smoke() {
    let count = Arc::new(AtomicUsize::new(0));
    let listener = Arc::new(CountingListener {
        count: count.clone(),
    });
    let runtime = ListenerRuntimeConfig {
        max_backoff_ms: 100,
        ..ListenerRuntimeConfig::default()
    };

    let task = tokio::spawn(agentim::listener::run_listener_supervisor(
        listener, runtime,
    ));
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    task.abort();

    assert!(count.load(Ordering::SeqCst) > 5);
}
