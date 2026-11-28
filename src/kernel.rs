#![allow(dead_code)]

//! 混合内核前端：子系统状态由 C/C++ 内核核心维护，Rust 负责探测与上报。
//! 终端、关于本机与安全中心通过 `subs()` / `ready_count()` 显示混合内核状态。

use crate::ckernel;
use crate::efi::*;
use crate::net::Nic;

pub const SUBSYS_TOTAL: usize = 9;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum State {
    Down = 0,
    Degraded = 1,
    Ready = 2,
}

#[derive(Clone, Copy)]
pub struct Subsystem {
    pub name: &'static str,
    pub state: State,
    pub detail: &'static str,
}

static mut SUBS: [Subsystem; SUBSYS_TOTAL] = [
    Subsystem { name: "Kernel Core", state: State::Down, detail: "" },
    Subsystem { name: "Video", state: State::Down, detail: "" },
    Subsystem { name: "Input", state: State::Down, detail: "" },
    Subsystem { name: "Storage", state: State::Down, detail: "" },
    Subsystem { name: "Network", state: State::Down, detail: "" },
    Subsystem { name: "Audio", state: State::Down, detail: "" },
    Subsystem { name: "UI", state: State::Down, detail: "" },
    Subsystem { name: "Ring3", state: State::Down, detail: "" },
    Subsystem { name: "Updates", state: State::Down, detail: "" },
];

pub fn subs() -> &'static [Subsystem; SUBSYS_TOTAL] {
    unsafe { &*core::ptr::addr_of!(SUBS) }
}

pub fn ready_count() -> usize {
    ckernel::ready()
}

pub fn ready_or_degraded() -> usize {
    ckernel::active()
}

pub fn state_name(s: State) -> &'static str {
    match s {
        State::Ready => "就绪",
        State::Degraded => "降级",
        State::Down => "未就绪",
    }
}

fn from_ck(v: u8) -> State {
    match v {
        2 => State::Ready,
        1 => State::Degraded,
        _ => State::Down,
    }
}

/// 把 C/C++ 内核中的权威状态快照回 Rust 表，供 UI 直接读取。
unsafe fn sync_from_kernel() {
    let t = &mut *core::ptr::addr_of_mut!(SUBS);
    let n = ckernel::subsys_count();
    let mut i = 0;
    while i < SUBSYS_TOTAL {
        if i < n {
            t[i].name = ckernel::subsys_name(i);
            t[i].detail = ckernel::subsys_detail(i);
            t[i].state = from_ck(ckernel::subsys_state(i));
        } else {
            t[i].state = State::Down;
        }
        i += 1;
    }
}

/// 初始化 C/C++ 混合内核核心，上报各子系统探测结果，再同步回 UI 表。
pub unsafe fn init() {
    ckernel::init();

    let vn = crate::video::name();
    ckernel::set_state(
        ckernel::SUB_VIDEO,
        if !vn.is_empty() { ckernel::ST_READY } else { ckernel::ST_DOWN },
        vn,
    );

    let id = crate::input::describe();
    ckernel::set_state(
        ckernel::SUB_INPUT,
        if !id.is_empty() { ckernel::ST_READY } else { ckernel::ST_DOWN },
        id,
    );

    ckernel::set_state(ckernel::SUB_STORAGE, ckernel::ST_READY, "FAT16/FAT32 + VFS");

    let nic = crate::net::info();
    if nic.present {
        ckernel::set_state(ckernel::SUB_NETWORK, ckernel::ST_READY, nic.name);
    } else if crate::hw::net().present {
        ckernel::set_state(ckernel::SUB_NETWORK, ckernel::ST_DEGRADED, "UEFI SNP（固件栈）");
    } else {
        ckernel::set_state(ckernel::SUB_NETWORK, ckernel::ST_DOWN, "未发现网卡");
    }

    ckernel::set_state(
        ckernel::SUB_AUDIO,
        if crate::audio::ok() { ckernel::ST_READY } else { ckernel::ST_DEGRADED },
        crate::audio::name(),
    );
    ckernel::set_state(ckernel::SUB_UI, ckernel::ST_DOWN, "Compositor");
    ckernel::set_state(ckernel::SUB_RING3, ckernel::ST_READY, "CPL3 + dual page tables");
    if nic.present || crate::hw::net().present {
        ckernel::set_state(ckernel::SUB_UPDATES, ckernel::ST_READY, "GitHub Releases");
    } else {
        ckernel::set_state(ckernel::SUB_UPDATES, ckernel::ST_DEGRADED, "离线更新包");
    }

    sync_from_kernel();

    serial_print("[kernel] hybrid C/C++ core, drivers bound=");
    serial_u32(ckernel::driver_bound() as u32);
    let st = ckernel::selftest();
    if st != 0 {
        serial_print("[kernel] selftest failed code=");
        serial_u32(st as u32);
    }
    serial_print("[kernel] hybrid subsystems ready=");
    serial_u32(ready_count() as u32);
}

