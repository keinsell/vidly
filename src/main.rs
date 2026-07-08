mod config;

use std::sync::Arc;

use askama::Template;
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json},
    routing::get,
};
use serde::Deserialize;

pub mod object_store {
    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use futures_util::StreamExt;
    use object_store::ObjectStore as ObjectStoreBackend;
    use object_store::ObjectStoreExt;
    use object_store::PutPayload;
    use object_store::local::LocalFileSystem;
    use object_store::path::Path as ObjectStorePath;

    /// Object storage is intentionally separate from the repository layer.
    /// The repository works with domain records while this store handles raw binary payloads.
    #[async_trait::async_trait]
    pub trait ObjectStore: Send + Sync {
        async fn put_bytes(
            &self,
            object_key: &str,
            bytes: Vec<u8>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

        async fn get_bytes(
            &self,
            object_key: &str,
        ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>>;

        async fn list_objects(
            &self,
            prefix: Option<&str>,
        ) -> Result<Vec<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>>;

        async fn delete_object(
            &self,
            object_key: &str,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

        async fn head_object(
            &self,
            object_key: &str,
        ) -> Result<Option<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>>;
    }

    #[derive(Clone, Debug, serde::Serialize)]
    pub struct ObjectMetadata {
        pub key: String,
        pub size: u64,
        pub last_modified: Option<String>,
    }

    #[derive(Clone)]
    pub struct FileBackedObjectStore {
        inner: Arc<dyn ObjectStoreBackend>,
        base_dir: PathBuf,
    }

    impl FileBackedObjectStore {
        pub fn new(
            base_dir: impl Into<PathBuf>,
        ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
            let base_dir = base_dir.into();
            fs::create_dir_all(&base_dir)?;
            let inner: Arc<dyn ObjectStoreBackend> =
                Arc::new(LocalFileSystem::new_with_prefix(&base_dir)?);

            Ok(Self { inner, base_dir })
        }

        pub fn default_path() -> PathBuf {
            crate::config::OBJECTS_DIR.to_path_buf()
        }

        pub fn base_dir(&self) -> &Path {
            &self.base_dir
        }
    }

    #[async_trait::async_trait]
    impl ObjectStore for FileBackedObjectStore {
        async fn put_bytes(
            &self,
            object_key: &str,
            bytes: Vec<u8>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let path = ObjectStorePath::from(object_key);
            self.inner
                .put(&path, PutPayload::from_bytes(bytes.into()))
                .await?;
            Ok(())
        }

        async fn get_bytes(
            &self,
            object_key: &str,
        ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
            let path = ObjectStorePath::from(object_key);
            match self.inner.get(&path).await {
                Ok(result) => Ok(Some(result.bytes().await?.to_vec())),
                Err(object_store::Error::NotFound { .. }) => Ok(None),
                Err(err) => Err(Box::new(err)),
            }
        }

        async fn list_objects(
            &self,
            prefix: Option<&str>,
        ) -> Result<Vec<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>> {
            let prefix = prefix.map(ObjectStorePath::from).unwrap_or_default();
            let mut results = Vec::new();
            let mut stream = self.inner.list(Some(&prefix));
            while let Some(result) = stream.next().await {
                let meta = result?;
                results.push(ObjectMetadata {
                    key: meta.location.to_string(),
                    size: meta.size,
                    last_modified: Some(meta.last_modified.to_rfc3339()),
                });
            }
            Ok(results)
        }

        async fn delete_object(
            &self,
            object_key: &str,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            let path = ObjectStorePath::from(object_key);
            self.inner.delete(&path).await?;
            Ok(())
        }

        async fn head_object(
            &self,
            object_key: &str,
        ) -> Result<Option<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>> {
            let path = ObjectStorePath::from(object_key);
            match self.inner.head(&path).await {
                Ok(meta) => Ok(Some(ObjectMetadata {
                    key: meta.location.to_string(),
                    size: meta.size,
                    last_modified: Some(meta.last_modified.to_rfc3339()),
                })),
                Err(object_store::Error::NotFound { .. }) => Ok(None),
                Err(err) => Err(Box::new(err)),
            }
        }
    }

    pub struct InMemoryObjectStore {
        data: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl InMemoryObjectStore {
        pub fn new() -> Self {
            Self {
                data: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl ObjectStore for InMemoryObjectStore {
        async fn put_bytes(
            &self,
            object_key: &str,
            bytes: Vec<u8>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.data
                .lock()
                .unwrap()
                .insert(object_key.to_string(), bytes);
            Ok(())
        }

        async fn get_bytes(
            &self,
            object_key: &str,
        ) -> Result<Option<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>> {
            Ok(self.data.lock().unwrap().get(object_key).cloned())
        }

        async fn list_objects(
            &self,
            prefix: Option<&str>,
        ) -> Result<Vec<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>> {
            let prefix = prefix.unwrap_or("");
            let data = self.data.lock().unwrap();
            let results: Vec<ObjectMetadata> = data
                .iter()
                .filter(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| ObjectMetadata {
                    key: k.clone(),
                    size: v.len() as u64,
                    last_modified: None,
                })
                .collect();
            Ok(results)
        }

        async fn delete_object(
            &self,
            object_key: &str,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.data.lock().unwrap().remove(object_key);
            Ok(())
        }

        async fn head_object(
            &self,
            object_key: &str,
        ) -> Result<Option<ObjectMetadata>, Box<dyn std::error::Error + Send + Sync>> {
            let data = self.data.lock().unwrap();
            Ok(data.get(object_key).map(|v| ObjectMetadata {
                key: object_key.to_string(),
                size: v.len() as u64,
                last_modified: None,
            }))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn default_path_uses_project_data_directory() {
            let path = FileBackedObjectStore::default_path();
            let rendered = path.to_string_lossy();

            assert!(rendered.ends_with("objects"));
            assert!(rendered.contains("vidly"));
        }

        #[tokio::test]
        async fn puts_and_reads_bytes_from_disk() {
            let temp_dir =
                std::env::temp_dir().join(format!("vidly-object-store-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&temp_dir);

            let store = FileBackedObjectStore::new(&temp_dir).unwrap();
            let payload = vec![0xde, 0xad, 0xbe, 0xef];

            store
                .put_bytes("sample.bin", payload.clone())
                .await
                .unwrap();
            let retrieved = store.get_bytes("sample.bin").await.unwrap().unwrap();

            assert_eq!(retrieved, payload);
        }

        #[tokio::test]
        async fn puts_and_reads_bytes_in_memory() {
            let store = InMemoryObjectStore::new();
            let payload = vec![0xca, 0xfe, 0xba, 0xbe];

            store
                .put_bytes("sample.bin", payload.clone())
                .await
                .unwrap();
            let retrieved = store.get_bytes("sample.bin").await.unwrap().unwrap();

            assert_eq!(retrieved, payload);
        }
    }
}

mod movie {
    #[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
    pub struct Movie {
        pub id: u32,
        pub title: String,
        pub description: String,
        pub subtitle: String,
        pub thumb: String,
        pub sources: Vec<String>,
    }

    pub mod repository {
        use std::sync::Mutex;

        use super::Movie;

        pub trait MovieRepository: Send + Sync {
            fn get_movie(&self, id: u32) -> Result<Option<Movie>, &'static str>;
            fn list_movies(&self) -> Result<Vec<Movie>, &'static str>;
            fn create_movie(&self, movie: Movie) -> Result<Movie, &'static str>;
        }

        pub struct InMemoryMovieRepository {
            movies: Mutex<Vec<Movie>>,
        }

        impl InMemoryMovieRepository {
            pub fn new() -> Self {
                Self {
                    movies: Mutex::new(vec![
                        Movie {
                            id: 1,
                            title: "Big Buck Bunny".to_string(),
                            description: "Big Buck Bunny tells the story of a giant rabbit with a heart bigger than himself.".to_string(),
                            subtitle: "By Blender Foundation".to_string(),
                            thumb: "https://upload.wikimedia.org/wikipedia/commons/c/c5/Big_buck_bunny_poster_big.jpg".to_string(),
                            sources: vec!["https://download.blender.org/peach/bigbuckbunny_movies/BigBuckBunny_320x180.mp4".to_string()],
                        },
                        Movie {
                            id: 2,
                            title: "Elephants Dream".to_string(),
                            description: "The first Blender Open Movie from 2006".to_string(),
                            subtitle: "By Blender Foundation".to_string(),
                            thumb: "https://upload.wikimedia.org/wikipedia/commons/0/0c/ElephantsDreamPoster.jpg".to_string(),
                            sources: vec!["https://download.blender.org/ED/elephantsdream-480-h264-st-aac.mov".to_string()],
                        },
                        Movie {
                            id: 3,
                            title: "Sintel".to_string(),
                            description: "An independently produced short film by Blender Foundation.".to_string(),
                            subtitle: "By Blender Foundation".to_string(),
                            thumb: "https://upload.wikimedia.org/wikipedia/commons/8/8f/Sintel_poster.jpg".to_string(),
                            sources: vec!["https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4".to_string()],
                        },
                        Movie {
                            id: 4,
                            title: "Tears of Steel".to_string(),
                            description: "A crowd-funded sci-fi film realized with Blender.".to_string(),
                            subtitle: "By Blender Foundation".to_string(),
                            thumb: "https://upload.wikimedia.org/wikipedia/commons/7/70/Tos-poster.png".to_string(),
                            sources: vec!["https://download.blender.org/demo/movies/tears-of-steel_teaser.mp4".to_string()],
                        },
                    ]),
                }
            }
        }

        impl MovieRepository for InMemoryMovieRepository {
            fn get_movie(&self, id: u32) -> Result<Option<Movie>, &'static str> {
                let movies = self.movies.lock().unwrap();
                Ok(movies.iter().find(|movie| movie.id == id).cloned())
            }

            fn list_movies(&self) -> Result<Vec<Movie>, &'static str> {
                let movies = self.movies.lock().unwrap();
                Ok(movies.clone())
            }

            fn create_movie(&self, mut movie: Movie) -> Result<Movie, &'static str> {
                let mut movies = self.movies.lock().unwrap();
                let max_id = movies.iter().map(|m| m.id).max().unwrap_or(0);
                movie.id = max_id + 1;
                movies.push(movie.clone());
                Ok(movie)
            }
        }
    }

    pub fn get_movie(
        id: u32,
        movie_repository: &dyn repository::MovieRepository,
    ) -> Result<Option<Movie>, &'static str> {
        movie_repository.get_movie(id)
    }

    pub fn list_movies(
        movie_repository: &dyn repository::MovieRepository,
    ) -> Result<Vec<Movie>, &'static str> {
        movie_repository.list_movies()
    }

    pub fn add_movie(
        movie: Movie,
        movie_repository: &dyn repository::MovieRepository,
    ) -> Result<Movie, &'static str> {
        movie_repository.create_movie(movie)
    }

    pub async fn upload_movie(
        title: String,
        description: String,
        subtitle: String,
        file_bytes: Vec<u8>,
        file_name: String,
        thumb_bytes: Vec<u8>,
        thumb_name: String,
        movie_repository: &dyn repository::MovieRepository,
        object_storage: &dyn crate::object_store::ObjectStore,
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
            sources: vec![format!("/object/{}", object_key)],
        };

        let created = movie_repository
            .create_movie(movie)
            .map_err(|e| format!("Failed to create movie record: {e}"))?;

        println!(
            "MovieUploaded: id={} title=\"{}\" sources={:?}",
            created.id, created.title, created.sources
        );

        Ok(created)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn get_movie_reads_from_repository() {
            let repo = repository::InMemoryMovieRepository::new();
            let movie = get_movie(2, &repo).unwrap().unwrap();

            assert_eq!(movie.id, 2);
            assert_eq!(movie.title, "Elephants Dream");
        }

        #[test]
        fn list_movies_reads_from_repository() {
            let repo = repository::InMemoryMovieRepository::new();
            let movies = list_movies(&repo).unwrap();

            assert_eq!(movies.len(), 4);
            assert_eq!(movies[0].title, "Big Buck Bunny");
            assert_eq!(movies[3].title, "Tears of Steel");
        }

        #[test]
        fn create_movie_adds_to_repository_and_assigns_id() {
            let repo = repository::InMemoryMovieRepository::new();
            let movie = Movie {
                id: 0,
                title: "Test Movie".to_string(),
                description: "A test".to_string(),
                subtitle: "Tester".to_string(),
                thumb: String::new(),
                sources: vec!["/object/uploads/test.mp4".to_string()],
            };

            let created = add_movie(movie, &repo).unwrap();
            assert_eq!(created.id, 5);
            assert_eq!(created.title, "Test Movie");

            let all = list_movies(&repo).unwrap();
            assert_eq!(all.len(), 5);
            assert_eq!(all[4].id, 5);
        }
    }
}

#[derive(Clone)]
struct AppState {
    movie_repository: Arc<dyn movie::repository::MovieRepository>,
    object_store: Arc<dyn object_store::ObjectStore>,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    movies: Vec<movie::Movie>,
}

#[derive(Template)]
#[template(path = "watch.html")]
struct WatchTemplate {
    movie: movie::Movie,
}

#[derive(Template)]
#[template(path = "upload.html")]
struct UploadTemplate;

async fn index(State(state): State<AppState>) -> impl IntoResponse {
    let movies = movie::list_movies(state.movie_repository.as_ref()).unwrap_or_default();
    let tpl = IndexTemplate { movies };

    match tpl.render() {
        Ok(body) => Html(body).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response(),
    }
}

async fn watch_movie(Path(id): Path<u32>, State(state): State<AppState>) -> impl IntoResponse {
    match movie::get_movie(id, state.movie_repository.as_ref()) {
        Ok(Some(movie)) => {
            let tpl = WatchTemplate { movie };
            match tpl.render() {
                Ok(body) => Html(body).into_response(),
                Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response(),
            }
        }
        Ok(None) | Err(_) => {
            (StatusCode::NOT_FOUND, Html("<h1>404 Not Found</h1>")).into_response()
        }
    }
}

async fn list_movies(State(state): State<AppState>) -> impl IntoResponse {
    match movie::list_movies(state.movie_repository.as_ref()) {
        Ok(movies) => (StatusCode::OK, serde_json::to_string(&movies).unwrap()).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error listing movies").into_response(),
    }
}

async fn get_object(Path(path): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    match state.object_store.get_bytes(&path).await {
        Ok(Some(bytes)) => {
            let mime = mime_guess::from_path(&path)
                .first_or_octet_stream()
                .to_string();
            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime)],
                bytes,
            )
                .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "object not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error retrieving object").into_response(),
    }
}

