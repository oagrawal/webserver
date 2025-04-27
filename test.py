# omagr@parcheesi:~/concurrency/project/server$ lsof -i :7878
# 
import subprocess
from datetime import datetime
server_process = subprocess.Popen(f"./target/debug/server 2 -q 8", 
                                    shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
print(f"server started!!")
result = subprocess.run("ab -n 10000 -c 10 -s 30 http://localhost:7878/", 
                              shell=True, capture_output=True, text=True)

print(f"OUTPUT: \n {result} \n __________________________________________")
server_process.terminate()
server_process.wait(timeout=5)
