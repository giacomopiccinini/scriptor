//! Integration tests for TUI DB operations.

use scriptor::tui::db::connections::init_db;
use scriptor::tui::db::models::{Codex, Folio, Fragmentum, NewCodex, NewFolio, NewFragmentum};

#[tokio::test]
async fn test_codex_crud() {
    let pool = init_db("sqlite::memory:").await.unwrap();

    let codex = Codex::create(
        &pool,
        NewCodex {
            name: "Test Codex".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(codex.name, "Test Codex");
    assert_eq!(codex.ordering, 1);

    let codices = Codex::get_all(&pool).await.unwrap();
    assert_eq!(codices.len(), 1);

    let fetched = Codex::get_by_id(&pool, codex.id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "Test Codex");

    let mut codex = fetched;
    let codex_id = codex.id;
    codex
        .update_name(&pool, "Updated Codex".to_string())
        .await
        .unwrap();
    assert_eq!(codex.name, "Updated Codex");

    codex.delete(&pool).await.unwrap();
    assert!(Codex::get_by_id(&pool, codex_id).await.unwrap().is_none());
}

#[tokio::test]
async fn test_folio_crud() {
    let pool = init_db("sqlite::memory:").await.unwrap();

    let codex = Codex::create(
        &pool,
        NewCodex {
            name: "Project".to_string(),
        },
    )
    .await
    .unwrap();

    let folio = Folio::create(
        &pool,
        NewFolio {
            codex_id: codex.id,
            name: "Recording 1".to_string(),
        },
    )
    .await
    .unwrap();
    assert_eq!(folio.name, "Recording 1");
    assert_eq!(folio.codex_id, codex.id);

    let folia = Folio::get_by_codex_id(&pool, codex.id).await.unwrap();
    assert_eq!(folia.len(), 1);

    let mut folio = folio;
    folio
        .update_name(&pool, "Updated Recording".to_string())
        .await
        .unwrap();
    assert_eq!(folio.name, "Updated Recording");

    folio.delete(&pool).await.unwrap();
}

#[tokio::test]
async fn test_fragmentum_crud() {
    let pool = init_db("sqlite::memory:").await.unwrap();

    let codex = Codex::create(
        &pool,
        NewCodex {
            name: "Project".to_string(),
        },
    )
    .await
    .unwrap();
    let folio = Folio::create(
        &pool,
        NewFolio {
            codex_id: codex.id,
            name: "Recording".to_string(),
        },
    )
    .await
    .unwrap();

    let fragmentum = Fragmentum::create(
        &pool,
        NewFragmentum {
            folio_id: folio.id,
            path: "/tmp/audio.wav".to_string(),
            content: "Hello world".to_string(),
            timestamp_start: Some(0.0),
            timestamp_end: Some(1.5),
        },
    )
    .await
    .unwrap();

    assert_eq!(fragmentum.content, "Hello world");
    assert_eq!(fragmentum.audio_path, "/tmp/audio.wav");
    assert_eq!(fragmentum.timestamp_start, Some(0.0));
    assert_eq!(fragmentum.timestamp_end, Some(1.5));

    let fragmenta = Fragmentum::get_by_folio_id(&pool, folio.id).await.unwrap();
    assert_eq!(fragmenta.len(), 1);

    fragmentum.delete(&pool).await.unwrap();
}
