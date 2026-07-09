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

use crate::{movie, object_store};

#[derive(Clone)]
pub struct ApplicationState {
    pub movie_repo: Arc<dyn movie::MovieRepository>,
    pub object_store: Arc<dyn object_store::ObjectStore>,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    movies: Vec<movie::Movie>,
}

#[derive(Template)]
#[template(path = "movie/watch.html")]
struct WatchMovieTemplate {
    movie: movie::Movie,
}

#[derive(Template)]
#[template(path = "movie/upload.html")]
struct UploadMovieTemplate;

async fn handle_index_render(State(state): State<ApplicationState>) -> impl IntoResponse {
    let movies = state.movie_repo.list_movies().unwrap_or_default();
    let tpl = IndexTemplate { movies };

    match tpl.render() {
        | Ok(body) => Html(body).into_response(),
        | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response(),
    }
}

async fn handle_get_movie_render(
    Path(id): Path<i32>, State(state): State<ApplicationState>,
) -> impl IntoResponse {
    match state.movie_repo.get_movie(id) {
        | Ok(Some(movie)) => {
            let tpl = WatchMovieTemplate { movie };
            match tpl.render() {
                | Ok(body) => Html(body).into_response(),
                | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response(),
            }
        }
        | Ok(None) | Err(_) => {
            (StatusCode::NOT_FOUND, Html("<h1>404 Not Found</h1>")).into_response()
        }
    }
}

async fn handle_query_movies(State(state): State<ApplicationState>) -> impl IntoResponse {
    match state.movie_repo.list_movies() {
        | Ok(movies) => (StatusCode::OK, serde_json::to_string(&movies).unwrap()).into_response(),
        | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error listing movies").into_response(),
    }
}

