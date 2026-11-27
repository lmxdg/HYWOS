#!/usr/bin/env python3
"""Create a CONLYUPD kernel update package for HYWOS."""
import struct
import sys
import zlib

LBA = 64
PAYLOAD_LEN = 512

def vercode(text: str) -> int:
    parts = [int(x) for x in text.split(".")]
    while len(parts) < 3:
        parts.append(0)
    return parts[0] * 10000 + parts[1] * 100 + parts[2]

def build(payload: bytes, version: int = 1) -> bytes:
    if len(payload) > 4 * 1024 * 1024:
        raise SystemExit("payload too large")
    crc = zlib.crc32(payload) & 0xFFFFFFFF
    head = b"CONLYUPD" + struct.pack("<IIIII", version, len(payload), LBA, crc, 0) + b"\x00\x00\x00\x00"
    assert len(head) == 32, len(head)
    return head + payload

def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "build/hywos-1.1.3.updpkg"
    version = sys.argv[2] if len(sys.argv) > 2 else "1.1.3"
    if len(sys.argv) > 3:
        with open(sys.argv[3], "rb") as fh:
            payload = fh.read()
    else:
        payload = ("HYWOS KERNEL %s\n" % version).encode()
        payload += b"CONLYUPD kernel region at LBA 64\n"
        payload += b"This package rewrites only the kernel block region.\n"
        payload += bytes((i * 7 + 13) & 0xFF for i in range(PAYLOAD_LEN - len(payload)))
        payload = payload[:PAYLOAD_LEN]
    with open(out, "wb") as fh:
        fh.write(build(payload, vercode(version)))
    print("wrote %s (%d bytes)" % (out, 32 + len(payload)))

if __name__ == "__main__":
    main()