mod config;
pub mod database;
mod movie;
pub mod object_store;
mod web;

use std::sync::Arc;

use web::ApplicationState;

async fn bootstrap() -> ApplicationState {
    let db = database::create_pool(std::path::Path::new(&*config::DATABASE_URL));
    database::run_migrations(&db);

    let movie_repo = Arc::new(database::SqliteMovieRepository::new(db));
    let object_storage = Arc::new(
        object_store::FileBackedObjectStore::new(
            object_store::FileBackedObjectStore::default_path(),
        )
            .expect("Failed to create object storage"),
    );

    let app_state = ApplicationState {
        movie_repo,
        object_store: object_storage,
    };

    app_state
}

#[tokio::main]
async fn main() {
    let app_state = bootstrap().await;

    println!(
        "Application started using SQLite database at {}",
        *config::DATABASE_URL
    );

    let app = web::router(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:80")
        .await
        .expect("failed to bind to :80");

    println!("Server listening on :80");

    axum::serve(listener, app).await.expect("server error");
}
