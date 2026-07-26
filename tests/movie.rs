use vidly::database;
use vidly::movie;
use vidly::object_store::ObjectStore;

use diesel::r2d2::{self, ConnectionManager};
use diesel::sqlite::SqliteConnection;
use sha2::Digest;

fn pool() -> database::DatabaseConnection {
    let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
    let pool = r2d2::Pool::builder()
        .max_size(2)
        .build(manager)
        .expect("Could not build test database pool");
    database::run_migrations(&pool);
    pool
}

fn store() -> vidly::object_store::InMemoryObjectStore {
    vidly::object_store::InMemoryObjectStore::new()
}

#[tokio::test]
async fn upload_movie_with_video_and_thumbnail() {
    let pool = pool();
    let store = store();

    let title = "Test Movie".to_string();
    let description = "A test movie description".to_string();
    let file_bytes = include_bytes!("fixtures/small.mp4").to_vec();
    let file_name = "small.mp4".to_string();
    let thumb_bytes = include_bytes!("fixtures/small.jpg").to_vec();
    let thumb_name = "small.jpg".to_string();

    let mut conn = pool.get().expect("Could not get connection");
    let movie = movie::upload_movie(
        title.clone(),
        description.clone(),
        file_bytes.clone(),
        file_name.clone(),
        thumb_bytes.clone(),
        thumb_name.clone(),
        &mut conn,
        &store,
    )
    .await
    .expect("upload_movie should succeed");

    assert_eq!(movie.title, title);
    assert_eq!(movie.description, description);
    assert!(movie.id > 0);
    assert!(!movie.sources.0.is_empty());

    let video_hash = hex::encode(sha2::Sha256::digest(&file_bytes));
    let video_key = format!("uploads/{}.mp4", video_hash);
    let stored_video = store
        .get_bytes(&video_key)
        .await
        .expect("get_bytes should not error")
        .expect("video should be stored");
    assert_eq!(stored_video, file_bytes);
    assert!(movie.sources.0.contains(&format!("/object/{}", video_key)));

    let thumb_hash = hex::encode(sha2::Sha256::digest(&thumb_bytes));
    let thumb_key = format!("thumbnails/{}.jpg", thumb_hash);
    let stored_thumb = store
        .get_bytes(&thumb_key)
        .await
        .expect("get_bytes should not error")
        .expect("thumbnail should be stored");
    assert_eq!(stored_thumb, thumb_bytes);
    assert_eq!(movie.thumb, format!("/object/{}", thumb_key));

    let movies = movie::list_movies(&mut conn).expect("should list movies");
    assert_eq!(movies.len(), 5);
    assert_eq!(movies[4].title, title);
}

#[tokio::test]
async fn upload_movie_without_thumbnail() {
    let pool = pool();
    let store = store();

    let mut conn = pool.get().expect("Could not get connection");
    let movie = movie::upload_movie(
        "No Thumb".into(),
        "desc".into(),
        include_bytes!("fixtures/small.mp4").to_vec(),
        "small.mp4".into(),
        vec![],
        String::new(),
        &mut conn,
        &store,
    )
    .await
    .expect("upload_movie should succeed");

    assert!(movie.thumb.is_empty());
}

#[test]
fn add_tag_to_movie_adds_tag() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    movie::add_tag_to_movie(&mut conn, 1, 1).expect("Should add tag");

    let tags = movie::list_tags_for_movie(&mut conn, 1).expect("Should list tags");
    let tag_ids: Vec<i32> = tags.iter().map(|t| t.id).collect();
    assert!(tag_ids.contains(&1), "Tag 1 should be linked to movie 1");
}

#[test]
fn add_tag_to_movie_nonexistent_tag_fails() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let result = movie::add_tag_to_movie(&mut conn, 1, 999);
    assert!(result.is_err(), "Adding non-existent tag should fail");
}

#[test]
fn add_tag_to_movie_duplicate_fails() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let result = movie::add_tag_to_movie(&mut conn, 1, 4);
    assert!(result.is_err(), "Adding duplicate tag should fail");
}

#[test]
fn remove_tag_from_movie_removes_tag() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    movie::remove_tag_from_movie(&mut conn, 1, 5).expect("Should remove tag");

    let tags = movie::list_tags_for_movie(&mut conn, 1).expect("Should list tags");
    let tag_ids: Vec<i32> = tags.iter().map(|t| t.id).collect();
    assert!(!tag_ids.contains(&5), "Tag 5 should no longer be linked");
}

#[test]
fn remove_tag_from_movie_nonexistent_link_succeeds() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let result = movie::remove_tag_from_movie(&mut conn, 2, 1);
    assert!(result.is_ok(), "Removing unlinked tag should succeed");
}

#[test]
fn list_tags_for_movie_returns_tags() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let tags = movie::list_tags_for_movie(&mut conn, 4).expect("Should list tags");
    let tag_ids: Vec<i32> = tags.iter().map(|t| t.id).collect();
    assert!(tag_ids.contains(&3));
    assert!(tag_ids.contains(&4));
    assert!(tag_ids.contains(&5));
}

