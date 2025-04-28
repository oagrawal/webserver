use std::{
    env,
    fs,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
    thread,
    time::Duration,
    
};
use rand::Rng;  

use server::{LockFreeThreadPool, ThreadPool};

fn main() {
    let mut implementation = "1"; 
    let mut workers = 8;          
    let mut queue_size = 100;     

    let args: Vec<String> = env::args().collect();
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "-w" | "--workers" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<usize>() {
                        Ok(w) if w > 0 => {
                            workers = w;
                            i += 2;
                        },
                        _ => {
                            println!("Error: Worker count must be a positive number");
                            println!("Usage: {} [implementation] [-w|--workers N] [-q|--queue-size N]", args[0]);
                            return;
                        }
                    }
                } else {
                    println!("Error: Missing value for workers");
                    return;
                }
            },
            "-q" | "--queue-size" => {
                if i + 1 < args.len() {
                    match args[i + 1].parse::<usize>() {
                        Ok(q) if q > 0 => {
                            queue_size = q;
                            i += 2;
                        },
                        _ => {
                            println!("Error: Queue size must be a positive number");
                            return;
                        }
                    }
                } else {
                    println!("Error: Missing value for queue size");
                    return;
                }
            },
            "-h" | "--help" => {
                println!("Usage: {} [implementation] [-w|--workers N] [-q|--queue-size N]", args[0]);
                println!("Implementations:");
                println!("  1: Sequential (single-threaded thread pool)");
                println!("  2: Lock-free queue with thread pool");
                println!("  3: Lock-based queue with thread pool");
                println!("  4: Thread-per-connection");
                println!("Options:");
                println!("  -w, --workers N    Number of worker threads (default: 4)");
                println!("  -q, --queue-size N Size of job queue for lock-free implementation (default: 100)");
                return;
            },
            imp if !imp.starts_with('-') => {
                implementation = imp;
                i += 1;
            },
            _ => {
                println!("Unknown option: {}", args[i]);
                println!("Usage: {} [implementation] [-w|--workers N] [-q|--queue-size N]", args[0]);
                return;
            }
        }
    }

    println!("Using implementation {}, {} worker threads, queue size of {}", 
            &implementation, &workers, &queue_size);


    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    
    match implementation {
        "1" => {
            // sequential: single-threaded thread pool
            println!("Running implementation 1: Sequential (single-threaded thread pool)");
            let pool = ThreadPool::new(1);
            
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                pool.execute(|| {
                    handle_connection(stream);
                });
            }
        },
        "2" => {
            // Lock-free queue with thread pool
            println!("Running implementation 2: Lock-free queue with thread pool");
            let pool = LockFreeThreadPool::new(workers, queue_size);
            
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                pool.execute(|| {
                    handle_connection(stream);
                });
            }
        },
        "3" => {
            // Lock-based queue with thread pool
            println!("Running implementation 3: Lock-based queue with thread pool");
            let pool = ThreadPool::new(workers);
            
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                pool.execute(|| {
                    handle_connection(stream);
                });
            }
        },
        "4" => {
            // thread-per-connection
            println!("Running implementation 4: thread-per-connection");
            
            for stream in listener.incoming() {
                let stream = stream.unwrap();
                thread::spawn(|| {
                    handle_connection(stream);
                });
            }
        },
        _ => {
            println!("Invalid implementation number. Choose 1-4.");
            return;
        }
    }
    
    println!("Shutting down.");
}

// Function to handle incoming connections
fn handle_connection(mut stream: TcpStream) {
    let buf_reader = BufReader::new(&stream);
    let request_line = match buf_reader.lines().next() {
        Some(Ok(line)) => line,
        Some(Err(_)) => {
            send_error_response(&mut stream, "500 Internal Server Error", "I/O Error");
            return;
        }
        None => {
            send_error_response(&mut stream, "400 Bad Request", "No request line");
            return;
        }
    };

    println!("Received request: '{}'", request_line);

    let (status_line, content) = if request_line.starts_with("GET / ") {
        match fs::read_to_string("response.html") {
            Ok(contents) => ("HTTP/1.1 200 OK", contents),
            Err(_) => {
                send_error_response(&mut stream, "500 Internal Server Error", "Failed to read response.html");
                return;
            }
        }
    } else if request_line.starts_with("GET /cpu ") {
        let mut primes = Vec::new();
        for num in 2..10000 {
            if is_prime(num) {
                primes.push(num);
            }
        }
        let result = format!("Found {} primes up to 10,000", primes.len());
        ("HTTP/1.1 200 OK", result)
    } else if request_line.starts_with("GET /sleep ") {
        thread::sleep(Duration::from_secs(1));  
        let contents = fs::read_to_string("response.html").unwrap();
        ("HTTP/1.1 200 OK", contents)
    } else if request_line.starts_with("GET /mixed ") {
        let mut rng = rand::thread_rng();  
        let workload_type: u8 = rng.gen_range(0..3);  
        
        match workload_type  {
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
                thread::sleep(Duration::from_secs(1));
                let contents = fs::read_to_string("response.html").unwrap();
                ("HTTP/1.1 200 OK", format!("Mixed workload (I/O): Completed after sleep"))
            }
        }
    } else {
        // 404 Not Found for any other paths
        println!("Path not recognized: '{}'", request_line);
        let contents = fs::read_to_string("404.html").unwrap_or_else(|_| "404 Not Found".to_string());
        ("HTTP/1.1 404 NOT FOUND", contents)
    };

    let length = content.len();
    let response = format!("{status_line}\r\nContent-Length: {length}\r\n\r\n{content}");

    if let Err(e) = stream.write_all(response.as_bytes()) {
        println!("Failed to send response: {}", e);
    }
}

fn send_error_response(stream: &mut TcpStream, status: &str, message: &str) {
    let response = format!("{status}\r\nContent-Length: {}\r\n\r\n{message}", message.len());
    if let Err(e) = stream.write_all(response.as_bytes()) {
        println!("Failed to send error response: {}", e);
    }
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