#[derive(Deserialize)]
struct ListObjectsQuery {
    prefix: Option<String>,
}

async fn list_objects(
    Query(query): Query<ListObjectsQuery>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state
        .object_store
        .list_objects(query.prefix.as_deref())
        .await
    {
        Ok(objects) => (StatusCode::OK, Json(objects)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error listing objects").into_response(),
    }
}

async fn delete_object(
    Path(path): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.object_store.delete_object(&path).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error deleting object").into_response(),
    }
}

async fn head_object(Path(path): Path<String>, State(state): State<AppState>) -> impl IntoResponse {
    match state.object_store.head_object(&path).await {
        Ok(Some(meta)) => {
            let mut response = StatusCode::OK.into_response();
            response.headers_mut().insert(
                axum::http::header::CONTENT_LENGTH,
                axum::http::HeaderValue::from_str(&meta.size.to_string()).unwrap(),
            );
            if let Some(last_modified) = meta.last_modified {
                response.headers_mut().insert(
                    axum::http::header::LAST_MODIFIED,
                    axum::http::HeaderValue::from_str(&last_modified).unwrap(),
                );
            }
            response
        }
        Ok(None) => (StatusCode::NOT_FOUND, "object not found").into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "error retrieving object metadata",
        )
            .into_response(),
    }
}

