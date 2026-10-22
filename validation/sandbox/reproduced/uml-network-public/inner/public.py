import socket
from pathlib import Path
Path('/etc/resolv.conf').write_text('nameserver 10.0.2.3\noptions timeout:2 attempts:2\n')
addresses=socket.getaddrinfo('deb.debian.org',80,socket.AF_INET,socket.SOCK_STREAM)
assert addresses
with socket.create_connection(addresses[0][4],5) as s:
 s.sendall(b'HEAD /debian/ HTTP/1.0\r\nHost: deb.debian.org\r\n\r\n')
 reply=s.recv(4096)
 assert reply.startswith(b'HTTP/1.') and reply.split()[1] in (b'200',b'301',b'302'),reply
print('UML_PUBLIC_DNS_HTTP_PASS',flush=True)
