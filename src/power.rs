#![allow(dead_code)]

//! 软关机与软重启：优先 UEFI RuntimeServices.ResetSystem，回退 ACPI PM1a 与 8042 复位。

use crate::efi::*;
use crate::port::*;

type ResetFn = extern "efiapi" fn(u32, usize, usize, *mut core::ffi::c_void);

unsafe fn reset_fn() -> Option<ResetFn> {
    let st = &*ST;
    let rt = st.runtime_services as *mut u8;
    if rt.is_null() {
        return None;
    }
    let p = *(rt.add(104) as *const usize);
    if p == 0 {
        return None;
    }
    Some(core::mem::transmute(p))
}

pub unsafe fn reboot() -> ! {
    serial_print("[power] reboot");
    if let Some(f) = reset_fn() {
        f(0, 0, 0, core::ptr::null_mut());
    }
    outb(0x64, 0xFE);
    loop {
        bs_stall(1_000_000);
    }
}

pub unsafe fn shutdown() -> ! {
    serial_print("[power] shutdown");
    if let Some(f) = reset_fn() {
        f(2, 0, 0, core::ptr::null_mut());
    }
    let pm = crate::hw::fadt_pm1a_cnt();
    if pm != 0 {
        outw(pm, (5u16 << 10) | (1u16 << 13));
    }
    loop {
        bs_stall(1_000_000);
    }
}

pub fn acpi_ready() -> bool {
    unsafe { crate::hw::fadt_present() }
}