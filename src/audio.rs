#![allow(dead_code)]

use crate::efi::*;
use crate::port::*;

static mut AC_OK: bool = false;
static mut VOLUME: u8 = 80;
static mut NAM: u16 = 0;
static mut NABM: u16 = 0;
static mut PCM: [u8; 96000] = [0; 96000];
static mut BD: [u32; 4] = [0; 4];

pub fn ok() -> bool {
    unsafe { AC_OK }
}

pub fn name() -> &'static str {
    if ok() {
        "Intel AC'97"
    } else {
        "PC Speaker"
    }
}

pub unsafe fn init() -> bool {
    let d = match find(0x8086, 0x2415) {
        Some(d) => d,
        None => return false,
    };
    let (i0, nam) = bar(&d, 0);
    let (i1, nabm) = bar(&d, 1);
    if !i0 || !i1 || nam == 0 || nabm == 0 {
        return false;
    }
    serial_print("[audio] nam/nabm");
    serial_u32(nam as u32);
    serial_u32(nabm as u32);
    NAM = nam as u16;
    NABM = nabm as u16;
    enable_bus_master(&d);
    outl(NABM + 0x00, 0x0000_0002);
    let mut i = 0;
    while i < 3000 {
        io_delay();
        i += 1;
    }
    outl(NABM + 0x00, 0x0000_0000);
    let att = (100 - VOLUME as u32) * 31 / 100;
    outw(NAM + 0x02, ((att << 8) | att) as u16);
    outw(NAM + 0x18, 0x0000);
    outw(NAM + 0x2C, 48000);
    outw(NAM + 0x2E, 48000);
    outb(NABM + 0x1B, 0x00);
    AC_OK = true;
    serial_print("[audio] AC'97 ready");
    set_volume(VOLUME);
    true
}

pub fn volume() -> u8 {
    unsafe { VOLUME }
}

pub unsafe fn set_volume(v: u8) {
    VOLUME = if v > 100 { 100 } else { v };
    if AC_OK {
        let att = (100 - VOLUME as u32) * 31 / 100;
        outw(NAM + 0x02, ((att << 8) | att) as u16);
    }
}

unsafe fn beep(freq: u32, ms: u32) {
    let div = 1193182u32 / freq;
    outb(0x43, 0xB6);
    outb(0x42, (div & 0xff) as u8);
    outb(0x42, ((div >> 8) & 0xff) as u8);
    let old = inb(0x61);
    outb(0x61, old | 3);
    bs_stall((ms as usize) * 1000);
    outb(0x61, old & !3);
}

pub unsafe fn chime() {
    if !AC_OK {
        beep(880, 150);
        return;
    }
    let rate = 48000u32;
    let frames = (rate * 45 / 100) as usize;
    let p = core::ptr::addr_of_mut!(PCM) as *mut u8;
    let mut i = 0usize;
    while i < frames {
        let t = i as f32 / rate as f32;
        let attack = if t * 18.0 < 1.0 { t * 18.0 } else { 1.0 };
        let release = if (0.45 - t) * 6.0 < 0.0 {
            0.0
        } else if (0.45 - t) * 6.0 > 1.0 {
            1.0
        } else {
            (0.45 - t) * 6.0
        };
        let env = attack * release;
        let s = crate::gfx::sinf(t * 440.0 * 6.2831853) * 0.26
            + crate::gfx::sinf(t * 554.37 * 6.2831853) * 0.19
            + crate::gfx::sinf(t * 659.25 * 6.2831853) * 0.14;
        let v = (s * env * 24000.0) as i16;
        let b = v.to_le_bytes();
        let o = i * 4;
        *p.add(o) = b[0];
        *p.add(o + 1) = b[1];
        *p.add(o + 2) = b[0];
        *p.add(o + 3) = b[1];
        i += 1;
    }
    let bd = core::ptr::addr_of_mut!(BD) as *mut u32;
    *bd = core::ptr::addr_of!(PCM) as u32;
    *bd.add(1) = (frames * 4) as u32;
    *bd.add(2) = 0;
    *bd.add(3) = 0;
    outl(NABM + 0x10, bd as u32);
    outb(NABM + 0x14, 0);
    outb(NABM + 0x15, 0);
    outw(NABM + 0x16, 0x0000);
    outb(NABM + 0x1B, 0x01);
    let mut guard = 0u32;
    while guard < 400_000 {
        io_delay();
        guard += 1;
    }
    serial_print("[audio] civ/lvi");
    serial_u32(inb(NABM + 0x14) as u32);
    serial_u32(inb(NABM + 0x15) as u32);
    serial_print("[audio] chime");
}
static mut PLEN: usize = 0;

