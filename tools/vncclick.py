#!/usr/bin/env python3
"""Send a VNC pointer click at absolute coordinates."""
import socket
import struct
import sys
import time


def recvn(s, n):
    buf = b""
    while len(buf) < n:
        c = s.recv(n - len(buf))
        if not c:
            raise EOFError
        buf += c
    return buf


def main():
    port = int(sys.argv[1])
    x = int(sys.argv[2])
    y = int(sys.argv[3])
    s = socket.create_connection(("127.0.0.1", port), timeout=10)
    recvn(s, 12)
    s.sendall(b"RFB 003.008\n")
    n = recvn(s, 1)[0]
    recvn(s, n)
    s.sendall(bytes([1]))
    recvn(s, 4)
    s.sendall(bytes([1]))
    hdr = recvn(s, 24)
    namelen = struct.unpack(">I", hdr[20:24])[0]
    recvn(s, namelen)

    def ev(mask, px=None, py=None):
        if px is None:
            px, py = x, y
        s.sendall(struct.pack(">BBHH", 5, mask, px, py))

    ev(0, 1, 1)
    time.sleep(0.2)
    ev(0)
    time.sleep(0.35)
    ev(1)
    time.sleep(0.35)
    ev(0)
    time.sleep(0.2)
    s.close()
    print("clicked %d,%d" % (x, y))


if __name__ == "__main__":
    main()