pub unsafe fn ui_ready() {
    ckernel::set_state(ckernel::SUB_UI, ckernel::ST_READY, "Compositor + Dock");
    sync_from_kernel();
}

pub unsafe fn status_line(out: &mut [u8]) -> usize {
    let mut l = 0usize;
    for c in "混合内核 · ".bytes() {
        if l < out.len() {
            out[l] = c;
            l += 1;
        }
    }
    l += crate::hw::num2(&mut out[l..], ready_count() as u32, 1);
    for c in "/".bytes() {
        if l < out.len() {
            out[l] = c;
            l += 1;
        }
    }
    l += crate::hw::num2(&mut out[l..], SUBSYS_TOTAL as u32, 1);
    for c in " · 驱动 ".bytes() {
        if l < out.len() {
            out[l] = c;
            l += 1;
        }
    }
    l += crate::hw::num2(&mut out[l..], ckernel::driver_bound() as u32, 1);
    for c in "/".bytes() {
        if l < out.len() {
            out[l] = c;
            l += 1;
        }
    }
    l += crate::hw::num2(&mut out[l..], ckernel::driver_count() as u32, 1);
    l
}

pub fn cores() -> u32 {
    crate::hw::cores() as u32
}

pub unsafe fn mem_stats() -> (u64, u64) {
    crate::hw::mem_kb()
}

pub fn uptime() -> u32 {
    crate::hw::uptime_secs()
}

pub fn video_name() -> &'static str {
    crate::video::name()
}

pub fn input_desc() -> &'static str {
    crate::input::describe()
}

pub fn audio_name() -> &'static str {
    crate::audio::name()
}

pub fn net_info() -> &'static Nic {
    crate::net::info()
}

pub unsafe fn ping(ip: [u8; 4], timeout_ms: u32) -> bool {
    crate::net::ping(ip, timeout_ms)
}

pub fn windows() -> usize {
    unsafe { crate::ui::window_count() }
}

pub fn driver_count() -> usize {
    ckernel::driver_count()
}

pub fn driver_bound() -> usize {
    ckernel::driver_bound()
}

pub fn driver_builtin() -> usize {
    ckernel::driver_builtin()
}

pub fn driver_module() -> usize {
    ckernel::driver_module()
}

pub fn driver_name(i: usize) -> &'static str {
    ckernel::driver_name(i)
}

pub fn driver_bus_name(i: usize) -> &'static str {
    ckernel::bus_name(ckernel::driver_bus(i))
}

pub fn driver_kind_name(i: usize) -> &'static str {
    ckernel::kind_name(ckernel::driver_kind(i))
}

pub fn driver_is_bound(i: usize) -> bool {
    ckernel::driver_is_bound(i)
}

pub fn kernel_kind() -> &'static str {
    ckernel::kind()
}

pub fn kernel_arch() -> &'static str {
    ckernel::arch()
}

pub fn kernel_ticks() -> u64 {
    ckernel::ticks()
}

/// 内核系统调用结果。Console 返回已拷贝到内核缓冲的用户输出。
pub enum SysResult {
    Value(u64),
    Console(&'static [u8]),
    Exit,
    Unknown,
}

static mut KCON: [u8; 512] = [0; 512];
static mut KCON_LEN: usize = 0;
pub static SYSCALL_NAMES: [&str; 3] = ["console_write", "exit", "uptime"];

/// 统一系统调用入口：ring3 只能通过这里访问内核服务。
pub unsafe fn syscall(num: u64, a1: u64, a2: u64) -> SysResult {
    match num {
        1 => {
            let base = crate::ring3::USER_BASE;
            if a1 < base || a1 >= base + 0x2000 || a2 > 256 {
                KCON_LEN = 0;
                return SysResult::Console(&[]);
            }
            let p = a1 as *const u8;
            let b = &mut *core::ptr::addr_of_mut!(KCON);
            let mut i = 0;
            while i < a2 as usize {
                b[i] = *p.add(i);
                i += 1;
            }
            KCON_LEN = a2 as usize;
            SysResult::Console(&b[..KCON_LEN])
        }
        2 => SysResult::Exit,
        3 => SysResult::Value(uptime() as u64),
        _ => SysResult::Unknown,
    }
}