// TODO(keinsell): Upon major release this functionality could not have a place and be released as it allows overriding content inside application without permission handling.
async fn put_object(
    Path(_path): Path<String>,
    State(state): State<AppState>,
    body: Bytes,
) -> impl IntoResponse {
    use sha2::Digest;

    let hash = hex::encode(sha2::Sha256::digest(&body));
    match state.object_store.put_bytes(&hash, body.to_vec()).await {
        Ok(_) => (StatusCode::CREATED, hash).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error storing object").into_response(),
    }
}

async fn upload_form() -> impl IntoResponse {
    match UploadTemplate.render() {
        Ok(body) => Html(body).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response(),
    }
}

struct MovieUploadPayload {
    title: String,
    description: String,
    subtitle: String,
    file_name: String,
    file_bytes: Vec<u8>,
    thumb_name: String,
    thumb_bytes: Vec<u8>,
}

impl MovieUploadPayload {
    async fn from_multipart(headers: &HeaderMap, body: Bytes) -> Result<Self, (StatusCode, &'static str)> {
        let boundary = multer::parse_boundary(
            headers
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or(""),
        )
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid multipart Content-Type"))?;

        let constraints = multer::Constraints::new()
            .size_limit(multer::SizeLimit::new().per_field(500 * 1024 * 1024));

        use futures_util::stream;
        let stream = stream::once(async { Ok::<_, multer::Error>(body) });
        let mut multipart = multer::Multipart::with_constraints(stream, &boundary, constraints);

        let mut payload = MovieUploadPayload {
            title: String::new(),
            description: String::new(),
            subtitle: String::new(),
            file_name: String::new(),
            file_bytes: Vec::new(),
            thumb_name: String::new(),
            thumb_bytes: Vec::new(),
        };

        while let Ok(Some(field)) = multipart.next_field().await {
            let name = field.name().unwrap_or("").to_string();
            let original_file_name = field.file_name().unwrap_or("video.mp4").to_string();
            let data = field.bytes().await.map_err(|_| (StatusCode::BAD_REQUEST, "Failed to read upload field"))?;

            match name.as_str() {
                "title" => payload.title = String::from_utf8_lossy(&data).to_string(),
                "description" => payload.description = String::from_utf8_lossy(&data).to_string(),
                "subtitle" => payload.subtitle = String::from_utf8_lossy(&data).to_string(),
                "video" => { payload.file_name = original_file_name; payload.file_bytes = data.to_vec(); }
                "thumbnail" => { payload.thumb_name = original_file_name; payload.thumb_bytes = data.to_vec(); }
                _ => {}
            }
        }

        if payload.file_bytes.is_empty() {
            return Err((StatusCode::BAD_REQUEST, "No video file uploaded"));
        }

        if payload.title.is_empty() {
            payload.title = payload.file_name.clone();
        }

        Ok(payload)
    }
}

