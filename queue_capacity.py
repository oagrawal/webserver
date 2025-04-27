import subprocess
import re
import matplotlib.pyplot as plt
import time
from datetime import datetime

def get_time_taken_for_tests(cargo_args):
    """Run the server with given args and benchmark it"""
    print(f"Starting server with args: {cargo_args}")
    # Start server with output suppressed
    server_process = subprocess.Popen(
        f"./target/debug/server {cargo_args}",
        shell=True,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL
    )
    
    # Wait for server to start
    time.sleep(2)
    
    try:
        # Run benchmark
        print("Running benchmark...")
    
        total = 0
        for i in range(3):
            result = subprocess.run(
                "ab -n 10000 -c 10 -s 30 http://localhost:7878/", 
                shell=True, 
                capture_output=True, 
                text=True
            )
            
            if result.returncode != 0:
                print(f"Benchmark failed with return code {result.returncode}")
                print(f"Error: {result.stderr}")
                return None
            
            # Extract time taken
            match = re.search(r"Time taken for tests:\s*([0-9]*\.?[0-9]+) seconds", result.stdout)
            if match:
                time_taken = float(match.group(1))
                print(f"Extracted time: {time_taken} seconds")
                # return time_taken
                total += time_taken
            else:
                print("Could not extract time from output. Output excerpt:")
                print(result.stdout[:500])
                return None
        return total / 3  # Return average time taken
    
    finally:
        # Always terminate the server
        print("Terminating server...")
        server_process.terminate()
        try:
            server_process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            print("Server didn't terminate gracefully, forcing...")
            server_process.kill()

def main():
    # Warm up phase
    print("\n=== Warming up ===")
    warm_up_time = get_time_taken_for_tests("1")
    if warm_up_time:
        print(f"Warm-up completed in {warm_up_time} seconds")
    
    # Get sequential time
    print("\n=== Getting sequential time ===")
    sequential_time = get_time_taken_for_tests("1")
    
    if not sequential_time:
        print("Error: Could not determine sequential time. Exiting.")
        return
    
    print(f"Sequential time: {sequential_time} seconds")
    
    # Internal capacity values to test
    internal_capacities = [1, 2, 4, 8, 16, 32, 64]
    parallel_times = []
    speedups = []
    
    # Test each capacity
    for capacity in internal_capacities:
        print(f"\n=== Testing internal capacity: {capacity} ===")
        parallel_time = get_time_taken_for_tests(f"2 -q {capacity}")
        
        if not parallel_time:
            print(f"Warning: Could not get valid time for capacity {capacity}")
            parallel_times.append(None)
            speedups.append(None)
            continue
            
        parallel_times.append(parallel_time)
        
        # Calculate and store speedup
        speedup = sequential_time / parallel_time
        speedups.append(speedup)
        print(f"Capacity {capacity}: Parallel time = {parallel_time:.3f}s, Speedup = {speedup:.3f}x")
    
    # Filter out None values for plotting
    valid_data = [(cap, spd) for cap, spd in zip(internal_capacities, speedups) if spd is not None]
    
    if not valid_data:
        print("Error: No valid data points to plot")
        return
        
    valid_capacities, valid_speedups = zip(*valid_data)
    
    # Plotting
    plt.figure(figsize=(10, 6))
    plt.plot(valid_capacities, valid_speedups, marker='o', linestyle='-', linewidth=2)
    plt.title('Speedup vs Queue Internal Capacity', fontsize=16)
    plt.xlabel('Queue Internal Capacity', fontsize=14)
    plt.ylabel('Speedup (Sequential / Parallel)', fontsize=14)
    plt.grid(True, linestyle='--', alpha=0.7)
    plt.xticks(valid_capacities)  # Set x-ticks to match our capacity values
    
    # Add annotations for each point
    for i, (cap, spd) in enumerate(zip(valid_capacities, valid_speedups)):
        plt.annotate(f"{spd:.2f}x", 
                    (cap, spd), 
                    textcoords="offset points",
                    xytext=(0, 10), 
                    ha='center')
    
    # Get current timestamp
    now = datetime.now()
    timestamp = now.strftime("%Y-%m-%d_%H-%M-%S")
    
    # Create a unique filename with timestamp
    filename = f"speedup_plot_{timestamp}.png"
    
    # Save the plot with high resolution
    plt.savefig(filename, dpi=300, bbox_inches='tight')
    print(f"\nPlot saved as: {filename}")
    
    # Save data to CSV
    csv_filename = f"speedup_data_{timestamp}.csv"
    with open(csv_filename, "w") as f:
        f.write("Capacity,ParallelTime,Speedup\n")
        for cap, pt, spd in zip(internal_capacities, parallel_times, speedups):
            if pt is not None and spd is not None:
                f.write(f"{cap},{pt},{spd}\n")
    print(f"Data saved to: {csv_filename}")
    
    # Show summary of results
    print("\n=== Results Summary ===")
    print(f"Sequential time: {sequential_time:.3f} seconds")
    print("Queue Capacity | Parallel Time | Speedup")
    print("-------------- | ------------- | -------")
    for cap, ptime, spd in zip(internal_capacities, parallel_times, speedups):
        if ptime and spd:
            print(f"{cap:14d} | {ptime:13.3f}s | {spd:7.3f}x")
        else:
            print(f"{cap:14d} | {'N/A':13s} | {'N/A':7s}")
    
    # Try to show the plot
    try:
        plt.show()
    except Exception as e:
        print(f"Could not display plot: {e}")
        print(f"Plot has been saved to {filename}")

if __name__ == '__main__':
    main()
