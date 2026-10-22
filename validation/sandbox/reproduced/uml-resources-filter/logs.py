import os,time
for i in range(128): os.write(1,b'x'*4096)
time.sleep(2)
