use agentim::listener::{load_checkpoint_from_path, save_checkpoint_to_path, ListenerCheckpoint};
use std::path::PathBuf;

fn temp_checkpoint_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "agentim-listener-checkpoint-{}.json",
        uuid::Uuid::new_v4()
    ))
}

#[tokio::test]
async fn listener_checkpoint_roundtrip_persists_cursor() {
    let path = temp_checkpoint_path();
    let checkpoint = ListenerCheckpoint {
        cursor: Some("123".to_string()),
    };

    save_checkpoint_to_path(Some(path.clone()), &checkpoint)
        .await
        .unwrap();
    let loaded = load_checkpoint_from_path::<ListenerCheckpoint>(Some(path.clone()))
        .await
        .unwrap();

    assert_eq!(loaded.cursor.as_deref(), Some("123"));
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn listener_checkpoint_corruption_falls_back_to_default() {
    let path = temp_checkpoint_path();
    std::fs::write(&path, "not valid json").unwrap();

    let loaded = load_checkpoint_from_path::<ListenerCheckpoint>(Some(path.clone()))
        .await
        .unwrap();

    assert_eq!(loaded.cursor, None);
    let corrupt_files = std::fs::read_dir(path.parent().unwrap())
        .unwrap()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert!(corrupt_files.iter().any(|name| name.contains("corrupt")));

    for entry in corrupt_files {
        if entry.contains("agentim-listener-checkpoint") {
            let _ = std::fs::remove_file(path.parent().unwrap().join(entry));
        }
    }
}
