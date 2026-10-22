#!/usr/bin/python3
"""Bounded synthetic authorities and a domain-routing resolver in the guest."""
import socket
import struct
import threading
import time

def question(data):
    i = 12
    labels = []
    while data[i]:
        size = data[i]
        labels.append(data[i+1:i+1+size].decode('ascii'))
        i += size + 1
    return '.'.join(labels), i+1

def exact(stream, size):
    data = b''
    while len(data) < size:
        part = stream.recv(size-len(data))
        if not part:
            raise ValueError('truncated fixture message')
        data += part
    return data

def answer(data, host, tcp):
    name, end = question(data)
    if host == '172.27.0.4':
        target = '172.27.0.3' if name.endswith('.private.test') else '172.27.0.2'
        print('SPLIT_ROUTE', name, target, 'tcp' if tcp else 'udp', flush=True)
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM if tcp else socket.SOCK_DGRAM) as upstream:
            upstream.settimeout(2)
            upstream.connect((target,53))
            if tcp:
                upstream.sendall(struct.pack('!H',len(data))+data)
                return exact(upstream,struct.unpack('!H',exact(upstream,2))[0])
            upstream.send(data)
            return upstream.recv(4096)
    expected = 'mirror.private.test' if host.endswith('.3') else 'mirror.build.test'
    qt, qc = struct.unpack('!HH',data[end:end+4])
    value = socket.inet_aton(host) if name == expected and qt == 1 else None
    record = b'\xc0\x0c'+struct.pack('!HHIH',1,1,60,4)+value if value else b''
    return data[:2]+struct.pack('!HHHHH',0x8180 if value else 0x8183,1,int(value is not None),0,0)+data[12:end+4]+record

def serve(host,tcp):
    with socket.socket(socket.AF_INET,socket.SOCK_STREAM if tcp else socket.SOCK_DGRAM) as server:
        server.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
        server.bind((host,53))
        if tcp:
            server.listen()
        while True:
            if tcp:
                connection,_ = server.accept()
                with connection:
                    connection.settimeout(3)
                    data=exact(connection,struct.unpack('!H',exact(connection,2))[0])
                    result=answer(data,host,True)
                    connection.sendall(struct.pack('!H',len(result))+result)
            else:
                data,peer=server.recvfrom(4096)
                server.sendto(answer(data,host,False),peer)
for host in ['172.27.0.2','172.27.0.3','172.27.0.4']:
    for tcp in [False,True]:
        threading.Thread(target=serve,args=(host,tcp),daemon=True).start()
time.sleep(60)
