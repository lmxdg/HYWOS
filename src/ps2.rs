#![allow(dead_code)]

use crate::port::*;

static mut KBD_OK: bool = false;
static mut MOUSE_OK: bool = false;
static mut EXT: bool = false;
static mut SHIFT: bool = false;
static mut KEYQ: [u16; 32] = [0; 32];
static mut QLEN: usize = 0;
static mut MX: i32 = 0;
static mut MY: i32 = 0;
static mut BTN: u8 = 0;
static mut PACKET: [u8; 3] = [0; 3];
static mut PI: usize = 0;

#[inline]
unsafe fn wait_write() {
    let mut i = 0;
    while i < 100_000 {
        if inb(0x64) & 2 == 0 {
            return;
        }
        i += 1;
    }
}

#[inline]
unsafe fn wait_read() -> bool {
    let mut i = 0;
    while i < 100_000 {
        if inb(0x64) & 1 != 0 {
            return true;
        }
        i += 1;
    }
    false
}

unsafe fn wr_cmd(c: u8) {
    wait_write();
    outb(0x64, c);
}

unsafe fn wr_data(d: u8) {
    wait_write();
    outb(0x60, d);
}

unsafe fn rd_data() -> u8 {
    if wait_read() {
        inb(0x60)
    } else {
        0
    }
}

unsafe fn flush_out() {
    let mut i = 0;
    while i < 32 && inb(0x64) & 1 != 0 {
        let _ = inb(0x60);
        i += 1;
    }
}

unsafe fn mouse_write(d: u8) {
    wr_cmd(0xD4);
    wr_data(d);
    let _ = rd_data();
}

pub fn keyboard_ok() -> bool {
    unsafe { KBD_OK }
}

pub fn mouse_ok() -> bool {
    unsafe { MOUSE_OK }
}

pub unsafe fn init() -> bool {
    flush_out();
    wr_cmd(0xAD);
    wr_cmd(0xA7);
    flush_out();
    wr_cmd(0xAE);
    wr_data(0xF4);
    let _ = rd_data();
    KBD_OK = true;
    wr_cmd(0xA8);
    mouse_write(0xF6);
    mouse_write(0xF4);
    mouse_write(0xF3);
    mouse_write(0x64);
    mouse_write(0xE8);
    mouse_write(0x03);
    mouse_write(0xF3);
    mouse_write(0x64);
    mouse_write(0xF2);
    let id = rd_data();
    MOUSE_OK = id == 0 || id == 3 || id == 4;
    flush_out();
    KBD_OK
}