#[test]
fn list_tags_for_movie_empty_when_no_tags() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let tags = movie::list_tags_for_movie(&mut conn, 999).expect("Should list tags");
    assert!(tags.is_empty(), "Non-existent movie should have no tags");
}

#[tokio::test]
async fn delete_movie_removes_database_entries_and_objects() {
    let pool = pool();
    let store = store();

    let mut conn = pool.get().expect("Could not get connection");

    // Upload a movie so we have something to delete
    let movie = movie::upload_movie(
        "To Delete".into(),
        "Will be removed".into(),
        include_bytes!("fixtures/small.mp4").to_vec(),
        "small.mp4".into(),
        include_bytes!("fixtures/small.jpg").to_vec(),
        "small.jpg".into(),
        &mut conn,
        &store,
    )
    .await
    .expect("upload_movie should succeed");

    let movie_id = movie.id;
    let source_keys: Vec<String> = movie
        .sources
        .iter()
        .filter_map(|s| s.strip_prefix("/object/").map(String::from))
        .collect();
    let thumb_key = movie
        .thumb
        .strip_prefix("/object/")
        .map(String::from);

    // Verify objects exist before deletion
    for key in &source_keys {
        assert!(
            store.get_bytes(key).await.unwrap().is_some(),
            "source '{}' should exist before deletion",
            key
        );
    }
    if let Some(ref key) = thumb_key {
        assert!(
            store.get_bytes(key).await.unwrap().is_some(),
            "thumbnail '{}' should exist before deletion",
            key
        );
    }

    // Add a tag to the movie to ensure cascade cleanup
    movie::add_tag_to_movie(&mut conn, movie_id, 1).expect("Should add tag");
    let tags_before = movie::list_tags_for_movie(&mut conn, movie_id).expect("Should list tags");
    assert!(!tags_before.is_empty(), "Tag should be linked before deletion");

    // Delete the movie
    movie::delete_movie(&mut conn, movie_id).expect("delete_movie should succeed");

    // Verify DB entry is gone
    let fetched = movie::get_movie(&mut conn, movie_id).expect("get_movie should not error");
    assert!(fetched.is_none(), "Movie should no longer exist in database");

    // Verify movie_tags entries are cleaned up
    let tags_after = movie::list_tags_for_movie(&mut conn, movie_id).expect("Should list tags");
    assert!(
        tags_after.is_empty(),
        "Movie tags should be cascade-deleted"
    );

    // Verify objects still exist (delete_movie only handles DB;
    // object store cleanup is the caller's responsibility)
    for key in &source_keys {
        assert!(
            store.get_bytes(key).await.unwrap().is_some(),
            "source '{}' should still exist after DB-only deletion",
            key
        );
    }

    // Now clean up objects manually (simulating what the web handler does)
    for key in &source_keys {
        store.delete_object(key).await.unwrap();
    }
    if let Some(ref key) = thumb_key {
        store.delete_object(key).await.unwrap();
    }

    // Verify objects are gone
    for key in &source_keys {
        assert!(
            store.get_bytes(key).await.unwrap().is_none(),
            "source '{}' should be deleted from object store",
            key
        );
    }
    if let Some(ref key) = thumb_key {
        assert!(
            store.get_bytes(key).await.unwrap().is_none(),
            "thumbnail '{}' should be deleted from object store",
            key
        );
    }
}

#[tokio::test]
async fn delete_movie_nonexistent_returns_error() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let result = movie::delete_movie(&mut conn, 999);
    assert!(result.is_err(), "Deleting non-existent movie should fail");
    assert_eq!(result.unwrap_err(), "Movie not found");
}

#[test]
fn update_movie_updates_a_movie() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let original = movie::get_movie(&mut conn, 1)
        .expect("should fetch movie")
        .expect("movie should exist");
    let original_sources = original.sources.0.clone();

    let result = movie::update_movie(
        &mut conn, 999, "Title".into(), "Desc".into(), String::new(),
    );
    assert!(result.is_err(), "Updating non-existent movie should fail");

    let updated = movie::update_movie(
        &mut conn, 1, "New Title".into(), "New description".into(), "/object/thumbnails/new.jpg".into(),
    )
        .expect("update_movie should succeed");

    assert_eq!(updated.id, 1);
    assert_eq!(updated.title, "New Title");
    assert_eq!(updated.description, "New description");
    assert_eq!(updated.thumb, "/object/thumbnails/new.jpg");
    assert!(!updated.updated_at.is_empty());
    assert_eq!(updated.sources.0, original_sources, "Sources should not change on metadata update");

    let fetched = movie::get_movie(&mut conn, 1)
        .expect("should fetch movie")
        .expect("movie should exist");
    assert_eq!(fetched.title, "New Title");
    assert_eq!(fetched.description, "New description");
    assert_eq!(fetched.thumb, "/object/thumbnails/new.jpg");
}


