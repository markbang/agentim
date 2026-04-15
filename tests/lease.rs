use agentim::lease::{FileLeaseStore, LeaseStore};
use chrono::{Duration, Utc};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("agentim-lease-{}", nanos))
}

#[tokio::test]
async fn file_lease_store_allows_single_owner_and_failover() {
    let dir = temp_dir();
    let store = FileLeaseStore::new(dir.clone());

    assert!(store.try_acquire("telegram", "owner-a", 60).await.unwrap());
    assert!(!store.try_acquire("telegram", "owner-b", 60).await.unwrap());

    assert!(store.renew("telegram", "owner-a", 60).await.unwrap());
    store.release("telegram", "owner-a").await.unwrap();
    assert!(store.try_acquire("telegram", "owner-b", 60).await.unwrap());

    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn file_lease_store_allows_takeover_after_expiry() {
    let dir = temp_dir();
    let store = FileLeaseStore::new(dir.clone());

    let lease_path = dir.join("discord.lease.json");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        &lease_path,
        serde_json::json!({
            "owner": "owner-a",
            "expires_at": (Utc::now() - Duration::seconds(30)).to_rfc3339(),
        })
        .to_string(),
    )
    .unwrap();

    assert!(store.try_acquire("discord", "owner-b", 60).await.unwrap());

    let _ = std::fs::remove_dir_all(dir);
}
