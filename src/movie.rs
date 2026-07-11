use diesel::prelude::*;
use diesel::sql_types::Integer;
use diesel::sqlite::SqliteConnection;

use crate::database::schema::movies;
use crate::object_store;

pub use crate::database::model::movie::{Movie, Sources};

pub fn get_movie(conn: &mut SqliteConnection, id: i32) -> Result<Option<Movie>, &'static str> {
    movies::table
        .filter(movies::id.eq(id))
        .select(Movie::as_select())
        .first::<Movie>(conn)
        .optional()
        .map_err(|_| "Database error fetching movie")
}

pub fn list_movies(conn: &mut SqliteConnection) -> Result<Vec<Movie>, &'static str> {
    movies::table
        .select(Movie::as_select())
        .load::<Movie>(conn)
        .map_err(|_| "Database error listing movies")
}

pub fn create_movie(conn: &mut SqliteConnection, movie: Movie) -> Result<Movie, &'static str> {
    diesel::insert_into(movies::table)
        .values((
            movies::title.eq(&movie.title),
            movies::description.eq(&movie.description),
            movies::thumb.eq(&movie.thumb),
            movies::sources.eq(&movie.sources),
        ))
        .execute(conn)
        .map_err(|_| "Database error creating movie")?;

    let last_id: i32 = diesel::dsl::select(diesel::dsl::sql::<Integer>("last_insert_rowid()"))
        .get_result(conn)
        .map_err(|_| "Database error getting last ID")?;

    movies::table
        .filter(movies::id.eq(last_id))
        .select(Movie::as_select())
        .first::<Movie>(conn)
        .map_err(|_| "Database error fetching created movie")
}

pub async fn upload_movie(
    title: String, description: String, file_bytes: Vec<u8>, file_name: String,
    thumb_bytes: Vec<u8>, thumb_name: String, conn: &mut SqliteConnection,
    object_storage: &dyn object_store::ObjectStore,
) -> Result<Movie, String> {
    use sha2::Digest;

    let video_hash = hex::encode(sha2::Sha256::digest(&file_bytes));
    let video_ext = std::path::Path::new(&file_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4");
    let object_key = format!("uploads/{}.{}", video_hash, video_ext);

    object_storage
        .put_bytes(&object_key, file_bytes)
        .await
        .map_err(|e| format!("Failed to store video: {e}"))?;

    let thumb = if thumb_bytes.is_empty() {
        String::new()
    } else {
        let thumb_hash = hex::encode(sha2::Sha256::digest(&thumb_bytes));
        let thumb_ext = std::path::Path::new(&thumb_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("jpg");
        let thumb_key = format!("thumbnails/{}.{}", thumb_hash, thumb_ext);
        if let Err(e) = object_storage.put_bytes(&thumb_key, thumb_bytes).await {
            eprintln!("Upload warning: failed to store thumbnail '{thumb_key}': {e}");
        }
        format!("/object/{}", thumb_key)
    };

    let movie = Movie {
        id: 0,
        title,
        description,
        thumb,
        sources: Sources(vec![format!("/object/{}", object_key)]),
    };

    let created = create_movie(conn, movie)
        .map_err(|e| format!("Failed to create movie record: {e}"))?;

    println!(
        "MovieUploaded: id={} title=\"{}\" sources={:?}",
        created.id, created.title, created.sources
    );

    Ok(created)
}
