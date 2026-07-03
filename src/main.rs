use askama::Template;
use std::net::{TcpListener, TcpStream};
use std::io::{BufRead, BufReader, Write};

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    message: &'a str,
}

fn handle_request(mut stream: TcpStream) {
    let reader = BufReader::new(&stream);
    let request_line = match reader.lines().next() {
        Some(Ok(line)) => line,
        _ => return,
    };
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let (method, path) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        ("", "/")
    };

    let (status, content_type, body): (&str, &str, String) = match (method, path) {
        ("GET", "/") => {
            let tpl = IndexTemplate { message: "Hello World" };
            ("200 OK", "text/html", tpl.render().unwrap())
        }
        ("GET", "/health") => ("200 OK", "text/plain", "OK".to_string()),
        ("GET", "/api/status") => {
            ("200 OK", "application/json", r#"{"status":"up"}"#.to_string())
        }
        _ => ("404 Not Found", "text/html", "<h1>404 Not Found</h1>".to_string()),
    };

    let response = format!(
        "HTTP/1.0 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, content_type, body.len(), body
    );
    stream.write_all(response.as_bytes()).unwrap();
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:80")?;
    println!("Server listening on :80");
    for stream in listener.incoming().flatten() {
        handle_request(stream);
    }
    Ok(())
}
