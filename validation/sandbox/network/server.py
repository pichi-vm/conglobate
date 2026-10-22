import socket,threading,struct,time

def dns(data):
 i=12
 while data[i]:i+=data[i]+1
 i+=1
 qt,qc=struct.unpack('!HH',data[i:i+4]);question=data[12:i+4]
 address={1:socket.inet_aton('172.27.0.2'),28:socket.inet_pton(socket.AF_INET6,'fd42:27::2')}.get(qt)
 answer=b'\xc0\x0c'+struct.pack('!HHIH',qt,1,60,len(address))+address if address else b''
 return data[:2]+struct.pack('!HHHHH',0x8180,1,int(address is not None),0,0)+question+answer

def udp():
 s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM);s.bind(('0.0.0.0',53))
 while True:
  d,a=s.recvfrom(4096);s.sendto(dns(d),a)

def tcp(port):
 s=socket.socket();s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);s.bind(('0.0.0.0',port));s.listen()
 while True:
  c,a=s.accept()
  with c:
   d=c.recv(4096)
   if port==53:
    reply=dns(d[2:]);c.sendall(struct.pack('!H',len(reply))+reply)
   else:c.sendall(b'HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nOK')
def tcp6():
 s=socket.socket(socket.AF_INET6)
 s.setsockopt(socket.IPPROTO_IPV6,socket.IPV6_V6ONLY,1)
 s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
 s.bind(('::',8080));s.listen()
 while True:
  c,a=s.accept()
  with c:
   c.recv(4096);c.sendall(b'HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nOK')
for f,args in [(udp,()),(tcp,(53,)),(tcp,(8080,)),(tcp6,())]:threading.Thread(target=f,args=args,daemon=True).start()
print('SERVERS_READY',flush=True)
time.sleep(120)
