use std::sync::Arc;

use askama::Template;
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::get,
};

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

async fn health() -> &'static str {
    "OK"
}

async fn fallback() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Html("<h1>404 Not Found</h1>"))
}

#[tokio::main]
async fn main() {
    let repository = Arc::new(movie::repository::InMemoryMovieRepository::new());
    let app_state = AppState {
        movie_repository: repository,
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/movies/{id}", get(watch_movie))
        .route("/movies", get(list_movies))
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
        let state = AppState {
            movie_repository: Arc::new(movie::repository::InMemoryMovieRepository::new()),
        };

        let response = index(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
