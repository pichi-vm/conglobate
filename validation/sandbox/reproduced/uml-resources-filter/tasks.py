import subprocess
children=[]
try:
 for i in range(32): children.append(subprocess.Popen(['/usr/bin/sleep','10']))
except BlockingIOError: print('PID_LIMIT_OK',flush=True)
