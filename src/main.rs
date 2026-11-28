#![no_std]
#![no_main]

mod anim;
mod apps;
mod audio;
mod cjk;
mod diskfmt;
mod http;
mod input;
mod installer;
mod logo;
mod net;
mod netstack;
mod port;
mod ps2;
mod ckernel;
mod kernel;
mod ring3;
mod updater;
mod usbhid;
mod version;
mod video;
mod efi;
mod font;
mod fs;
mod gfx;
mod hw;
mod icon;
mod ui;

use core::panic::PanicInfo;
use efi::*;
use gfx::{rgb, Gfx};

#[export_name = "efi_main"]
pub extern "efiapi" fn efi_main(image: Handle, systab: *mut SystemTable) -> Status {
    unsafe {
        ST = systab;
        if ST.is_null() {
            return 1;
        }
        BS = (*ST).boot_services;
        installer::set_image(image);
        debug_print("HYWOS 1.1.6 (x86_64 UEFI, hybrid C/C++ kernel + Rust UI)");
        let _ = version::VERSION;
        hw::firmware_vendor();
        hw::cache_time();
        debug_print("Firmware detected");
        bs_stall(60_000);

        let gop = Gfx::init();
        let g = video::probe(gop);
        if !g.ok {
            debug_print("FATAL: no usable graphics device");
            loop { bs_stall(1_000_000); }
        }
        debug_print("[stage] gfx ok");
        g.clear(rgb(0, 0, 0));
        debug_print("[stage] clear ok");

        let vols = fs::fs_init();
        serial_print("[fs] volumes");
        serial_u32(vols as u32);
        debug_print("[stage] fs init done");
        let mut vk = 0;
        while vk < vols {
            serial_print("[fs] probe vol");
            serial_u32(vk as u32);
            serial_print(if fs::exists_vol(vk, "\\HYWOS\\CD") { "cd=1" } else { "cd=0" });
            serial_print(if fs::exists_vol(vk, "\\EFI\\BOOT\\BOOTX64.EFI") { "efi=1" } else { "efi=0" });
            vk += 1;
        }
        if fs::is_install_media() {
            debug_print("[stage] install media detected");
            installer::run(&g);
        }
        fs::init_home();
        debug_print("[stage] home ready");

        let mut boot = anim::BootAnim::new(&g);
        debug_print("[stage] boot anim begin");
        let mut next_report = 0u32;
        loop {
            if boot.frame() >= next_report {
                debug_print("[stage] anim frame");
                next_report += 8;
            }
            if boot.step(&g) {
                break;
            }
            bs_stall(34_000);
        }
        debug_print("[stage] boot anim done");

        g.clear(rgb(4, 5, 10));
        g.text_center(g.w as i32 / 2, g.h as i32 / 2 - 16, "HYWOS", rgb(230, 242, 255), 3);
        debug_print("[stage] icon preload begin");
        icon::preload();
        debug_print("[stage] icon preload done");

        hw::acpi_probe();
        debug_print("[stage] acpi done");
        hw::net_probe();
        if net::init() {
            debug_print("[net] driver bound");
        } else {
            debug_print("[net] firmware only");
        }
        debug_print("[stage] net done");
        input::init();
        hw::pointer_init();
        let fw_kbd = !(*ST).con_in.is_null();
        if !input::absolute_pointer() && !hw::pointer_present() {
            if ps2::init_mouse() {
                debug_print("[input] PS/2 mouse ready");
            } else {
                debug_print("[input] PS/2 mouse unavailable");
            }
        }
        let kbd_ok = ps2::init_keyboard();
        ps2::keyboard_from_firmware(fw_kbd);
        if !fw_kbd {
            ps2::keyboard_takeover();
        }
        if kbd_ok {
            debug_print("[input] PS/2 keyboard standby");
        }
        let need_mouse = !input::absolute_pointer() && !hw::pointer_present() && !ps2::mouse_ok();
        if need_mouse || !fw_kbd {
            if usbhid::init(!fw_kbd, need_mouse) {
                debug_print("[input] USB HID ready");
            }
        }
        serial_print("[input] firmware input kept in control");
        audio::init();
        kernel::init();
        debug_print("[stage] pointer done");
        hw::cache_time();
        debug_print("[stage] rtc cached");
        debug_print("filesystem volumes mounted");
        debug_print("ACPI MADT parsed");
        if hw::net().present {
            debug_print("network adapter detected");
        }
        if hw::pointer_present() {
            debug_print("simple pointer ready");
        }
        let _ = vols;

        debug_print("[stage] ui begin");
        kernel::ui_ready();
        let mut ui = ui::Ui::new(g);
        audio::chime();
        debug_print("[stage] ui loop");
        loop {
            ui.tick();
            bs_stall(8_000);
        }
    }
}

struct SerialSink;

impl core::fmt::Write for SerialSink {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        serial_print(s);
        Ok(())
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use core::fmt::Write;
    serial_print("!!! HYWOS kernel panic !!!");
    let mut sink = SerialSink;
    let _ = write!(&mut sink, "{}", info);
    serial_print("system halted");
    loop {
        unsafe { bs_stall(1_000_000) }
    }
}

#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    let mut i = 0;
    while i + 8 <= n {
        *(dest.add(i) as *mut u64) = *(src.add(i) as *const u64);
        i += 8;
    }
    while i < n {
        *dest.add(i) = *src.add(i);
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as usize) < (src as usize) {
        let mut i = 0;
        while i < n {
            *dest.add(i) = *src.add(i);
            i += 1;
        }
    } else if (dest as usize) > (src as usize) {
        let mut i = n;
        while i > 0 {
            i -= 1;
            *dest.add(i) = *src.add(i);
        }
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, c: i32, n: usize) -> *mut u8 {
    let v = c as u8;
    let mut i = 0;
    while i < n {
        *dest.add(i) = v;
        i += 1;
    }
    dest
}

#[no_mangle]
pub unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let x = *a.add(i);
        let y = *b.add(i);
        if x != y {
            return x as i32 - y as i32;
        }
        i += 1;
    }
    0
}

#[no_mangle]
pub unsafe extern "C" fn bcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    memcmp(a, b, n)
}

#[no_mangle]
pub unsafe extern "C" fn strlen(s: *const u8) -> usize {
    let mut n = 0;
    while *s.add(n) != 0 {
        n += 1;
    }
    n
}