unsafe fn kbd_press(code: u8) {
    let mut out: Option<(u16, u16)> = None;
    if EXT {
        EXT = false;
        out = match code {
            0x48 => Some((0x01, 0)),
            0x50 => Some((0x02, 0)),
            0x4D => Some((0x03, 0)),
            0x4B => Some((0x04, 0)),
            0x47 => Some((0x05, 0)),
            0x4F => Some((0x06, 0)),
            0x53 => Some((0x07, 0)),
            0x52 => Some((0x08, 0)),
            0x49 => Some((0x09, 0)),
            0x51 => Some((0x0A, 0)),
            0x1C => Some((0, 13)),
            0x35 => Some((0, b'/' as u16)),
            _ => None,
        };
    } else {
        let ch: u8 = match code {
            0x02 => b'1', 0x03 => b'2', 0x04 => b'3', 0x05 => b'4', 0x06 => b'5',
            0x07 => b'6', 0x08 => b'7', 0x09 => b'8', 0x0A => b'9', 0x0B => b'0',
            0x0C => b'-', 0x0D => b'=', 0x10 => b'q', 0x11 => b'w', 0x12 => b'e',
            0x13 => b'r', 0x14 => b't', 0x15 => b'y', 0x16 => b'u', 0x17 => b'i',
            0x18 => b'o', 0x19 => b'p', 0x1A => b'[', 0x1B => b']', 0x1E => b'a',
            0x1F => b's', 0x20 => b'd', 0x21 => b'f', 0x22 => b'g', 0x23 => b'h',
            0x24 => b'j', 0x25 => b'k', 0x26 => b'l', 0x27 => b';', 0x28 => b'\'',
            0x29 => b'`', 0x2B => b'\\', 0x2C => b'z', 0x2D => b'x', 0x2E => b'c',
            0x2F => b'v', 0x30 => b'b', 0x31 => b'n', 0x32 => b'm', 0x33 => b',',
            0x34 => b'.', 0x35 => b'/', 0x39 => b' ', 0x1C => 0, 0x0E => 0,
            0x0F => 0, 0x1D => 0, 0x2A => 0, 0x36 => 0,
            _ => 0,
        };
        out = match code {
            0x01 => Some((0x17, 0)),
            0x1C => Some((0, 13)),
            0x0E => Some((0, 8)),
            0x0F => Some((0, 9)),
            0x39 => Some((0, 32)),
            0x3B => Some((0x0B, 0)), 0x3C => Some((0x0C, 0)),
            0x3D => Some((0x0D, 0)), 0x3E => Some((0x0E, 0)),
            0x3F => Some((0x0F, 0)), 0x40 => Some((0x10, 0)),
            0x41 => Some((0x11, 0)), 0x42 => Some((0x12, 0)),
            0x43 => Some((0x13, 0)), 0x44 => Some((0x14, 0)),
            0x57 => Some((0x15, 0)), 0x58 => Some((0x16, 0)),
            _ => {
                if ch != 0 {
                    let c = if SHIFT && ch >= b'a' && ch <= b'z' { ch - 32 } else { ch };
                    Some((0, c as u16))
                } else {
                    None
                }
            }
        };
    }
    if let Some(k) = out {
        if QLEN < 32 {
            KEYQ[QLEN] = ((k.0 as u32) << 16 | k.1 as u32) as u16;
            QLEN += 1;
        }
    }
}

unsafe fn kbd_byte(b: u8) {
    if b == 0xFA || b == 0xAA || b == 0xEE {
        return;
    }
    if b == 0xE0 {
        EXT = true;
        return;
    }
    if b & 0x80 != 0 {
        let code = b & 0x7F;
        if code == 0x2A || code == 0x36 {
            SHIFT = false;
        }
        EXT = false;
        return;
    }
    if b == 0x2A || b == 0x36 {
        SHIFT = true;
        return;
    }
    kbd_press(b);
}

unsafe fn mouse_byte(b: u8) {
    if PI == 0 {
        if b & 0x08 == 0 {
            return;
        }
        PACKET[0] = b;
        PI = 1;
        return;
    }
    PACKET[PI] = b;
    PI += 1;
    if PI == 3 {
        PI = 0;
        let dx = PACKET[1] as i8 as i32;
        let dy = PACKET[2] as i8 as i32;
        MX += dx;
        MY -= dy;
        BTN = PACKET[0] & 0x07;
    }
}

pub unsafe fn poll() {
    let mut guard = 0;
    while guard < 64 {
        let st = inb(0x64);
        if st & 1 == 0 {
            break;
        }
        let b = inb(0x60);
        if st & 0x20 != 0 {
            mouse_byte(b);
        } else {
            kbd_byte(b);
        }
        guard += 1;
    }
}

pub unsafe fn key_count() -> usize {
    QLEN
}

pub unsafe fn take_key() -> Option<(u16, u16)> {
    if QLEN == 0 {
        return None;
    }
    let v = KEYQ[0] as u32;
    let mut i = 1;
    while i < QLEN {
        KEYQ[i - 1] = KEYQ[i];
        i += 1;
    }
    QLEN -= 1;
    let scan = (v >> 16) as u16;
    let ch = (v & 0xffff) as u16;
    Some((scan, ch))
}

pub unsafe fn take_mouse() -> (i32, i32, u8, bool) {
    let dx = MX;
    let dy = MY;
    let b = BTN;
    let moved = dx != 0 || dy != 0;
    MX = 0;
    MY = 0;
    (dx, dy, b, moved)
}