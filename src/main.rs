use std::net::{TcpListener, TcpStream};
use std::io::{BufRead, BufReader, Write};

fn handle_request(mut stream: TcpStream) {
    let reader = BufReader::new(&stream);
    let request_line = match reader.lines().next() {
        Some(Ok(line)) => line,
        _ => return,
    };

    println!("Request: {}", request_line);

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    let (method, path) = if parts.len() >= 2 {
        (parts[0], parts[1])
    } else {
        ("", "/")
    };

    // Route based on path
    let body = match (method, path) {
        ("GET", "/") => "<html><body><h1>Hello World!</h1></body></html>",
        ("GET", "/health") => "OK",
        _ => "<h1>404 Not Found</h1>",
    };

    let response = format!(
        "HTTP/1.0 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    stream.write_all(response.as_bytes()).unwrap();
}

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("0.0.0.0:80")?;
    println!("Server have been launched and listening on ::80");

    for stream in listener.incoming() {
        if let Ok(s) = stream {
            handle_request(s);
        }
    }
    Ok(())
}
