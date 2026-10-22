#!/usr/bin/python3
import pathlib
import socket
import struct
status=dict(line.split(':',1) for line in pathlib.Path('/proc/self/status').read_text().splitlines())
assert int(status['Seccomp']) == 2 and int(status['NoNewPrivs']) == 1
config=pathlib.Path('/etc/resolv.conf').read_text()
first=config.splitlines()[0].split()[1]

def query(name,server,tcp=False):
    q=b'\x01\x02'+struct.pack('!HHHHH',0x100,1,0,0,0)
    q+=b''.join(bytes([len(label)])+label.encode() for label in name.split('.'))+b'\0'+struct.pack('!HH',1,1)
    with socket.socket(socket.AF_INET,socket.SOCK_STREAM if tcp else socket.SOCK_DGRAM) as stream:
        stream.settimeout(2)
        stream.connect((server,53))
        if tcp:
            stream.sendall(struct.pack('!H',len(q))+q)
            def exact(size):
                result=b''
                while len(result)<size:
                    part=stream.recv(size-len(result))
                    assert part
                    result+=part
                return result
            reply=exact(struct.unpack('!H',exact(2))[0])
        else:
            stream.send(q)
            reply=stream.recv(4096)
        return reply
pairs=[('mirror.build.test','172.27.0.2'),('mirror.private.test','172.27.0.3')]
# Both authorities are reachable; each intentionally denies the other's name.
for name,server in pairs:
    reply=query(name,server)
    assert reply[3]&15 == 0 and reply[-4:] == socket.inet_aton(server)
for name,server in pairs:
    if first == '172.27.0.4' or server == first:
        assert socket.getaddrinfo(name+'.',80,socket.AF_INET,socket.SOCK_STREAM)[0][4][0] == server
    else:
        try:
            socket.getaddrinfo(name+'.',80,socket.AF_INET,socket.SOCK_STREAM)
        except socket.gaierror as error:
            assert error.errno == socket.EAI_NONAME
        else:
            raise AssertionError('expected first authority NXDOMAIN to prevent fallback')
if first == '172.27.0.4':
    for name,server in pairs:
        reply=query(name,first,True)
        assert reply[3]&15 == 0 and reply[-4:] == socket.inet_aton(server)
    print('SPLIT_ROUTED_UDP_TCP_PASS',flush=True)
else:
    print('SPLIT_FLAT_LIST_LIMIT_CONFIRMED',first,flush=True)
