mod config;

use std::sync::Arc;

use askama::Template;
use axum::{
    Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
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
        Ok(Some(bytes)) => (StatusCode::OK, bytes).into_response(),
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

    #[tokio::test]
    async fn index_uses_state_repository() {
        let temp_dir = std::env::temp_dir().join(format!("vidly-app-state-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&temp_dir);

        let state = AppState {
            movie_repository: Arc::new(movie::repository::InMemoryMovieRepository::new()),
            object_store: Arc::new(object_store::InMemoryObjectStore::new()),
        };

        let response = index(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
