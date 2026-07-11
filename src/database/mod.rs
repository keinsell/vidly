pub mod model;
pub mod schema;

use std::path::Path;

use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::sql_types::Integer;
use diesel::sqlite::SqliteConnection;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

use crate::database::model::movie::Movie;
use crate::database::schema::movies;

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

pub struct SqliteMovieRepository {
    pool: DatabaseConnection,
}

impl SqliteMovieRepository {
    pub fn new(pool: DatabaseConnection) -> Self {
        Self { pool }
    }
}

impl crate::movie::MovieRepository for SqliteMovieRepository {
    fn get_movie(&self, id: i32) -> Result<Option<Movie>, &'static str> {
        let mut conn = self.pool.get().map_err(|_| "Failed to get connection")?;

        movies::table
            .filter(movies::id.eq(id))
            .select(Movie::as_select())
            .first::<Movie>(&mut conn)
            .optional()
            .map_err(|_| "Database error fetching movie")
    }

    fn list_movies(&self) -> Result<Vec<Movie>, &'static str> {
        let mut conn = self.pool.get().map_err(|_| "Failed to get connection")?;

        movies::table
            .select(Movie::as_select())
            .load::<Movie>(&mut conn)
            .map_err(|_| "Database error listing movies")
    }

    fn create_movie(&self, movie: Movie) -> Result<Movie, &'static str> {
        let mut conn = self.pool.get().map_err(|_| "Failed to get connection")?;

        diesel::insert_into(movies::table)
            .values((
                movies::title.eq(&movie.title),
                movies::description.eq(&movie.description),
                movies::thumb.eq(&movie.thumb),
                movies::sources.eq(&movie.sources),
            ))
            .execute(&mut conn)
            .map_err(|_| "Database error creating movie")?;

        let last_id: i32 = diesel::dsl::select(diesel::dsl::sql::<Integer>("last_insert_rowid()"))
            .get_result(&mut conn)
            .map_err(|_| "Database error getting last ID")?;

        movies::table
            .filter(movies::id.eq(last_id))
            .select(Movie::as_select())
            .first::<Movie>(&mut conn)
            .map_err(|_| "Database error fetching created movie")
    }
}


