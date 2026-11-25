#![allow(dead_code)]

use crate::efi::*;
use core::ffi::c_void;

#[repr(C)]
struct UsbIo {
    control: usize,
    bulk: usize,
    async_int: usize,
    sync_int: usize,
    iso: usize,
    async_iso: usize,
    get_dev_desc: usize,
    get_cfg_desc: usize,
    get_if_desc: usize,
    get_ep_desc: usize,
    get_str_desc: usize,
    get_langid: usize,
    port_reset: usize,
}

#[repr(C)]
struct UsbReq {
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct IfDesc {
    len: u8,
    typ: u8,
    num: u8,
    alt: u8,
    neps: u8,
    class: u8,
    sub: u8,
    proto: u8,
    str_idx: u8,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct EpDesc {
    len: u8,
    typ: u8,
    addr: u8,
    attr: u8,
    mps: u16,
    interval: u8,
}

struct Kbd {
    io: *mut UsbIo,
    ep: u8,
    ok: bool,
    prev: [u8; 6],
    shift: bool,
    caps: bool,
    queue: [u32; 24],
    qlen: usize,
}

struct Mou {
    io: *mut UsbIo,
    ep: u8,
    ok: bool,
    abs: bool,
    seen: bool,
    ax: i32,
    ay: i32,
    rx: i32,
    ry: i32,
    btn: u8,
}

static mut KBD: Kbd = Kbd {
    io: core::ptr::null_mut(),
    ep: 0,
    ok: false,
    prev: [0; 6],
    shift: false,
    caps: false,
    queue: [0; 24],
    qlen: 0,
};

static mut MOU: Mou = Mou {
    io: core::ptr::null_mut(),
    ep: 0,
    ok: false,
    abs: false,
    seen: false,
    ax: -1,
    ay: -1,
    rx: 0,
    ry: 0,
    btn: 0,
};

static mut ABS_LAST: (i32, i32) = (-1, -1);

const DIR_IN: u32 = 0;
const DIR_NONE: u32 = 2;

unsafe fn ctrl(io: *mut UsbIo, rt: u8, req: u8, val: u16, idx: u16, data: *mut u8, len: usize, dir: u32, ms: usize) -> Status {
    let f: extern "efiapi" fn(*mut UsbIo, *mut UsbReq, u32, usize, *mut u8, usize, *mut u32) -> Status =
        core::mem::transmute((*io).control);
    let mut r = UsbReq { request_type: rt, request: req, value: val, index: idx, length: len as u16 };
    let mut tr = 0u32;
    f(io, &mut r, dir, ms, data, len, &mut tr)
}

unsafe fn get_iface(io: *mut UsbIo) -> Option<IfDesc> {
    let f: extern "efiapi" fn(*mut UsbIo, *mut IfDesc) -> Status =
        core::mem::transmute((*io).get_if_desc);
    let mut d = IfDesc { len: 0, typ: 0, num: 0, alt: 0, neps: 0, class: 0, sub: 0, proto: 0, str_idx: 0 };
    if f(io, &mut d) == EFI_SUCCESS && d.len >= 9 { Some(d) } else { None }
}

unsafe fn get_ep(io: *mut UsbIo, idx: u8) -> Option<EpDesc> {
    let f: extern "efiapi" fn(*mut UsbIo, u8, *mut EpDesc) -> Status =
        core::mem::transmute((*io).get_ep_desc);
    let mut d = EpDesc { len: 0, typ: 0, addr: 0, attr: 0, mps: 0, interval: 0 };
    if f(io, idx, &mut d) == EFI_SUCCESS && d.len >= 7 { Some(d) } else { None }
}

unsafe fn find_in_ep(io: *mut UsbIo, d: &IfDesc) -> Option<(u8, usize)> {
    let mut i = 0u8;
    while i < d.neps && i < 8 {
        if let Some(ep) = get_ep(io, i) {
            if ep.attr & 0x03 == 0x03 && ep.addr & 0x80 != 0 {
                return Some((ep.addr, (ep.mps & 0x7ff) as usize));
            }
        }
        i += 1;
    }
    None
}

unsafe fn get_descriptor(io: *mut UsbIo, dtype: u16, idx: u16, buf: &mut [u8]) -> Status {
    ctrl(io, 0x81, 0x06, dtype, idx, buf.as_mut_ptr(), buf.len(), DIR_IN, 1000)
}

unsafe fn hid_report_len(io: *mut UsbIo, ifnum: u8) -> usize {
    let mut d = [0u8; 9];
    if !status_ok(get_descriptor(io, 0x2100, ifnum as u16, &mut d)) {
        return 0;
    }
    if d[0] < 9 || d[5] < 1 || d[6] != 0x22 {
        return 0;
    }
    (d[7] as usize) | ((d[8] as usize) << 8)
}

unsafe fn read_report_desc(io: *mut UsbIo, ifnum: u8, out: &mut [u8]) -> usize {
    let len = hid_report_len(io, ifnum);
    if len == 0 {
        return 0;
    }
    let n = if len < out.len() { len } else { out.len() };
    if status_ok(get_descriptor(io, 0x2200, ifnum as u16, &mut out[..n])) {
        n
    } else {
        0
    }
}

fn find_seq(buf: &[u8], seq: &[u8]) -> bool {
    let mut i = 0;
    while i + seq.len() <= buf.len() {
        let mut k = 0;
        while k < seq.len() && buf[i + k] == seq[k] { k += 1; }
        if k == seq.len() { return true; }
        i += 1;
    }
    false
}

unsafe fn set_boot(io: *mut UsbIo, ifnum: u8) {
    let _ = ctrl(io, 0x21, 0x0B, 0, ifnum as u16, core::ptr::null_mut(), 0, DIR_NONE, 1000);
    let _ = ctrl(io, 0x21, 0x0A, 0, ifnum as u16, core::ptr::null_mut(), 0, DIR_NONE, 1000);
}

unsafe fn poll_one(io: *mut UsbIo, ep: u8, buf: &mut [u8], ms: usize) -> Option<usize> {
    let f: extern "efiapi" fn(*mut UsbIo, u8, *mut u8, *mut usize, usize, *mut u32) -> Status =
        core::mem::transmute((*io).sync_int);
    let mut len = buf.len();
    let mut tr = 0u32;
    let s = f(io, ep, buf.as_mut_ptr(), &mut len, ms, &mut tr);
    if status_ok(s) && len > 0 && len <= buf.len() { Some(len) } else { None }
}

fn key_map(usage: u8, shift: bool) -> Option<(u16, u16)> {
    let c: u8 = match usage {
        0x04..=0x1D => b'a' + (usage - 0x04),
        0x1E..=0x27 => b'1' + (usage - 0x1E),
        0x2A => 8,
        0x2B => 9,
        0x2C => b' ',
        0x2D => b'-',
        0x2E => b'=',
        0x2F => b'[',
        0x30 => b']',
        0x31 => b'\\',
        0x33 => b';',
        0x34 => b'\'',
        0x35 => b'`',
        0x36 => b',',
        0x37 => b'.',
        0x38 => b'/',
        0x3A => 0x0B,
        0x3B => 0x0C,
        0x3C => 0x0D,
        0x3D => 0x0E,
        0x3E => 0x0F,
        0x3F => 0x10,
        0x40 => 0x11,
        0x41 => 0x12,
        0x42 => 0x13,
        0x43 => 0x14,
        0x44 => 0x15,
        0x45 => 0x16,
        0x49 => 0x07,
        0x4A => 0x05,
        0x4B => 0x09,
        0x4C => 0x08,
        0x4D => 0x06,
        0x4E => 0x0A,
        0x4F => 0x03,
        0x50 => 0x04,
        0x51 => 0x02,
        0x52 => 0x01,
        _ => 0,
    };
    match usage {
        0x28 => Some((0, 13)),
        0x29 => Some((0x17, 0)),
        0x2A | 0x2B => Some((0, c as u16)),
        0x2C..=0x38 => {
            let mut ch = c;
            if shift && ch >= b'a' && ch <= b'z' { ch -= 32; } else if shift {
                ch = match ch {
                    b'1' => b'!', b'2' => b'@', b'3' => b'#', b'4' => b'$', b'5' => b'%',
                    b'6' => b'^', b'7' => b'&', b'8' => b'*', b'9' => b'(', b'0' => b')',
                    b'-' => b'_', b'=' => b'+', b'[' => b'{', b']' => b'}', b'\\' => b'|',
                    b';' => b':', b'\'' => b'"', b',' => b'<', b'.' => b'>', b'/' => b'?',
                    b'`' => b'~', x => x,
                };
            }
            Some((0, ch as u16))
        }
        0x3A..=0x45 => Some((c as u16, 0)),
        0x49..=0x52 => Some((c as u16, 0)),
        _ => None,
    }
}

unsafe fn push_key(scan: u16, ch: u16) {
    let k = &mut *core::ptr::addr_of_mut!(KBD);
    if k.qlen < 24 {
        k.queue[k.qlen] = ((scan as u32) << 16) | (ch as u32 & 0xffff);
        k.qlen += 1;
    }
}

unsafe fn kbd_report(buf: &[u8], n: usize) {
    if n < 3 { return; }
    let k = &mut *core::ptr::addr_of_mut!(KBD);
    let mods = buf[0];
    k.shift = mods & 0x22 != 0;
    let mut cur = [0u8; 6];
    let mut i = 2;
    while i < n && i < 8 {
        cur[i - 2] = buf[i];
        i += 1;
    }
    let mut j = 0;
    while j < 6 {
        let u = cur[j];
        if u != 0 {
            let mut seen = false;
            let mut p = 0;
            while p < 6 {
                if k.prev[p] == u { seen = true; }
                p += 1;
            }
            if !seen {
                if u == 0x39 {
                    k.caps = !k.caps;
                } else if let Some((s, c)) = key_map(u, k.shift) {
                    let mut ch = c;
                    if ch >= b'a' as u16 && ch <= b'z' as u16 && k.caps && !k.shift {
                        ch -= 32;
                    }
                    if ch != 0 || s != 0 { push_key(s, ch); }
                }
            }
        }
        j += 1;
    }
    k.prev = cur;
}

pub unsafe fn init(need_kbd: bool, need_mouse: bool) -> bool {
    let mut n: usize = 0;
    let mut buf: *mut Handle = core::ptr::null_mut();
    if !status_ok(bs_locate_handle_buffer(&GUID_USB_IO, &mut n, &mut buf)) || buf.is_null() {
        serial_print("[usb] no io handles");
        return false;
    }
    let mut found = false;
    let mut i = 0;
    while i < n {
        let mut io: *mut UsbIo = core::ptr::null_mut();
        if status_ok(bs_handle_protocol(
            *buf.add(i),
            &GUID_USB_IO,
            &mut io as *mut *mut UsbIo as *mut *mut c_void,
        )) && !io.is_null()
        {
            if let Some(d) = get_iface(io) {
                if d.class == 0x03 && (d.sub == 0x01 || d.sub == 0x00) {
                    let mut kind = if d.proto == 0x01 {
                        1
                    } else if d.proto == 0x02 {
                        2
                    } else {
                        0
                    };
                    let mut is_abs = false;
                    if kind == 0 {
                        let mut rd = [0u8; 256];
                        let rn = read_report_desc(io, d.num, &mut rd);
                        if rn > 0 && find_seq(&rd[..rn], &[0x09, 0x06]) {
                            kind = 1;
                        } else if rn > 0 && find_seq(&rd[..rn], &[0x09, 0x02]) {
                            kind = 2;
                        }
                        if rn == 0 {
                            if need_mouse {
                                kind = 2;
                                is_abs = true;
                            } else if need_kbd {
                                kind = 1;
                            }
                        }
                        if kind == 2 && rn > 0 {
                            is_abs = find_seq(&rd[..rn], &[0x16, 0xFF, 0x7F]) || find_seq(&rd[..rn], &[0x26, 0xFF, 0x7F]);
                        }
                    }
                    if kind != 0 {
                        if let Some((ep, _mps)) = find_in_ep(io, &d) {
                            set_boot(io, d.num);
                            if kind == 1 && need_kbd {
                                let k = &mut *core::ptr::addr_of_mut!(KBD);
                                k.io = io;
                                k.ep = ep;
                                k.ok = true;
                                k.prev = [0; 6];
                                found = true;
                                serial_print("[usb] keyboard ready");
                            } else if kind == 2 && need_mouse {
                                let m = &mut *core::ptr::addr_of_mut!(MOU);
                                m.io = io;
                                m.ep = ep;
                                m.ok = true;
                                m.abs = is_abs;
                                m.seen = false;
                                m.ax = -1;
                                m.ay = -1;
                                found = true;
                                if is_abs {
                                    serial_print("[usb] absolute mouse ready");
                                } else {
                                    serial_print("[usb] relative mouse ready");
                                }
                            }
                        } else {
                        }
                    }
                }
            }
        }
        i += 1;
    }
    bs_free_pool(buf as *mut u8);
    found
}

static mut POLL_TICK: u32 = 0;

pub unsafe fn poll(w: i32, h: i32) {
    POLL_TICK = POLL_TICK.wrapping_add(1);
    let k = &mut *core::ptr::addr_of_mut!(KBD);
    let m = &mut *core::ptr::addr_of_mut!(MOU);
    let both = k.ok && m.ok;
    if k.ok && (!both || POLL_TICK & 1 == 0) {
        let mut buf = [0u8; 8];
        if let Some(n) = poll_one(k.io, k.ep, &mut buf, 10) {
            kbd_report(&buf, n);
        }
    }
    if m.ok && (!both || POLL_TICK & 1 == 1) {
        let mut buf = [0u8; 8];
        if let Some(n) = poll_one(m.io, m.ep, &mut buf, 10) {
            if m.abs && n >= 5 {
                let x = buf[1] as i32 | ((buf[2] as i32) << 8);
                let y = buf[3] as i32 | ((buf[4] as i32) << 8);
                if !m.seen && x == 0 && y == 0 {
                    m.btn = buf[0] & 0x07;
                    return;
                }
                m.seen = true;
                let mut sx = x * w / 32767;
                let mut sy = y * h / 32767;
                if sx < 0 { sx = 0; }
                if sy < 0 { sy = 0; }
                if sx >= w { sx = w - 1; }
                if sy >= h { sy = h - 1; }
                m.ax = sx;
                m.ay = sy;
            } else if n >= 3 {
                m.rx += buf[1] as i8 as i32;
                m.ry += buf[2] as i8 as i32;
                if m.rx > 2000 { m.rx = 2000; }
                if m.rx < -2000 { m.rx = -2000; }
                if m.ry > 2000 { m.ry = 2000; }
                if m.ry < -2000 { m.ry = -2000; }
            }
            m.btn = buf[0] & 0x07;
        }
    }
}

pub unsafe fn abs_state() -> Option<(i32, i32, u8, bool)> {
    let m = &*core::ptr::addr_of!(MOU);
    if !m.ok || !m.abs || m.ax < 0 { return None; }
    let moved = ABS_LAST.0 != m.ax || ABS_LAST.1 != m.ay;
    ABS_LAST = (m.ax, m.ay);
    Some((m.ax, m.ay, m.btn, moved))
}

pub unsafe fn rel_state() -> Option<(i32, i32, u8, bool)> {
    let m = &mut *core::ptr::addr_of_mut!(MOU);
    if !m.ok || m.abs { return None; }
    let dx = m.rx;
    let dy = m.ry;
    let b = m.btn;
    let moved = dx != 0 || dy != 0;
    m.rx = 0;
    m.ry = 0;
    Some((dx, dy, b, moved))
}

pub unsafe fn take_key() -> Option<(u16, u16)> {
    let k = &mut *core::ptr::addr_of_mut!(KBD);
    if k.qlen == 0 { return None; }
    let v = k.queue[0];
    let mut i = 1;
    while i < k.qlen {
        k.queue[i - 1] = k.queue[i];
        i += 1;
    }
    k.qlen -= 1;
    Some(((v >> 16) as u16, (v & 0xffff) as u16))
}

pub fn mouse_present() -> bool {
    unsafe { MOU.ok }
}

pub fn mouse_absolute() -> bool {
    unsafe { MOU.ok && MOU.abs }
}

pub fn kbd_present() -> bool {
    unsafe { KBD.ok }
}