async fn post_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let payload = match MovieUploadPayload::from_multipart(&headers, body).await {
        Ok(p) => p,
        Err((code, msg)) => return (code, msg).into_response(),
    };

    match movie::upload_movie(
        payload.title,
        payload.description,
        payload.subtitle,
        payload.file_bytes,
        payload.file_name,
        payload.thumb_bytes,
        payload.thumb_name,
        state.movie_repo.as_ref(),
        state.object_store.as_ref(),
    )
    .await
    {
        Ok(created) => (
            StatusCode::SEE_OTHER,
            [("Location", format!("/movies/{}", created.id))],
        )
            .into_response(),
        Err(err) => {
            eprintln!("Upload error: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, err).into_response()
        }
    }
}

async fn health() -> &'static str {
    "OK"
}

async fn fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html("<h1>404 Not Found</h1>"))
}

#[tokio::main]
async fn main() {
    let movie_repository = Arc::new(movie::repository::InMemoryMovieRepository::new());
    let object_storage = Arc::new(object_store::InMemoryObjectStore::new());
    let app_state = AppState {
        movie_repository,
        object_store: object_storage,
    };

    println!(
        "WARNING: Application use in-memory implemtation of persistance which may lead to memory leaks under excessive load."
    );
    println!("DO NOT USE AT PRODUCTION");

    let app = Router::new()
        .route("/", get(index))
        .route("/upload", get(upload_form).post(post_upload))
        .route("/movies/{id}", get(watch_movie))
        .route("/movies", get(list_movies))
        .route("/objects", get(list_objects))
        .route(
            "/object/{*path}",
            get(get_object)
                .put(put_object)
                .delete(delete_object)
                .head(head_object),
        )
        .route("/health", get(health))
        .fallback(fallback)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:80")
        .await
        .expect("failed to bind to :80");

    println!("Server listening on :80");
    axum::serve(listener, app).await.expect("server error");
}

