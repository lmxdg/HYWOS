#![allow(dead_code)]

//! 显示与音频偏好：亮度、音量、缩放、分辨率，持久化到用户卷。

use crate::fs;

const PATH: &str = "\\home\\user\\.apps\\display.kv";

static mut BRIGHT: u32 = 100;
static mut VOL: u32 = 80;
static mut SCALE: u32 = 100;
static mut RES_W: u32 = 1024;
static mut RES_H: u32 = 768;

pub fn brightness() -> u32 { unsafe { BRIGHT } }
pub fn volume() -> u32 { unsafe { VOL } }
pub fn scale() -> u32 { unsafe { SCALE } }
pub fn width() -> u32 { unsafe { RES_W } }
pub fn height() -> u32 { unsafe { RES_H } }

unsafe fn parse_u32(b: &[u8], key: &[u8]) -> Option<u32> {
    let mut i = 0;
    while i + key.len() < b.len() {
        let mut ok = true;
        let mut k = 0;
        while k < key.len() {
            if b[i + k] != key[k] { ok = false; break; }
            k += 1;
        }
        if ok && b[i + key.len()] == b'=' {
            let mut v = 0u32;
            let mut j = i + key.len() + 1;
            let mut any = false;
            while j < b.len() && b[j] >= b'0' && b[j] <= b'9' {
                v = v * 10 + (b[j] - b'0') as u32;
                j += 1;
                any = true;
            }
            if any { return Some(v); }
        }
        i += 1;
    }
    None
}

pub unsafe fn load() {
    if let Some(n) = fs::read_file(fs::usr_vol(), PATH) {
        let b = &fs::rdbuf()[..n];
        if let Some(v) = parse_u32(b, b"brightness") { BRIGHT = if v > 100 { 100 } else { v }; }
        if let Some(v) = parse_u32(b, b"volume") { VOL = if v > 100 { 100 } else { v }; }
        if let Some(v) = parse_u32(b, b"scale") { SCALE = if v < 100 { 100 } else if v > 150 { 150 } else { v }; }
        if let Some(v) = parse_u32(b, b"width") { if v >= 800 && v <= 1920 { RES_W = v; } }
        if let Some(v) = parse_u32(b, b"height") { if v >= 600 && v <= 1080 { RES_H = v; } }
    }
    crate::gfx::set_brightness(BRIGHT as u8);
    crate::audio::set_volume(VOL as u8);
}

fn put_s(buf: &mut [u8], l: &mut usize, s: &[u8]) {
    for c in s {
        if *l < buf.len() { buf[*l] = *c; *l += 1; }
    }
}

fn put_n(buf: &mut [u8], l: &mut usize, mut v: u32) {
    let mut d = [0u8; 10];
    let mut n = 0;
    if v == 0 { d[0] = b'0'; n = 1; }
    while v > 0 {
        d[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    while n > 0 {
        n -= 1;
        if *l < buf.len() { buf[*l] = d[n]; *l += 1; }
    }
}

unsafe fn save() {
    let mut b = [0u8; 128];
    let mut l = 0usize;
    put_s(&mut b, &mut l, b"brightness="); put_n(&mut b, &mut l, BRIGHT);
    put_s(&mut b, &mut l, b"\nvolume="); put_n(&mut b, &mut l, VOL);
    put_s(&mut b, &mut l, b"\nscale="); put_n(&mut b, &mut l, SCALE);
    put_s(&mut b, &mut l, b"\nwidth="); put_n(&mut b, &mut l, RES_W);
    put_s(&mut b, &mut l, b"\nheight="); put_n(&mut b, &mut l, RES_H);
    put_s(&mut b, &mut l, b"\n");
    fs::write_file(fs::usr_vol(), PATH, &b[..l]);
}

pub unsafe fn set_brightness(v: u32) {
    BRIGHT = if v > 100 { 100 } else { v };
    crate::gfx::set_brightness(BRIGHT as u8);
    save();
}

pub unsafe fn set_volume(v: u32) {
    VOL = if v > 100 { 100 } else { v };
    crate::audio::set_volume(VOL as u8);
    save();
}

pub unsafe fn set_scale(v: u32) {
    SCALE = if v < 100 { 100 } else if v > 150 { 150 } else { v };
    save();
}

pub unsafe fn set_resolution(w: u32, h: u32) {
    RES_W = w;
    RES_H = h;
    save();
}