#![allow(dead_code)]

use crate::diskfmt;
use crate::efi::*;
use crate::fs;
use crate::gfx::{rgb, Gfx};
use core::ffi::c_void;

#[repr(C)]
struct LoadedImage {
    revision: u32,
    parent_handle: Handle,
    system_table: *mut c_void,
    device_handle: Handle,
    file_path: *mut c_void,
    reserved: *mut c_void,
    load_options_size: u32,
    load_options: *mut c_void,
    image_base: *mut c_void,
    image_size: usize,
}

static mut IMAGE: Handle = core::ptr::null_mut();

pub unsafe fn set_image(h: Handle) {
    IMAGE = h;
}

fn big2(g: &Gfx, x: i32, y: i32, ch: u8, c: u32, alpha: u8) {
    let idx = if ch >= 32 && ch <= 126 { (ch - 32) as usize } else { 0 };
    let bpg = crate::font::FONT_16X32_BPG;
    let glyph = &crate::font::FONT_16X32[idx * bpg..(idx + 1) * bpg];
    let mut gy = 0;
    while gy < 32 {
        let mut gx = 0;
        while gx < 16 {
            let byte = glyph[gy * 8 + gx / 2];
            let nib = if gx % 2 == 0 { byte >> 4 } else { byte & 0x0f };
            if nib > 0 {
                let a = ((nib as u32 * 17 * alpha as u32) / 255) as u8;
                g.rect(x + gx as i32 * 2, y + gy as i32 * 2, 2, 2, c, a);
            }
            gx += 1;
        }
        gy += 1;
    }
}

fn line(g: &Gfx, y: i32, s: &str, c: u32) {
    g.text(64, y, s, c, 2);
}

unsafe fn screen_confirm(g: &Gfx, cap_mb: u64, part_mb: u64) {
    g.clear(rgb(6, 8, 14));
    let mut x = 64;
    let title = "HYWOS";
    for ch in title.bytes() {
        big2(g, x, 44, ch, rgb(150, 210, 255), 255);
        x += 32;
    }
    g.text(64 + title.len() as i32 * 32 + 16, 60, "1.0.4 安装程序", rgb(220, 236, 255), 2);
    g.rect(64, 116, g.w as i32 - 128, 2, rgb(70, 110, 160), 200);
    line(g, 140, "安装介质 : HYWOS 1.0.4 ISO（UEFI 启动）", rgb(170, 190, 220));
    let mut b = [0u8; 96];
    let mut l = 0;
    for ch in "目标磁盘 : ".as_bytes().iter() { b[l] = *ch; l += 1; }
    l += crate::hw::num2(&mut b[l..], cap_mb as u32, 1);
    for ch in " MB（可安装分区 ".as_bytes().iter() { b[l] = *ch; l += 1; }
    l += crate::hw::num2(&mut b[l..], part_mb as u32, 1);
    for ch in " MB）".as_bytes().iter() { b[l] = *ch; l += 1; }
    line(g, 184, core::str::from_utf8_unchecked(&b[..l]), rgb(170, 190, 220));
    line(g, 228, "引导方式 : UEFI · FAT16 引导分区（ESP）", rgb(170, 190, 220));
    line(g, 272, "系统文件 : /EFI/BOOT/BOOTX64.EFI", rgb(170, 190, 220));
    g.rect(64, 320, g.w as i32 - 128, 1, rgb(60, 80, 110), 200);
    line(g, 344, "警告：安装会清空目标磁盘的分区表与全部数据。", rgb(255, 190, 120));
    line(g, 392, "是否将 HYWOS 安装到此磁盘？", rgb(235, 245, 255));
    g.rect(64, 436, 260, 48, rgb(20, 60, 120), 220);
    line(g, 444, "  [ y ] 安装    [ n ] 取消  ", rgb(200, 230, 255));
    line(g, 520, "直接按 y 确认安装，按 n 取消。", rgb(140, 160, 190));
    line(g, 680, "HYWOS Installer · 命令行安装模式", rgb(110, 130, 160));
}

