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

/*
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
}*/

fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_line = buf_reader.lines().next().unwrap().unwrap();

    let (status_line, content) = match &request_line[..] {
        // 1. Baseline Throughput Test - Quick in-memory response
        "GET / HTTP/1.1" => {
            let contents = fs::read_to_string("response.html").unwrap();
            ("HTTP/1.1 200 OK", contents)
        },
        
        // 2. CPU-Bound Workload Test - Computationally intensive
        "GET /cpu HTTP/1.1" => {
            // Calculate primes up to 10,000
            let mut primes = Vec::new();
            for num in 2..10000 {
                if is_prime(num) {
                    primes.push(num);
                }
            }
            let result = format!("Found {} primes up to 10,000", primes.len());
            ("HTTP/1.1 200 OK", result)
        },
        
        // 3. IO-Bound Workload Test - Blocking sleep operation
        "GET /sleep HTTP/1.1" => {
            thread::sleep(Duration::from_secs(5));  // Reduced from 10 to 5 seconds
            let contents = fs::read_to_string("response.html").unwrap();
            ("HTTP/1.1 200 OK", contents)
        },
        
        // 4. Mixed Workload with Variable Concurrency Test
        "GET /mixed HTTP/1.1" => {
            // Use timestamp for randomization to simulate variable workloads
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            match now % 3 {
                0 => {
                    // Baseline - quick response
                    let contents = fs::read_to_string("response.html").unwrap();
                    ("HTTP/1.1 200 OK", contents)
                },
                1 => {
                    // CPU-bound - prime calculation
                    let mut primes = Vec::new();
                    for num in 2..10000 {
                        if is_prime(num) {
                            primes.push(num);
                        }
                    }
                    let result = format!("Mixed workload (CPU): Found {} primes up to 10,000", primes.len());
                    ("HTTP/1.1 200 OK", result)
                },
                _ => {
                    // IO-bound - sleep
                    thread::sleep(Duration::from_secs(5));
                    let contents = fs::read_to_string("response.html").unwrap();
                    ("HTTP/1.1 200 OK", format!("Mixed workload (I/O): Completed after sleep"))
                }
            }
        },
        
        // 404 Not Found
        _ => {
            let contents = fs::read_to_string("404.html").unwrap();
            ("HTTP/1.1 404 NOT FOUND", contents)
        },
    };

    let length = content.len();
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{content}");
    stream.write_all(response.as_bytes()).unwrap();
}

// Helper function for prime calculation
fn is_prime(n: u64) -> bool {
    if n <= 1 {
        return false;
    }
    for i in 2..=((n as f64).sqrt() as u64) {
        if n % i == 0 {
            return false;
        }
    }
    true
}

