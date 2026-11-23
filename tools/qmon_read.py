#!/usr/bin/env python3
"""Send monitor commands and print their replies."""
import socket
import sys
import time

def main():
    path = sys.argv[1]
    cmds = sys.argv[2:]
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    s.settimeout(4)
    s.connect(path)
    time.sleep(0.5)
    try:
        s.recv(262144)
    except Exception:
        pass
    for c in cmds:
        s.sendall(c.encode() + b"\n")
        time.sleep(0.8)
        out = b""
        try:
            while True:
                chunk = s.recv(262144)
                if not chunk:
                    break
                out += chunk
                if len(chunk) < 262144:
                    break
        except Exception:
            pass
        print("### %s" % c)
        print(out.decode("utf-8", "replace"))
    s.close()

if __name__ == "__main__":
    main()