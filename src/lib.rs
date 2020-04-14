use log::{error, info, trace};
use std::error::Error;
use std::io::prelude::*;
use std::net::{TcpListener, TcpStream};
use std::str;
use std::thread;

pub mod http;
pub mod middleware;
pub mod routing;

use middleware::{FileMiddleware, Middleware};
use routing::Router;

pub struct Config {
    pub port: String,
}

impl Config {
    pub fn new(args: &[String]) -> Result<Config, &str> {
        if args.len() < 2 {
            return Err("too few arguments, please include port");
        }

        let port = args[1].clone();

        Ok(Config { port })
    }
}

// Box<dyn Error> is a pointer to any type that implements Error
// ? unwraps a Result to the value in Ok, if it's an Err then the entire func returns an Err
pub fn run(config: Config) -> Result<(), Box<dyn Error>> {
    info!(
        "Starting {} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    let address = format!("127.0.0.1:{}", config.port);

    info!("Listening at: {}", address);
    let listener = TcpListener::bind(address).unwrap();

    for stream in listener.incoming() {
        let stream = stream.unwrap();

        // vulnerable to DoS attack (one thread per request -> 1000 threads for 1000 requests)
        // different models: thread pool, fork/join model, single-threaded async I/O model
        thread::spawn(move || {
            handle_client(stream);
        });
    }

    Ok(())
}

fn setup_router() -> Router {
    let mut router = Router::new();

    router.register("/hello", http::Method::Get, |_| http::Response {
        version: http::Version::OneDotOne,
        status: http::Status::Ok,
        headers: http::Headers {
            headers: Vec::new(),
        },
        body: "Hello, world!".to_string(),
    });

    router
}

fn setup_middleware() -> Box<dyn Middleware> {
    Box::new(FileMiddleware {
        file_directory: "public",
    })
}

fn create_response(
    s: String,
    middleware: &Box<dyn Middleware>,
    router: &Router,
) -> Result<http::Response, http::Status> {
    let request = http::Request::parse(&s).or(Err(http::Status::BadRequest))?;

    let response = middleware
        .answer(&request)
        .or_else(|_| router.dispatch(&request));

    response
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    let _ = stream.read(&mut buffer[..]).unwrap();

    let req_str = str::from_utf8(&buffer).unwrap();

    let request = String::from(req_str);

    // TODO
    let request_copy = request.clone();
    let first_line = request_copy.splitn(2, "\r\n").next().unwrap();

    let router = setup_router();
    let middleware = setup_middleware();

    let response = match create_response(request, &middleware, &router) {
        Ok(r) => {
            info!("{} => {}", first_line, r.status);
            r
        }
        Err(e) => {
            error!("{} => {}", first_line, e);

            http::Response {
                version: http::Version::OneDotOne,
                status: e,
                headers: http::Headers {
                    headers: Vec::new(),
                },
                body: "".to_string(),
            }
        }
    };

    let _ = stream.write(format!("{}", response).as_bytes());
}
