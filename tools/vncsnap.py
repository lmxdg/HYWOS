#!/usr/bin/env python3
"""Capture the QEMU VNC framebuffer without external dependencies."""
import socket
import struct
import sys
from PIL import Image


def recvn(s, n):
    buf = b""
    while len(buf) < n:
        chunk = s.recv(n - len(buf))
        if not chunk:
            raise EOFError("vnc closed")
        buf += chunk
    return buf


def main():
    host = "127.0.0.1"
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 5901
    out = sys.argv[2] if len(sys.argv) > 2 else "shot.png"
    s = socket.create_connection((host, port), timeout=10)
    ver = recvn(s, 12)
    if not ver.startswith(b"RFB "):
        raise SystemExit("not an RFB server: %r" % ver)
    s.sendall(b"RFB 003.008\n")
    n = recvn(s, 1)[0]
    types = recvn(s, n)
    if 1 in types:
        s.sendall(bytes([1]))
    else:
        raise SystemExit("unsupported VNC security types: %r" % types)
    res = struct.unpack(">I", recvn(s, 4))[0]
    if res != 0:
        raise SystemExit("VNC auth failed (%d)" % res)
    s.sendall(bytes([1]))
    hdr = recvn(s, 24)
    w, h = struct.unpack(">HH", hdr[0:4])
    bpp, depth, be, truecolor = hdr[4], hdr[5], hdr[6], hdr[7]
    rmax, gmax, bmax = struct.unpack(">HHH", hdr[8:14])
    rshift, gshift, bshift = hdr[14], hdr[15], hdr[16]
    namelen = struct.unpack(">I", hdr[20:24])[0]
    recvn(s, namelen)
    s.sendall(bytes([2, 0, 0, 1, 0, 0, 0, 0]))
    s.sendall(struct.pack(">BBHHHH", 3, 0, 0, 0, w, h))
    px = None
    while True:
        t = recvn(s, 1)[0]
        if t == 0:
            recvn(s, 1)
            nr = struct.unpack(">H", recvn(s, 2))[0]
            for _ in range(nr):
                rx, ry, rw, rh = struct.unpack(">HHHH", recvn(s, 8))
                enc = struct.unpack(">i", recvn(s, 4))[0]
                if enc == 0:
                    data = recvn(s, rw * rh * (bpp // 8))
                    if px is None:
                        px = Image.new("RGB", (w, h))
                    raw = bytearray(rw * rh * 3)
                    step = bpp // 8
                    for i in range(rw * rh):
                        o = i * step
                        if step == 4:
                            v = struct.unpack("<I", data[o:o + 4])[0]
                        elif step == 3:
                            v = data[o] | (data[o + 1] << 8) | (data[o + 2] << 16)
                        else:
                            v = data[o]
                        r = ((v >> rshift) & rmax) * 255 // max(rmax, 1)
                        g = ((v >> gshift) & gmax) * 255 // max(gmax, 1)
                        b = ((v >> bshift) & bmax) * 255 // max(bmax, 1)
                        raw[i * 3] = r
                        raw[i * 3 + 1] = g
                        raw[i * 3 + 2] = b
                    img = Image.frombytes("RGB", (rw, rh), bytes(raw))
                    px.paste(img, (rx, ry))
                elif enc == -223:
                    pass
                elif enc == -308:
                    recvn(s, 1)  # cursor encoding has no payload for QEMU
                else:
                    pass
            break
        elif t == 1:
            recvn(s, 1)
            ncol = struct.unpack(">H", recvn(s, 2))[0]
            recvn(s, 6 * ncol)
        elif t == 2:
            pass
        elif t == 3:
            recvn(s, 3)
            l = struct.unpack(">I", recvn(s, 4))[0]
            recvn(s, l)
        else:
            break
    if px is None:
        raise SystemExit("no framebuffer data")
    px.save(out)
    print("captured %dx%d -> %s" % (w, h, out))


if __name__ == "__main__":
    main()