pub unsafe fn stop() {
    if !AC_OK { return; }
    outb(NABM + 0x1B, 0x00);
    PLEN = 0;
}

unsafe fn start(frames: usize) {
    let bd = core::ptr::addr_of_mut!(BD) as *mut u32;
    *bd = core::ptr::addr_of!(PCM) as u32;
    *bd.add(1) = (frames * 4) as u32;
    *bd.add(2) = 0;
    *bd.add(3) = 0;
    outl(NABM + 0x10, bd as u32);
    outb(NABM + 0x14, 0);
    outb(NABM + 0x15, 0);
    outw(NABM + 0x16, 0x0000);
    outb(NABM + 0x1B, 0x01);
    PLEN = frames;
}

fn le16(d: &[u8], o: usize) -> u16 { d[o] as u16 | ((d[o + 1] as u16) << 8) }
fn le32(d: &[u8], o: usize) -> u32 { d[o] as u32 | ((d[o + 1] as u32) << 8) | ((d[o + 2] as u32) << 16) | ((d[o + 3] as u32) << 24) }

pub unsafe fn play_wav(d: &[u8]) -> bool {
    if !AC_OK || d.len() < 44 { return false; }
    if &d[0..4] != b"RIFF" || &d[8..12] != b"WAVE" { return false; }
    let mut off = 12usize;
    let mut ch = 1u16;
    let mut rate = 48000u32;
    let mut bits = 16u16;
    let mut data_off = 0usize;
    let mut data_len = 0usize;
    while off + 8 <= d.len() {
        let id = &d[off..off + 4];
        let sz = le32(d, off + 4) as usize;
        let body = off + 8;
        if id == b"fmt " && body + 16 <= d.len() {
            ch = le16(d, body + 2);
            rate = le32(d, body + 4);
            bits = le16(d, body + 14);
        } else if id == b"data" {
            data_off = body;
            data_len = if body + sz <= d.len() { sz } else { d.len().saturating_sub(body) };
            break;
        }
        off = body + sz + (sz & 1);
    }
    if data_off == 0 || data_len == 0 || bits != 16 || (ch != 1 && ch != 2) || rate == 0 {
        return false;
    }
    let bps = (ch as usize) * 2;
    let total = data_len / bps;
    let out_rate = 48000u32;
    let max_frames = 24000usize;
    let want = total * out_rate as usize / rate as usize;
    let frames = if want < max_frames { want } else { max_frames };
    let p = core::ptr::addr_of_mut!(PCM) as *mut u8;
    let mut i = 0usize;
    while i < frames {
        let src = (i * rate as usize / out_rate as usize) * bps + data_off;
        let mut l = 0i16;
        let mut r = 0i16;
        if src + 2 <= data_off + data_len {
            l = (d[src] as i16) | ((d[src + 1] as i16) << 8);
            r = if ch == 2 && src + 4 <= data_off + data_len {
                (d[src + 2] as i16) | ((d[src + 3] as i16) << 8)
            } else { l };
        }
        let o = i * 4;
        *p.add(o) = l as u8;
        *p.add(o + 1) = (l >> 8) as u8;
        *p.add(o + 2) = r as u8;
        *p.add(o + 3) = (r >> 8) as u8;
        i += 1;
    }
    start(frames);
    true
}

pub unsafe fn tone_seq() -> bool {
    if !AC_OK { return false; }
    let rate = 48000usize;
    let notes = [440u32, 554, 659, 880];
    let per = rate / 4;
    let p = core::ptr::addr_of_mut!(PCM) as *mut u8;
    let mut i = 0usize;
    while i < per * notes.len() {
        let n = i / per;
        let t = (i % per) as f32 / rate as f32;
        let env = if t < 0.01 { t * 100.0 } else if t > 0.20 { (0.25 - t).max(0.0) * 4.0 } else { 1.0 };
        let s = crate::gfx::sinf(t * notes[n] as f32 * 6.2831853) * 0.22 * env;
        let v = (s * 30000.0) as i16;
        let o = i * 4;
        *p.add(o) = v as u8;
        *p.add(o + 1) = (v >> 8) as u8;
        *p.add(o + 2) = v as u8;
        *p.add(o + 3) = (v >> 8) as u8;
        i += 1;
    }
    start(per * notes.len());
    true
}

pub fn playing() -> bool {
    unsafe { PLEN > 0 }
}
