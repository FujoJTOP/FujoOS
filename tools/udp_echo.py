#!/usr/bin/env python3
# udp_echo.py — W14a host 侧回显服务器 (QEMU slirp: guest -> 10.0.2.2:7777 = 127.0.0.1:7777)
# 用法: python tools/udp_echo.py [port]
import socket
import sys

port = int(sys.argv[1]) if len(sys.argv) > 1 else 7777
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind(("127.0.0.1", port))
print(f"udp_echo: listening 127.0.0.1:{port}", flush=True)
while True:
    d, a = s.recvfrom(2048)
    print(f"udp_echo: rx {len(d)} bytes from {a}", flush=True)
    s.sendto(d, a)
