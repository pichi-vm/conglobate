#!/usr/bin/python3
"""Negative control: image loopback stub is not the upstream resolver."""
import pathlib
import socket

assert "127.0.0.53" in pathlib.Path("/etc/resolv.conf").read_text()
try:
    socket.getaddrinfo("mirror.build.test", 8080, socket.AF_INET, socket.SOCK_STREAM)
except socket.gaierror as error:
    assert error.errno == socket.EAI_AGAIN, error
else:
    raise AssertionError("unconfigured loopback stub unexpectedly resolved")
print("LOOPBACK_STUB_UNREACHABLE_CONFIRMED", flush=True)
