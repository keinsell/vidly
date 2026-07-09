use crate::object_store;

pub use crate::database::model::movie::{Movie, Sources};

pub trait MovieRepository: Send + Sync {
    fn get_movie(&self, id: i32) -> Result<Option<Movie>, &'static str>;
    fn list_movies(&self) -> Result<Vec<Movie>, &'static str>;
    fn create_movie(&self, movie: Movie) -> Result<Movie, &'static str>;
    fn delete_movie(&self, id: i32) -> Result<bool, &'static str>;
}

pub async fn upload_movie(
    title: String, description: String, subtitle: String, file_bytes: Vec<u8>, file_name: String,
    thumb_bytes: Vec<u8>, thumb_name: String, repo: &dyn MovieRepository,
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
        subtitle,
        thumb,
        sources: Sources(vec![format!("/object/{}", object_key)]),
    };

    let created = repo
        .create_movie(movie)
        .map_err(|e| format!("Failed to create movie record: {e}"))?;

    println!(
        "MovieUploaded: id={} title=\"{}\" sources={:?}",
        created.id, created.title, created.sources
    );

    Ok(created)
}

pub async fn delete_movie(
    id: i32, repo: &dyn MovieRepository,
    object_storage: &dyn object_store::ObjectStore,
) -> Result<bool, String> {
    let movie = repo
        .get_movie(id)
        .map_err(|e| format!("Failed to fetch movie: {e}"))?;

    let movie = match movie {
        | Some(m) => m,
        | None => return Ok(false),
    };

    for source in &movie.sources {
        let object_key = source.strip_prefix("/object/").unwrap_or(source);
        if let Err(e) = object_storage.delete_object(object_key).await {
            eprintln!("Delete warning: failed to delete object '{object_key}': {e}");
        }
    }

    if !movie.thumb.is_empty() {
        let thumb_key = movie.thumb.strip_prefix("/object/").unwrap_or(&movie.thumb);
        if let Err(e) = object_storage.delete_object(thumb_key).await {
            eprintln!("Delete warning: failed to delete thumbnail '{thumb_key}': {e}");
        }
    }

    let deleted = repo
        .delete_movie(id)
        .map_err(|e| format!("Failed to delete movie record: {e}"))?;

    if deleted {
        println!(
            "MovieDeleted: id={} title=\"{}\" sources={:?}",
            id, movie.title, movie.sources
        );
    }

    Ok(deleted)
}
