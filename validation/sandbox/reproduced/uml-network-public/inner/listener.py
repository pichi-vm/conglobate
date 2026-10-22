#!/usr/bin/python3
"""Bounded guest-only listeners used to verify unsolicited inbound denial."""
import pathlib
import socket
import time

listeners = []
for family, address in ((socket.AF_INET, ("0.0.0.0", 8081)),
                        (socket.AF_INET6, ("::", 8081))):
    server = socket.socket(family)
    if family == socket.AF_INET6:
        server.setsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY, 1)
    server.bind(address)
    server.listen()
    listeners.append(server)
pathlib.Path("/tmp/listener-ready").write_text("ready\n")
print("UML_LISTENER_READY",flush=True)
time.sleep(5)
for server in listeners:
    server.close()
print("NETWORK_RESTRICTED_LISTENERS_FINISHED", flush=True)
