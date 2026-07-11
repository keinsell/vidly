use crate::object_store;

pub use crate::database::model::movie::{Movie, Sources};

pub trait MovieRepository: Send + Sync {
    fn get_movie(&self, id: i32) -> Result<Option<Movie>, &'static str>;
    fn list_movies(&self) -> Result<Vec<Movie>, &'static str>;
    fn create_movie(&self, movie: Movie) -> Result<Movie, &'static str>;
}

pub async fn upload_movie(
    title: String, description: String, file_bytes: Vec<u8>, file_name: String,
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