unsafe fn screen_msg(g: &Gfx, title: &str, body: &str, color: u32) {
    g.clear(rgb(6, 8, 14));
    let mut x = 64;
    for ch in "HYWOS".bytes() {
        big2(g, x, 60, ch, rgb(150, 210, 255), 255);
        x += 32;
    }
    g.text(64 + 5 * 32 + 16, 76, "1.0.4 安装程序", rgb(220, 236, 255), 2);
    g.rect(64, 132, g.w as i32 - 128, 2, rgb(70, 110, 160), 200);
    g.text(64, 200, title, color, 2);
    let mut y = 260;
    for part in body.split('\n') {
        g.text(64, y, part, rgb(190, 205, 230), 2);
        y += 44;
    }
}

pub unsafe fn get_image() -> Option<(*const u8, usize)> {
    if IMAGE.is_null() {
        return None;
    }
    let mut li: *mut LoadedImage = core::ptr::null_mut();
    if !status_ok(bs_handle_protocol(
        IMAGE,
        &GUID_LOADED_IMAGE,
        &mut li as *mut *mut LoadedImage as *mut *mut c_void,
    )) || li.is_null()
    {
        return None;
    }
    if (*li).image_base.is_null() || (*li).image_size == 0 {
        return None;
    }
    Some(((*li).image_base as *const u8, (*li).image_size))
}

pub unsafe fn run(g: &Gfx) -> ! {
    let target = diskfmt::pick_target();
    let (cap, part) = match &target {
        Some(t) => diskfmt::target_info(t),
        None => (0, 0),
    };
    screen_confirm(g, cap, part);
    let mut chosen = false;
    loop {
        if let Some((_scan, ch)) = read_key() {
            if ch == b'y' as u16 || ch == b'Y' as u16 {
                chosen = true;
                break;
            }
            if ch == b'n' as u16 || ch == b'N' as u16 || ch == 0x1b {
                break;
            }
        }
        bs_stall(30_000);
    }
    if !chosen {
        screen_msg(g, "安装已取消", "未对磁盘做任何修改。\n请关闭虚拟机或按电源键重启。", rgb(255, 200, 120));
        loop {
            bs_stall(1_000_000);
        }
    }
    let t = match target {
        Some(t) => t,
        None => {
            screen_msg(g, "未找到目标磁盘", "请为虚拟机添加一块可写硬盘（>=100MB）。\n安装程序无法继续。", rgb(255, 140, 130));
            loop {
                bs_stall(1_000_000);
            }
        }
    };
    screen_msg(g, "正在安装 HYWOS…", "写入分区表与 FAT16 引导分区\n复制 BOOTX64.EFI 到 ESP", rgb(150, 210, 255));
    let ok = match fs::open_sys("\\EFI\\BOOT\\BOOTX64.EFI") {
        Some(f) => {
            let sz = fs::file_size(f);
            let r = match sz {
                Some(n) => diskfmt::install_file(&t, f, n),
                None => false,
            };
            fs::close_f(f);
            r
        }
        None => false,
    };
    if ok {
        let (cap, part) = diskfmt::target_info(&t);
        let mut b = [0u8; 96];
        let mut l = 0;
        for ch in "已写入 ".as_bytes().iter() { b[l] = *ch; l += 1; }
        l += crate::hw::num2(&mut b[l..], part as u32, 1);
        for ch in " MB FAT16 引导分区\n系统文件 /EFI/BOOT/BOOTX64.EFI".as_bytes().iter() { b[l] = *ch; l += 1; }
        let _ = cap;
        screen_msg(
            g,
            "安装完成",
            core::str::from_utf8_unchecked(&b[..l]),
            rgb(120, 235, 150),
        );
        let mut y = 380;
        g.text(64, y, "请关闭虚拟机，移除 ISO，从硬盘启动 HYWOS。", rgb(220, 235, 250), 2);
        y += 44;
        g.text(64, y, "UEFI 会自动识别 ESP 中的 BOOTX64.EFI。", rgb(170, 190, 220), 2);
    } else {
        screen_msg(g, "安装失败", "无法写入目标磁盘。\n请确认磁盘可写且容量足够。", rgb(255, 120, 110));
    }
    let _ = fs::is_install_media();
    loop {
        bs_stall(1_000_000);
    }
}