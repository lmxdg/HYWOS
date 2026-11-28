#![allow(dead_code)]

//! C/C++ 混合内核的 Rust FFI 绑定。
//!
//! 内核核心与驱动总线运行在 `kernel/`（C/C++）；界面、桌面与硬件寄存器访问仍在
//! Rust。所有导出函数使用 `ms_abi`，因此这里统一用 `extern "efiapi"` 声明。

use core::ffi::{c_char, CStr};

pub const SUB_KERNEL: usize = 0;
pub const SUB_VIDEO: usize = 1;
pub const SUB_INPUT: usize = 2;
pub const SUB_STORAGE: usize = 3;
pub const SUB_NETWORK: usize = 4;
pub const SUB_AUDIO: usize = 5;
pub const SUB_UI: usize = 6;
pub const SUB_RING3: usize = 7;
pub const SUB_UPDATES: usize = 8;

pub const ST_DOWN: u8 = 0;
pub const ST_DEGRADED: u8 = 1;
pub const ST_READY: u8 = 2;

extern "efiapi" {
    fn hyw_kernel_init();
    fn hyw_kernel_kind() -> *const c_char;
    fn hyw_kernel_arch() -> *const c_char;
    fn hyw_kernel_version() -> *const c_char;
    fn hyw_kernel_caps() -> u32;

    fn hyw_subsys_count() -> u32;
    fn hyw_subsys_name(index: u32) -> *const c_char;
    fn hyw_subsys_detail(index: u32) -> *const c_char;
    fn hyw_subsys_state(index: u32) -> i32;
    fn hyw_subsys_ready() -> u32;
    fn hyw_subsys_active() -> u32;
    fn hyw_subsys_set(index: u32, state: i32, detail: *const c_char);
    fn hyw_subsys_register(name: *const c_char, detail: *const c_char) -> i32;

    fn hyw_kernel_ticks() -> u64;
    fn hyw_kernel_tick();

    fn hyw_driver_count() -> u32;
    fn hyw_driver_bound() -> u32;
    fn hyw_driver_builtin() -> u32;
    fn hyw_driver_module() -> u32;
    fn hyw_driver_name(index: u32) -> *const c_char;
    fn hyw_driver_bus(index: u32) -> i32;
    fn hyw_driver_kind(index: u32) -> i32;
    fn hyw_driver_is_bound(index: u32) -> i32;
    fn hyw_driver_bind_all() -> u32;

    fn hyw_selftest() -> i32;
}

unsafe fn as_str(p: *const c_char) -> &'static str {
    if p.is_null() {
        return "";
    }
    match CStr::from_ptr(p).to_str() {
        Ok(s) => s,
        Err(_) => "",
    }
}

/// `hyw_subsys_set` 需要一个以 NUL 结尾的 C 字符串，这里用固定缓冲转换。
static mut DETAIL_BUF: [u8; 96] = [0; 96];

pub fn init() {
    unsafe {
        hyw_kernel_init();
    }
}

pub fn kind() -> &'static str {
    unsafe { as_str(hyw_kernel_kind()) }
}

pub fn arch() -> &'static str {
    unsafe { as_str(hyw_kernel_arch()) }
}

pub fn version() -> &'static str {
    unsafe { as_str(hyw_kernel_version()) }
}

pub fn caps() -> u32 {
    unsafe { hyw_kernel_caps() }
}

pub fn subsys_count() -> usize {
    unsafe { hyw_subsys_count() as usize }
}

pub fn subsys_name(i: usize) -> &'static str {
    unsafe { as_str(hyw_subsys_name(i as u32)) }
}

pub fn subsys_detail(i: usize) -> &'static str {
    unsafe { as_str(hyw_subsys_detail(i as u32)) }
}

pub fn subsys_state(i: usize) -> u8 {
    unsafe {
        match hyw_subsys_state(i as u32) {
            2 => ST_READY,
            1 => ST_DEGRADED,
            _ => ST_DOWN,
        }
    }
}

pub fn ready() -> usize {
    unsafe { hyw_subsys_ready() as usize }
}

pub fn active() -> usize {
    unsafe { hyw_subsys_active() as usize }
}

pub fn set_state(i: usize, state: u8, detail: &str) {
    unsafe {
        let buf = &mut *core::ptr::addr_of_mut!(DETAIL_BUF);
        let src = detail.as_bytes();
        let mut n = 0usize;
        while n < src.len() && n + 1 < buf.len() {
            buf[n] = src[n];
            n += 1;
        }
        buf[n] = 0;
        hyw_subsys_set(i as u32, state as i32, buf.as_ptr() as *const c_char);
    }
}

pub fn ticks() -> u64 {
    unsafe { hyw_kernel_ticks() }
}

pub fn tick() {
    unsafe {
        hyw_kernel_tick();
    }
}

pub fn driver_count() -> usize {
    unsafe { hyw_driver_count() as usize }
}

pub fn driver_bound() -> usize {
    unsafe { hyw_driver_bound() as usize }
}

pub fn driver_builtin() -> usize {
    unsafe { hyw_driver_builtin() as usize }
}

pub fn driver_module() -> usize {
    unsafe { hyw_driver_module() as usize }
}

pub fn driver_name(i: usize) -> &'static str {
    unsafe { as_str(hyw_driver_name(i as u32)) }
}

pub fn driver_bus(i: usize) -> i32 {
    unsafe { hyw_driver_bus(i as u32) }
}

pub fn driver_kind(i: usize) -> i32 {
    unsafe { hyw_driver_kind(i as u32) }
}

pub fn driver_is_bound(i: usize) -> bool {
    unsafe { hyw_driver_is_bound(i as u32) != 0 }
}

pub fn driver_bind_all() -> usize {
    unsafe { hyw_driver_bind_all() as usize }
}

pub fn selftest() -> i32 {
    unsafe { hyw_selftest() }
}

pub fn bus_name(bus: i32) -> &'static str {
    match bus {
        1 => "PCI",
        2 => "USB",
        3 => "VIRTIO",
        4 => "PS/2",
        5 => "PLATFORM",
        _ => "NONE",
    }
}

pub fn kind_name(kind: i32) -> &'static str {
    if kind == 1 {
        "module"
    } else {
        "builtin"
    }
}