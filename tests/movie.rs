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
