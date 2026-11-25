#![allow(dead_code)]

use crate::efi::*;
use core::ffi::c_void;

pub const GUID_TEXT_INPUT_EX: Guid = Guid::new(
    0xdd9e7534,
    0x7762,
    0x4698,
    [0x8c, 0x14, 0xf5, 0x85, 0x17, 0xa6, 0x25, 0xaa],
);

pub const GUID_ABS_POINTER: Guid = Guid::new(
    0x8d59d00a,
    0x1b1d,
    0x4ba0,
    [0x8e, 0x1f, 0x8c, 0x1c, 0x0a, 0x7c, 0x3a, 0x2c],
);

#[repr(C)]
struct KeyState {
    shift: u32,
    toggle: u8,
    pad: [u8; 3],
}

#[repr(C)]
struct Key {
    scan: u16,
    ch: u16,
}

#[repr(C)]
struct KeyData {
    state: KeyState,
    key: Key,
}

#[repr(C)]
struct TextInputEx {
    reset: usize,
    read: usize,
    wait: usize,
    set_state: usize,
    register: usize,
    unregister: usize,
}

#[repr(C)]
struct AbsMode {
    minx: u64,
    maxx: u64,
    miny: u64,
    maxy: u64,
    resx: u32,
    resy: u32,
    attrs: u32,
}

#[repr(C)]
struct AbsState {
    x: u64,
    y: u64,
    buttons: u32,
}

#[repr(C)]
struct AbsPointer {
    reset: usize,
    get_state: usize,
    wait: usize,
    mode: *mut AbsMode,
}

static mut TIEX: *mut c_void = core::ptr::null_mut();
static mut ABS: *mut AbsPointer = core::ptr::null_mut();
static mut ABS_LAST: (i32, i32) = (-1, -1);

pub unsafe fn init() {
    TIEX = bs_locate_protocol(&GUID_TEXT_INPUT_EX);
    ABS = bs_locate_protocol(&GUID_ABS_POINTER) as *mut AbsPointer;
    if !TIEX.is_null() {
        serial_print("[input] SimpleTextInputEx ready");
    }
    if !ABS.is_null() {
        serial_print("[input] AbsolutePointer ready");
    }
}

pub fn modern_keyboard() -> bool {
    unsafe { !TIEX.is_null() }
}

pub fn absolute_pointer() -> bool {
    unsafe { !ABS.is_null() }
}

pub unsafe fn read_key_ex() -> Option<(u16, u16)> {
    if TIEX.is_null() {
        return None;
    }
    let t = TIEX as *mut TextInputEx;
    let f: extern "efiapi" fn(*mut TextInputEx, *mut KeyData) -> Status =
        core::mem::transmute((*t).read);
    let mut kd = KeyData {
        state: KeyState { shift: 0, toggle: 0, pad: [0; 3] },
        key: Key { scan: 0, ch: 0 },
    };
    if f(t, &mut kd) == EFI_SUCCESS {
        Some((kd.key.scan, kd.key.ch))
    } else {
        None
    }
}

pub unsafe fn abs_mouse(w: i32, h: i32) -> Option<(i32, i32, u8, bool)> {
    if ABS.is_null() || w <= 1 || h <= 1 {
        return None;
    }
    let p = ABS;
    let f: extern "efiapi" fn(*mut AbsPointer, *mut AbsState) -> Status =
        core::mem::transmute((*p).get_state);
    let mut st = AbsState { x: 0, y: 0, buttons: 0 };
    if f(p, &mut st) != EFI_SUCCESS {
        return None;
    }
    let m = (*p).mode;
    if m.is_null() {
        return None;
    }
    let (minx, maxx, miny, maxy) = ((*m).minx, (*m).maxx, (*m).miny, (*m).maxy);
    if maxx <= minx || maxy <= miny {
        return None;
    }
    let mut sx = ((st.x.saturating_sub(minx)) * (w as u64) / (maxx - minx)) as i32;
    let mut sy = ((st.y.saturating_sub(miny)) * (h as u64) / (maxy - miny)) as i32;
    if sx < 0 { sx = 0; }
    if sy < 0 { sy = 0; }
    if sx > w - 1 { sx = w - 1; }
    if sy > h - 1 { sy = h - 1; }
    let moved = ABS_LAST.0 != sx || ABS_LAST.1 != sy;
    ABS_LAST = (sx, sy);
    Some((sx, sy, (st.buttons & 3) as u8, moved))
}

static mut DESC: [u8; 80] = [0; 80];

fn put(dst: &mut [u8], l: &mut usize, s: &str) {
    for c in s.bytes() {
        if *l < dst.len() - 1 {
            dst[*l] = c;
            *l += 1;
        }
    }
}

pub fn describe() -> &'static str {
    unsafe {
        let d = &mut *core::ptr::addr_of_mut!(DESC);
        let mut l = 0usize;
        let kb = if modern_keyboard() {
            "UEFI Ex 键盘"
        } else if crate::usbhid::kbd_present() {
            "USB HID 键盘"
        } else if crate::ps2::keyboard_ok() {
            "PS/2 键盘"
        } else {
            "固件键盘"
        };
        let ms = if absolute_pointer() {
            "UEFI 绝对指针"
        } else if crate::hw::pointer_present() {
            "UEFI SimplePointer"
        } else if crate::usbhid::mouse_absolute() {
            "USB HID 绝对鼠标"
        } else if crate::usbhid::mouse_present() {
            "USB HID 鼠标"
        } else if crate::ps2::mouse_ok() {
            "PS/2 鼠标"
        } else {
            "无指针设备"
        };
        put(d, &mut l, kb);
        put(d, &mut l, " + ");
        put(d, &mut l, ms);
        core::str::from_utf8_unchecked(&d[..l])
    }
}