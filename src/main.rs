use askama::Template;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    message: &'a str,
}

async fn index() -> impl IntoResponse {
    let tpl = IndexTemplate { message: "Hello World" };
    match tpl.render() {
        Ok(body) => Html(body).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response(),
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
    let app = Router::new()
        .route("/", get(index))
        .route("/health", get(health))
        .fallback(fallback);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:80")
        .await
        .expect("failed to bind to :80");

    println!("Server listening on :80");

    axum::serve(listener, app)
        .await
        .expect("server error");
}