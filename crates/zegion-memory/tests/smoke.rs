use zegion_memory::{Memory, MemoryConfig, MemoryKind, MemoryStore, EMBEDDING_DIM};

fn test_cfg() -> MemoryConfig {
    MemoryConfig {
        db_path: ":memory:".into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn insert_and_keyword_recall() {
    let store = MemoryStore::open(test_cfg()).await.unwrap();
    let m = Memory::new(MemoryKind::Semantic, "the user prefers dark mode");
    store.insert(&m).await.unwrap();

    let hits = store.recall_keyword("dark mode", 5).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].memory.content.contains("dark mode"));
}

#[tokio::test]
async fn vector_recall_roundtrip() {
    let store = MemoryStore::open(test_cfg()).await.unwrap();
    let m = Memory::new(MemoryKind::Semantic, "embedding test fact");
    store.insert(&m).await.unwrap();

    let mut v = vec![0.0_f32; EMBEDDING_DIM];
    v[0] = 1.0;
    store.set_embedding(m.id, v.clone()).await.unwrap();

    let hits = store.recall_vector(v, 5).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].memory.id, m.id);
}

#[tokio::test]
async fn episodic_window_and_clear() {
    let store = MemoryStore::open(test_cfg()).await.unwrap();
    for i in 0..5 {
        store
            .insert(&Memory::new(MemoryKind::Episodic, format!("turn {i}")))
            .await
            .unwrap();
    }
    let recent = store.recent_episodic(10).await.unwrap();
    assert_eq!(recent.len(), 5);

    let cleared = store.clear_episodic().await.unwrap();
    assert_eq!(cleared, 5);
    assert_eq!(store.count(MemoryKind::Episodic).await.unwrap(), 0);
}
