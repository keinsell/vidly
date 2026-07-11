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

#[test]
fn list_movies_for_tag_returns_movies() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let movies = movie::list_movies_for_tag(&mut conn, 5).expect("Should list movies");
    let movie_ids: Vec<i32> = movies.iter().map(|m| m.id).collect();
    assert!(movie_ids.contains(&1));
    assert!(movie_ids.contains(&2));
    assert!(movie_ids.contains(&3));
    assert!(movie_ids.contains(&4));
}

#[test]
fn list_movies_for_tag_empty_when_no_movies() {
    let pool = pool();
    let mut conn = pool.get().expect("Could not get connection");

    let movies = movie::list_movies_for_tag(&mut conn, 1).expect("Should list movies");
    assert!(movies.is_empty(), "Tag 1 should have no linked movies");
}
