use std::{
    env,
    fs,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
};

use server::{LockFreeThreadPool, ThreadPool};

fn main() {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();
    let implementation = if args.len() > 1 {
        args[1].as_str()
    } else {
        "1" // Default to implementation 1 if no argument provided
    };

    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    
    match implementation {
        "1" => {
            // Lock-free queue with thread pool
            println!("Running implementation 1: Lock-free queue with thread pool");
            let pool = LockFreeThreadPool::new(4, 100);
            
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                match pool.execute(|| {
                    handle_connection(stream);
                }) {
                    Ok(()) => {},
                    Err(()) => println!("Queue full, connection rejected"),
                }
            }
        },
        "2" => {
            // Lock-based queue with thread pool
            println!("Running implementation 2: Lock-based queue with thread pool");
            let pool = ThreadPool::new(4);
            
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                pool.execute(|| {
                    handle_connection(stream);
                });
            }
        },
        "3" => {
            // thread-per-connection
            println!("Running implementation 3: thread-per-connection");
            
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                thread::spawn(|| {
                    handle_connection(stream);
                });
            }
        },
        _ => {
            println!("Invalid implementation number. Choose 1-3.");
            return;
        }
    }
    
    println!("Shutting down.");
}

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_line = buf_reader.lines().next().unwrap().unwrap();

    let (status_line, filename) = match &request_line[..] {
        "GET / HTTP/1.1" => ("HTTP/1.1 200 OK", "response.html"),
        "GET /sleep HTTP/1.1" => {
            thread::sleep(Duration::from_secs(10));
            ("HTTP/1.1 200 OK", "response.html")
        }
        _ => ("HTTP/1.1 404 NOT FOUND", "404.html"),
    };

    let contents = fs::read_to_string(filename).unwrap();
    let length = contents.len();
    let response =
        format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{contents}");
    stream.write_all(response.as_bytes()).unwrap();
}
