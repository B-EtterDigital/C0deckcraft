//! The browser smoke covers the wasm-specific unsupported process id regression.

#[test]
fn recovery_identity_is_stable_and_document_ids_remain_distinct_json_numbers() {
    let owner = deckcraft_engine::recovery::instance_id();
    assert_eq!(owner, deckcraft_engine::recovery::instance_id());
    assert!(owner > 0 && owner < 1 << 53);
    let mut session = deckcraft_engine::Session::new();
    for _ in 0..3 {
        session.execute("file.new", &serde_json::json!({})).unwrap();
    }
    let mut ids: Vec<_> = session.documents().iter().map(|d| d.uid).collect();
    assert!(ids.iter().all(|id| *id > 0 && *id < 1 << 53));
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 3);
}
