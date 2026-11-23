#!/usr/bin/env python3
"""Talk to the QEMU human monitor over a unix socket."""
import socket
import sys
import time

def main():
    path = sys.argv[1]
    cmds = sys.argv[2:]
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(5)
    s.connect(path)
    time.sleep(0.3)
    try:
        s.recv(65536)
    except Exception:
        pass
    for c in cmds:
        s.sendall(c.encode() + b"\n")
        time.sleep(0.4)
        try:
            s.recv(65536)
        except Exception:
            pass
    s.close()

if __name__ == "__main__":
    main()