#[cfg(test)]
mod app_tests {
    use super::*;

    fn test_state() -> AppState {
        AppState {
            movie_repository: Arc::new(movie::repository::InMemoryMovieRepository::new()),
            object_store: Arc::new(object_store::InMemoryObjectStore::new()),
        }
    }

    #[tokio::test]
    async fn index_uses_state_repository() {
        let response = index(State(test_state())).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn upload_form_returns_ok() {
        let response = upload_form().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    fn multipart_body(boundary: &str, include_video: bool) -> Vec<u8> {
        let mut body = format!(
            "\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"title\"\r\n\
             \r\n\
             Uploaded Test Movie\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"description\"\r\n\
             \r\n\
             A test upload\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"subtitle\"\r\n\
             \r\n\
             Tester\r\n"
        )
        .into_bytes();

        if include_video {
            body.extend_from_slice(
                format!(
                    "\
                     --{boundary}\r\n\
                     Content-Disposition: form-data; name=\"video\"; filename=\"uploaded.mp4\"\r\n\
                     Content-Type: video/mp4\r\n\
                     \r\n\
                     fake-video-content\r\n\
                     --{boundary}--\r\n"
                )
                .as_bytes(),
            );
        } else {
            body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        }

        body
    }

    #[tokio::test]
    async fn upload_endpoint_accepts_multipart_and_creates_movie() {
        let state = test_state();
        let boundary = "----testboundary";
        let body = multipart_body(boundary, true);

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary)
                .parse()
                .unwrap(),
        );

        let response = post_upload(State(state.clone()), headers, Bytes::from(body))
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap()
            .to_string();
        assert!(location.starts_with("/movies/"));

        let movies = movie::list_movies(state.movie_repository.as_ref()).unwrap();
        assert_eq!(movies.len(), 5);
        assert_eq!(movies[4].title, "Uploaded Test Movie");
        assert_eq!(movies[4].description, "A test upload");
        assert!(!movies[4].sources[0].is_empty());
    }

    #[tokio::test]
    async fn upload_endpoint_returns_bad_request_when_no_video() {
        let state = test_state();
        let boundary = "----testboundary";
        let body = multipart_body(boundary, false);

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary)
                .parse()
                .unwrap(),
        );

        let response = post_upload(State(state.clone()), headers, Bytes::from(body))
            .await
            .into_response();

        assert!(response.status() == StatusCode::BAD_REQUEST);

        let movies = movie::list_movies(state.movie_repository.as_ref()).unwrap();
        assert_eq!(movies.len(), 4);
    }

    #[tokio::test]
    async fn upload_endpoint_returns_bad_request_when_missing_content_type() {
        let state = test_state();
        let headers = HeaderMap::new();
        let body = Bytes::from("not-multipart");

        let response = post_upload(State(state), headers, body)
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
