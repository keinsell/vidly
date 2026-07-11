use diesel::prelude::*;
use diesel::sql_types::Integer;
use diesel::sqlite::SqliteConnection;

use crate::database::model::MovieTag;
use crate::database::schema::{movie_tags, movies, tags};
use crate::object_store;
use crate::tag;

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

pub fn add_tag_to_movie(
    conn: &mut SqliteConnection, movie_id: i32, tag_id: i32,
) -> Result<(), &'static str> {
    let tag_exists = tags::table
        .filter(tags::id.eq(tag_id).and(tags::deleted_at.is_null()))
        .select(tags::id)
        .first::<i32>(conn)
        .optional()
        .map_err(|_| "Database error checking tag existence")?;

    if tag_exists.is_none() {
        return Err("Tag does not exist");
    }

    let link = MovieTag { movie_id, tag_id };

    diesel::insert_into(movie_tags::table)
        .values(&link)
        .execute(conn)
        .map_err(|_| "Database error adding tag to movie")?;

    let movie_title = get_movie(conn, movie_id)
        .ok()
        .flatten()
        .map(|m| m.title)
        .unwrap_or_default();
    let tag_name = tag::get_tag(conn, tag_id)
        .ok()
        .flatten()
        .map(|t| t.name)
        .unwrap_or_default();

    println!(
        "MovieTagAdded: movie_id={} title=\"{}\" tag_id={} tag=\"{}\"",
        movie_id, movie_title, tag_id, tag_name,
    );

    Ok(())
}

pub fn remove_tag_from_movie(
    conn: &mut SqliteConnection, movie_id: i32, tag_id: i32,
) -> Result<(), &'static str> {
    diesel::delete(
        movie_tags::table.filter(
            movie_tags::movie_id
                .eq(movie_id)
                .and(movie_tags::tag_id.eq(tag_id)),
        ),
    )
    .execute(conn)
    .map_err(|_| "Database error removing tag from movie")?;

    let movie_title = get_movie(conn, movie_id)
        .ok()
        .flatten()
        .map(|m| m.title)
        .unwrap_or_default();
    let tag_name = tag::get_tag(conn, tag_id)
        .ok()
        .flatten()
        .map(|t| t.name)
        .unwrap_or_default();

    println!(
        "MovieTagRemoved: movie_id={} title=\"{}\" tag_id={} tag=\"{}\"",
        movie_id, movie_title, tag_id, tag_name,
    );

    Ok(())
}

pub fn list_tags_for_movie(
    conn: &mut SqliteConnection, movie_id: i32,
) -> Result<Vec<tag::Tag>, &'static str> {
    let tag_ids: Vec<i32> = movie_tags::table
        .filter(movie_tags::movie_id.eq(movie_id))
        .select(movie_tags::tag_id)
        .load(conn)
        .map_err(|_| "Database error fetching tag IDs for movie")?;

    if tag_ids.is_empty() {
        return Ok(Vec::new());
    }

    tags::table
        .filter(tags::id.eq_any(tag_ids).and(tags::deleted_at.is_null()))
        .select(tag::Tag::as_select())
        .load::<tag::Tag>(conn)
        .map_err(|_| "Database error fetching tags for movie")
}

pub fn list_movies_for_tag(
    conn: &mut SqliteConnection, tag_id: i32,
) -> Result<Vec<Movie>, &'static str> {
    let movie_ids: Vec<i32> = movie_tags::table
        .filter(movie_tags::tag_id.eq(tag_id))
        .select(movie_tags::movie_id)
        .load(conn)
        .map_err(|_| "Database error fetching movie IDs for tag")?;

    if movie_ids.is_empty() {
        return Ok(Vec::new());
    }

    movies::table
        .filter(movies::id.eq_any(movie_ids))
        .select(Movie::as_select())
        .load::<Movie>(conn)
        .map_err(|_| "Database error fetching movies for tag")
}
