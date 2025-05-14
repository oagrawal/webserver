# Multithreaded Lock-Free Web Server in Rust

## Here is a demo of the project

This project implements and analyzes a high-performance multithreaded web server in Rust, comparing different concurrency strategies and their performance characteristics.


https://github.com/user-attachments/assets/e9b4a654-b83c-4fb9-bdc5-396656fd35b1


## Project Overview

This project explores the design space of concurrent web server implementations by comparing:

- **Queue Implementation**: Traditional lock-based queue vs. lock-free queue
- **Thread Management**: Thread-per-connection model vs. thread pool approach

We developed three implementations:
1. Web server that spawns a thread per connection
2. Web server using a lock-based ThreadPool
3. Web server using a ThreadPool with a lock-free queue algorithm

Additionally, we included a sequential implementation (single-worker locked implementation) as a baseline for performance comparison.

## Implementation Details

### Web Server Architecture

All implementations use Rust's `net::TcpListener` to listen on port 7878. Each implementation handles connections differently:

#### Thread-Per-Connection Model
- Spawns a new thread for each incoming connection
- Each thread executes the `handle_connection()` function

#### Lock-Based Thread Pool
- Uses a fixed number of worker threads
- Distributes work via a channel protected by a lock
- Workers compete to acquire the lock to process the next request

#### Lock-Free Thread Pool
- Uses a custom lock-free queue (ArrayQueue) to distribute work
- Workers use Compare-And-Swap (CAS) operations to claim work items
- Avoids lock contention through atomic operations
- Uses cache padding to prevent false sharing

### Workloads Tested

1. **In-memory GET operations**: Retrieves and sends an HTML page
2. **CPU-bound operations**: Calculates prime numbers up to 10,000
3. **I/O operations**: Simulated via sleep() calls
4. **Mixed operations**: Combination of the above workloads

## Performance Analysis

### Key Findings

1. **Worker Scaling**:
   - For CPU-bound workloads, lock-free implementation shows peak performance at 1-8 workers with 45x speedup
   - Lock-free implementation performance declines with more than 16-32 workers due to CAS contention
   - I/O-bound workloads show similar performance across implementations, bottlenecked by I/O

2. **Queue Capacity Impact**:
   - Smaller queue sizes (1-8) generally provide better performance
   - Larger queue sizes may reduce cache locality and decrease performance

3. **Request Scaling**:
   - With increasing request numbers, the lock-free implementation scales significantly better
   - For CPU-intensive workloads with 10,000 requests, lock-free implementation achieved 41x speedup

4. **Workload Differences**:
   - Simple in-memory operations show minimal benefit from parallelism due to overhead
   - CPU-bound operations benefit significantly from parallelism
   - I/O-bound operations are limited by I/O, not concurrency implementation

## Technical Details

- **Language**: Rust
- **Key Technologies**: Atomic operations, Thread pools, Lock-free data structures
- **Testing Tool**: Apache HTTP server benchmarking tool (ab)
- **Metrics**: End-to-end time (Time taken for tests)

## Conclusions

- Lock-free implementation generally outperforms the lock-based approach, especially at scale
- The optimal number of worker threads depends on workload characteristics
- Queue implementation matters most for CPU-intensive workloads with many requests
- Thread-per-connection model is viable but less scalable than thread pool approaches






to run server: 
cd server
cargo run

port forwarding:
7878

cargo run -- 1  # Run implementation 1:     Lock-free queue with thread pool
cargo run -- 2  # Running implementation 2: Lock-based queue with thread pool
cargo run -- 3  # Run implementation 3:     Thread-per-connection

cargo run -- 2 -w 100 -q 10
