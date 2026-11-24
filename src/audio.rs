#![allow(dead_code)]

use crate::efi::*;
use crate::port::*;

static mut AC_OK: bool = false;
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
    outw(NAM + 0x02, 0x0000);
    outw(NAM + 0x18, 0x0000);
    outw(NAM + 0x2C, 48000);
    outw(NAM + 0x2E, 48000);
    outb(NABM + 0x1B, 0x00);
    AC_OK = true;
    serial_print("[audio] AC'97 ready");
    true
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