async fn handle_get_object(
    Path(path): Path<String>, State(state): State<ApplicationState>,
) -> impl IntoResponse {
    match state.object_store.get_bytes(&path).await {
        | Ok(Some(bytes)) => {
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
        | Ok(None) => (StatusCode::NOT_FOUND, "object not found").into_response(),
        | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error retrieving object").into_response(),
    }
}

#[derive(Deserialize)]
struct ListObjectsQuery {
    prefix: Option<String>,
}

async fn handle_query_objects(
    Query(query): Query<ListObjectsQuery>, State(state): State<ApplicationState>,
) -> impl IntoResponse {
    match state
        .object_store
        .list_objects(query.prefix.as_deref())
        .await
    {
        | Ok(objects) => (StatusCode::OK, Json(objects)).into_response(),
        | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error listing objects").into_response(),
    }
}

async fn handle_delete_object(
    Path(path): Path<String>, State(state): State<ApplicationState>,
) -> impl IntoResponse {
    match state.object_store.delete_object(&path).await {
        | Ok(_) => StatusCode::NO_CONTENT.into_response(),
        | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error deleting object").into_response(),
    }
}

async fn handle_head_object(
    Path(path): Path<String>, State(state): State<ApplicationState>,
) -> impl IntoResponse {
    match state.object_store.head_object(&path).await {
        | Ok(Some(meta)) => {
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
        | Ok(None) => (StatusCode::NOT_FOUND, "object not found").into_response(),
        | Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            "error retrieving object metadata",
        )
            .into_response(),
    }
}

// TODO(keinsell): Upon major release this functionality could not have a place and be released as it allows overriding content inside application without permission handling.
async fn handle_put_object(
    Path(_path): Path<String>, State(state): State<ApplicationState>, body: Bytes,
) -> impl IntoResponse {
    use sha2::Digest;

    let hash = hex::encode(sha2::Sha256::digest(&body));
    match state.object_store.put_bytes(&hash, body.to_vec()).await {
        | Ok(_) => (StatusCode::CREATED, hash).into_response(),
        | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error storing object").into_response(),
    }
}

async fn handle_movie_upload_render() -> impl IntoResponse {
    match UploadMovieTemplate.render() {
        | Ok(body) => Html(body).into_response(),
        | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response(),
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
    async fn from_multipart(
        headers: &HeaderMap, body: Bytes,
    ) -> Result<Self, (StatusCode, &'static str)> {
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
            let data = field
                .bytes()
                .await
                .map_err(|_| (StatusCode::BAD_REQUEST, "Failed to read upload field"))?;

            match name.as_str() {
                | "title" => payload.title = String::from_utf8_lossy(&data).to_string(),
                | "description" => payload.description = String::from_utf8_lossy(&data).to_string(),
                | "subtitle" => payload.subtitle = String::from_utf8_lossy(&data).to_string(),
                | "video" => {
                    payload.file_name = original_file_name;
                    payload.file_bytes = data.to_vec();
                }
                | "thumbnail" => {
                    payload.thumb_name = original_file_name;
                    payload.thumb_bytes = data.to_vec();
                }
                | _ => {}
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

async fn handle_movie_upload_form(
    State(state): State<ApplicationState>, headers: HeaderMap, body: Bytes,
) -> impl IntoResponse {
    let payload = match MovieUploadPayload::from_multipart(&headers, body).await {
        | Ok(p) => p,
        | Err((code, msg)) => return (code, msg).into_response(),
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
        | Ok(created) => (
            StatusCode::SEE_OTHER,
            [("Location", format!("/movies/{}", created.id))],
        )
            .into_response(),
        | Err(err) => {
            eprintln!("Upload error: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, err).into_response()
        }
    }
}

async fn handle_delete_movie(
    Path(id): Path<i32>, State(state): State<ApplicationState>,
) -> impl IntoResponse {
    match movie::delete_movie(id, state.movie_repo.as_ref(), state.object_store.as_ref()).await {
        | Ok(true) => StatusCode::NO_CONTENT.into_response(),
        | Ok(false) => (StatusCode::NOT_FOUND, "movie not found").into_response(),
        | Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "error deleting movie").into_response(),
    }
}

async fn handle_healthcheck() -> &'static str {
    "OK"
}

async fn handle_fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html("<h1>404 Not Found</h1>"))
}

pub fn router(state: ApplicationState) -> Router {
    Router::new()
        .route("/", get(handle_index_render))
        .route(
            "/upload",
            get(handle_movie_upload_render).post(handle_movie_upload_form),
        )
        .route("/movies/{id}", get(handle_get_movie_render).delete(handle_delete_movie))
        .route("/movies", get(handle_query_movies))
        .route("/objects", get(handle_query_objects))
        .route(
            "/object/{*path}",
            get(handle_get_object)
                .put(handle_put_object)
                .delete(handle_delete_object)
                .head(handle_head_object),
        )
        .route("/health", get(handle_healthcheck))
        .fallback(handle_fallback)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    use axum::body::Bytes;
    use axum::extract::State;
    use axum::http::HeaderMap;
    use axum::response::IntoResponse;
    use diesel::r2d2::{self, ConnectionManager};
    use diesel::sqlite::SqliteConnection;

    fn test_state() -> ApplicationState {
        let manager = ConnectionManager::<SqliteConnection>::new(":memory:");
        let pool = r2d2::Pool::builder()
            .max_size(1)
            .build(manager)
            .expect("Could not build test database pool");
        crate::database::run_migrations(&pool);
        ApplicationState {
            movie_repo: Arc::new(crate::database::SqliteMovieRepository::new(pool)),
            object_store: Arc::new(crate::object_store::InMemoryObjectStore::new()),
        }
    }

    #[tokio::test]
    async fn index_uses_state_database() {
        let response = handle_index_render(State(test_state()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn upload_form_returns_ok() {
        let response = handle_movie_upload_render().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    fn testutil_multipart_body(boundary: &str, include_video: bool) -> Vec<u8> {
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
        let body = testutil_multipart_body(boundary, true);

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary)
                .parse()
                .unwrap(),
        );

        let response = handle_movie_upload_form(State(state.clone()), headers, Bytes::from(body))
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

        let movies = state.movie_repo.list_movies().unwrap();
        assert_eq!(movies.len(), 5);
        assert_eq!(movies[4].title, "Uploaded Test Movie");
        assert_eq!(movies[4].description, "A test upload");
        assert!(!movies[4].sources[0].is_empty());
    }

    #[tokio::test]
    async fn upload_endpoint_returns_bad_request_when_no_video() {
        let state = test_state();
        let boundary = "----testboundary";
        let body = testutil_multipart_body(boundary, false);

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary)
                .parse()
                .unwrap(),
        );

        let response = handle_movie_upload_form(State(state.clone()), headers, Bytes::from(body))
            .await
            .into_response();

        assert!(response.status() == StatusCode::BAD_REQUEST);

        let movies = state.movie_repo.list_movies().unwrap();
        assert_eq!(movies.len(), 4);
    }

    #[tokio::test]
    async fn upload_endpoint_returns_bad_request_when_missing_content_type() {
        let state = test_state();
        let headers = HeaderMap::new();
        let body = Bytes::from("not-multipart");

        let response = handle_movie_upload_form(State(state), headers, body)
            .await
            .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn delete_movie_returns_no_content_and_removes_movie() {
        let state = test_state();
        let boundary = "----testboundary";
        let body = testutil_multipart_body(boundary, true);

        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={}", boundary)
                .parse()
                .unwrap(),
        );

        let _ = handle_movie_upload_form(State(state.clone()), headers, Bytes::from(body))
            .await
            .into_response();

        let movies = state.movie_repo.list_movies().unwrap();
        assert_eq!(movies.len(), 5);
        let movie_id = movies[4].id;

        let response = handle_delete_movie(Path(movie_id), State(state.clone()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let movies = state.movie_repo.list_movies().unwrap();
        assert_eq!(movies.len(), 4);

        let response = handle_delete_movie(Path(movie_id), State(state))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delete_movie_returns_not_found_for_nonexistent_id() {
        let state = test_state();
        let response = handle_delete_movie(Path(9999), State(state))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
