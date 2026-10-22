#!/usr/bin/python3
import errno, pathlib, os
status = dict(line.split(":", 1) for line in pathlib.Path("/proc/self/status").read_text().splitlines())
assert int(status["Seccomp"]) == 2 and int(status["NoNewPrivs"]) == 1
assert os.getuid() == 0
try:
    with open("/etc/resolv.conf", "w") as stream: stream.write("should not write")
except OSError as error:
    assert error.errno == errno.EROFS
else:
    raise AssertionError("resolver bind was writable")
import socket,struct
assert socket.getaddrinfo("mirror", 8080, socket.AF_INET, socket.SOCK_STREAM)[0][4][0] == "172.27.0.2"
try:
    socket.getaddrinfo("absent.invalid.", 8080, socket.AF_INET, socket.SOCK_STREAM)
except socket.gaierror as error:
    assert error.errno == socket.EAI_NONAME
else:
    raise AssertionError("fixture resolver answered a nonexistent name")
print("DNS_SEARCH_DOMAIN_PASS", flush=True)
print('DNS_LOOKUP',socket.getaddrinfo('mirror.build.test',8080),flush=True)
ipv4=socket.getaddrinfo('mirror.build.test',8080,socket.AF_INET,socket.SOCK_STREAM)[0][4]
s=socket.create_connection(ipv4,2);s.sendall(b'GET / HTTP/1.0\r\n\r\n');assert b'200 OK' in s.recv(1024);s.close()
print('NAT_PRIVATE_DESTINATION_AND_DNS_UDP_PASS',flush=True)
q=b'\x01\x02'+struct.pack('!HHHHH',0x100,1,0,0,0)+b'\x06mirror\x05build\x04test\x00'+struct.pack('!HH',1,1)
s=socket.create_connection(('172.27.0.2',53),2);s.sendall(struct.pack('!H',len(q))+q);assert s.recv(1024)[-4:]==socket.inet_aton('172.27.0.2');s.close()
print('DNS_TCP_PASS',flush=True)
try: socket.create_connection(('10.44.0.1',8080),1)
except OSError:print('BUILDER_ENDPOINT_BLOCKED_PASS',flush=True)
else:raise AssertionError('builder reachable')
addresses=socket.getaddrinfo('mirror.build.test',8080,socket.AF_INET6,socket.SOCK_STREAM)
assert addresses[0][4][0]=='fd42:27::2',addresses
s=socket.socket(socket.AF_INET6);s.settimeout(2)
s.connect(addresses[0][4]);s.sendall(b'GET / HTTP/1.0\r\n\r\n')
assert b'200 OK' in s.recv(1024);s.close()
print('IPV6_NAT_AND_AAAA_DNS_PASS',flush=True)
try:socket.create_connection(('fd42:44::1',8080),1)
except OSError:print('IPV6_BUILDER_ENDPOINT_BLOCKED_PASS',flush=True)
else:raise AssertionError('IPv6 builder reachable')

print("NETWORK_RESTRICTED_CLIENT_PASS", flush=True)
