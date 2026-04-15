use agentim::manager::AgentIM;
use agentim::store::{FileSessionStore, MemorySessionStore, SessionStore};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_path(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{}-{}", prefix, nanos))
}

#[tokio::test]
async fn memory_session_store_roundtrip() {
    let store = Arc::new(MemorySessionStore::default());
    let _agentim = AgentIM::with_session_store(store.clone());

    let session = agentim::session::Session::new(
        "default-agent".to_string(),
        "test-channel".to_string(),
        "user-1".to_string(),
    );
    store.save_sessions(vec![session.clone()]).await.unwrap();

    let loaded = store.load_sessions().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, session.id);

    let empty = AgentIM::new();
    assert_eq!(empty.restore_sessions_via_store().await.unwrap(), 0);
}

#[tokio::test]
async fn file_session_store_roundtrip() {
    let path = temp_path("agentim-store-test");
    let store = FileSessionStore::new(&path, 2);

    let session = agentim::session::Session::new(
        "default-agent".to_string(),
        "test-channel".to_string(),
        "user-2".to_string(),
    );
    store.save_sessions(vec![session.clone()]).await.unwrap();

    let loaded = store.load_sessions().await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].id, session.id);

    let _ = std::fs::remove_file(path);
}
