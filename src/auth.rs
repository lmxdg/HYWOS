#![allow(dead_code)]

//! 用户密码：设置、校验与启动锁屏。
//! 密码使用 FNV-1a 哈希保存到用户卷，不保存明文。

use crate::efi::*;
use crate::fs;
use crate::gfx::{rgb, Gfx};
use crate::logo::{LOGO_RGBA, LOGO_SIZE};

const ACCOUNT_PATH: &str = "\\home\\user\\.apps\\account.kv";

fn hash(pw: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut i = 0;
    while i < pw.len() {
        h ^= pw[i] as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
        i += 1;
    }
    h
}

fn parse_hex(s: &[u8], off: usize) -> Option<u64> {
    if off + 16 > s.len() {
        return None;
    }
    let mut v: u64 = 0;
    let mut i = 0;
    while i < 16 {
        let c = s[off + i];
        let d = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => return None,
        };
        v = (v << 4) | d as u64;
        i += 1;
    }
    Some(v)
}

fn write_hex(v: u64, out: &mut [u8; 16]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut i = 0;
    while i < 16 {
        out[i] = HEX[((v >> ((15 - i) * 4)) & 0xf) as usize];
        i += 1;
    }
}

/// 读取已保存的密码哈希；未设置密码时返回 None。
pub unsafe fn stored_hash() -> Option<u64> {
    let n = fs::read_file(fs::usr_vol(), ACCOUNT_PATH)?;
    let buf = &fs::rdbuf()[..n];
    let mut i = 0;
    while i + 5 <= buf.len() {
        if buf[i] == b'H' && buf[i + 1] == b'A' && buf[i + 2] == b'S' && buf[i + 3] == b'H' && buf[i + 4] == b' ' {
            return parse_hex(buf, i + 5);
        }
        i += 1;
    }
    None
}

pub unsafe fn is_set() -> bool {
    stored_hash().is_some()
}

/// 未设置密码时直接放行。
pub unsafe fn verify(pw: &[u8]) -> bool {
    match stored_hash() {
        Some(h) => h == hash(pw),
        None => true,
    }
}

pub unsafe fn set_password(pw: &[u8]) -> bool {
    if pw.is_empty() {
        return false;
    }
    let mut line = [0u8; 24];
    line[0] = b'H';
    line[1] = b'A';
    line[2] = b'S';
    line[3] = b'H';
    line[4] = b' ';
    let mut hx = [0u8; 16];
    write_hex(hash(pw), &mut hx);
    let mut i = 0;
    while i < 16 {
        line[5 + i] = hx[i];
        i += 1;
    }
    line[21] = b'\n';
    fs::write_file(fs::usr_vol(), ACCOUNT_PATH, &line[..22])
}

pub unsafe fn clear_password() -> bool {
    fs::write_file(fs::usr_vol(), ACCOUNT_PATH, b"")
}

unsafe fn draw_lock(g: &Gfx, input: &[u8], err: bool, frame: u32) {
    let w = g.w as i32;
    let h = g.h as i32;
    g.vgrad(0, 0, w, h, rgb(8, 13, 30), rgb(3, 4, 10));
    g.ring(w / 2, h / 2 - 60, 220, 1, rgb(60, 116, 190), 40);
    g.ring(w / 2, h / 2 - 60, 320, 1, rgb(46, 90, 150), 24);

    let lw = 104i32;
    let lx = w / 2 - lw / 2;
    let ly = h / 2 - 220;
    g.blit_rgba(lx, ly, lw, lw, &LOGO_RGBA, LOGO_SIZE as i32, LOGO_SIZE as i32);
    g.text_center(w / 2, ly + lw + 16, "HYWOS", rgb(226, 240, 255), 3);
    g.text_center(w / 2, ly + lw + 58, "user", rgb(176, 190, 214), 1);

    let bw = 360i32;
    let bx = w / 2 - bw / 2;
    let by = h / 2 + 56;
    g.round_rect(bx, by, bw, 48, 12, rgb(18, 24, 40), 235);
    g.round_rect_outline(
        bx,
        by,
        bw,
        48,
        12,
        if err { rgb(210, 96, 96) } else { rgb(80, 120, 190) },
        200,
    );
    if input.is_empty() {
        g.text(bx + 20, by + 16, "输入密码", rgb(110, 126, 152), 1);
    } else {
        let n = if input.len() > 16 { 16 } else { input.len() };
        let mut i = 0;
        while i < n {
            g.circle(bx + 20 + i as i32 * 18, by + 24, 5, rgb(198, 220, 248), 240);
            i += 1;
        }
    }
    if (frame / 18) % 2 == 0 {
        let cx = bx + 20 + if input.len() > 16 { 16 * 18 } else { input.len() as i32 * 18 };
        g.rect(cx, by + 14, 2, 20, rgb(140, 190, 255), 200);
    }
    let (msg, col) = if err {
        ("密码错误，请重试", rgb(230, 110, 110))
    } else {
        ("输入密码后按 Enter 解锁", rgb(150, 168, 196))
    };
    g.text_center(w / 2, by + 68, msg, col, 1);
    g.text_center(w / 2, h - 46, "HYWOS 1.2.4 · 锁屏", rgb(90, 104, 130), 1);
}

/// 启动时若已设置密码，阻塞等待用户输入正确密码。
pub unsafe fn unlock(g: &Gfx) {
    if !is_set() {
        return;
    }
    let mut buf = [0u8; 32];
    let mut len = 0usize;
    let mut err = false;
    let mut frame = 0u32;
    loop {
        draw_lock(g, &buf[..len], err, frame);
        frame = frame.wrapping_add(1);
        if let Some((scan, ch)) = read_key() {
            if ch == 13 {
                if verify(&buf[..len]) {
                    serial_print("[auth] unlocked");
                    break;
                }
                err = true;
                len = 0;
            } else if ch == 8 || scan == 0x08 {
                if len > 0 {
                    len -= 1;
                }
                err = false;
            } else if scan == 0x17 {
                len = 0;
                err = false;
            } else if ch >= 32 && ch < 127 && len < buf.len() {
                buf[len] = ch as u8;
                len += 1;
                err = false;
            }
        }
        bs_stall(20_000);
    }
}