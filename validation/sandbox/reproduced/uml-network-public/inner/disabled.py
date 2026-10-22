import socket
for addr in [('172.27.0.2',8080),('fd42:27::2',8080)]:
 try:socket.create_connection(addr,.3)
 except OSError:pass
 else:raise AssertionError('disabled network reached upstream')
try:socket.getaddrinfo('mirror.build.test',8080)
except socket.gaierror:pass
else:raise AssertionError('disabled DNS resolved')
print('UML_DISABLED_NETWORK_PASS',flush=True)
