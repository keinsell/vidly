pub mod model;
pub mod schema;

use std::path::Path;

use diesel::r2d2::{self, ConnectionManager};
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub type DatabaseConnection = r2d2::Pool<ConnectionManager<SqliteConnection>>;

pub fn create_pool(uri: &Path) -> DatabaseConnection {
    let database_url = uri.to_string_lossy().to_string();
    if let Some(parent) = uri.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create database directory");
    }

    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    r2d2::Pool::builder()
        .build(manager)
        .expect("Could not build database connection pool")
}

pub fn run_migrations(pool: &DatabaseConnection) {
    let mut conn = pool.get().expect("Could not get connection for migrations");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Error running database migrations");
    println!("Database migrations up to date.");
}




