#![allow(dead_code)]

use crate::efi::*;
use crate::net::Nic;

pub const SUBSYS_TOTAL: usize = 8;

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
    let mut n = 0;
    for s in subs() {
        if s.state == State::Ready { n += 1; }
    }
    n
}

pub fn ready_or_degraded() -> usize {
    let mut n = 0;
    for s in subs() {
        if s.state != State::Down { n += 1; }
    }
    n
}

pub fn state_name(s: State) -> &'static str {
    match s {
        State::Ready => "就绪",
        State::Degraded => "降级",
        State::Down => "未就绪",
    }
}

unsafe fn set(idx: usize, state: State, detail: &'static str) {
    let t = &mut *core::ptr::addr_of_mut!(SUBS);
    t[idx].state = state;
    t[idx].detail = detail;
}

/// 在所有内核子系统完成初始化后调用；UI 由 ui_ready 单独标记。
pub unsafe fn init() {
    let vn = crate::video::name();
    set(0, if vn.len() > 0 { State::Ready } else { State::Down }, vn);
    let id = crate::input::describe();
    set(1, if id.len() > 0 { State::Ready } else { State::Down }, id);
    set(2, State::Ready, "FAT16/FAT32 + VFS");
    let nic = crate::net::info();
    if nic.present {
        set(3, State::Ready, nic.name);
    } else if crate::hw::net().present {
        set(3, State::Degraded, "UEFI SNP（固件栈）");
    } else {
        set(3, State::Down, "未发现网卡");
    }
    set(4, if crate::audio::ok() { State::Ready } else { State::Degraded }, crate::audio::name());
    set(5, State::Down, "Compositor");
    set(6, State::Ready, "CPL3 + dual page tables");
    if nic.present || crate::hw::net().present {
        set(7, State::Ready, "GitHub Releases");
    } else {
        set(7, State::Degraded, "离线更新包");
    }
    serial_print("[kernel] monolithic subsystems");
    serial_u32(ready_count() as u32);
}

pub unsafe fn ui_ready() {
    set(5, State::Ready, "Compositor + Dock");
}

pub unsafe fn status_line(out: &mut [u8]) -> usize {
    let mut l = 0usize;
    for c in "宏内核 · ".bytes() { if l < out.len() { out[l] = c; l += 1; } }
    l += crate::hw::num2(&mut out[l..], ready_count() as u32, 1);
    for c in "/".bytes() { if l < out.len() { out[l] = c; l += 1; } }
    l += crate::hw::num2(&mut out[l..], SUBSYS_TOTAL as u32, 1);
    for c in " 子系统就绪".bytes() { if l < out.len() { out[l] = c; l += 1; } }
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
            while i < a2 as usize { b[i] = *p.add(i); i += 1; }
            KCON_LEN = a2 as usize;
            SysResult::Console(&b[..KCON_LEN])
        }
        2 => SysResult::Exit,
        3 => SysResult::Value(uptime() as u64),
        _ => SysResult::Unknown,
    }
}
