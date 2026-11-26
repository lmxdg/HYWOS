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

fn empty_file() -> diskfmt::SystemFile<'static> {
    diskfmt::SystemFile {
        dir: diskfmt::DIR_ROOT,
        name: [0; 11],
        size: 0,
        src: core::ptr::null_mut(),
        data: b"",
    }
}

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
    g.text(64 + title.len() as i32 * 32 + 16, 60, "1.1.2 安装程序", rgb(220, 236, 255), 2);
    g.rect(64, 116, g.w as i32 - 128, 2, rgb(70, 110, 160), 200);
    line(g, 140, "安装介质 : HYWOS 1.1.2 ISO（UEFI 启动）", rgb(170, 190, 220));
    let mut b = [0u8; 96];
    let mut l = 0;
    for ch in "目标磁盘 : ".as_bytes().iter() { b[l] = *ch; l += 1; }
    l += crate::hw::num2(&mut b[l..], cap_mb as u32, 1);
    for ch in " MB（可安装分区 ".as_bytes().iter() { b[l] = *ch; l += 1; }
    l += crate::hw::num2(&mut b[l..], part_mb as u32, 1);
    for ch in " MB）".as_bytes().iter() { b[l] = *ch; l += 1; }
    line(g, 184, core::str::from_utf8_unchecked(&b[..l]), rgb(170, 190, 220));
    line(g, 228, "系统文件 : 内核 + 系统资源全部复制到本地磁盘", rgb(170, 190, 220));
    line(g, 272, "安装顺序 : 先复制系统文件，再创建引导分区", rgb(170, 190, 220));
    line(g, 316, "引导方式 : UEFI · FAT16 引导分区（ESP）", rgb(170, 190, 220));
    g.rect(64, 360, g.w as i32 - 128, 1, rgb(60, 80, 110), 200);
    line(g, 380, "警告：安装会清空目标磁盘的分区表与全部数据。", rgb(255, 190, 120));
    line(g, 424, "是否将 HYWOS 安装到此磁盘？", rgb(235, 245, 255));
    g.rect(64, 468, 300, 48, rgb(20, 60, 120), 220);
    line(g, 476, "  [ Enter ] 安装    [ N ] 取消  ", rgb(200, 230, 255));
    line(g, 548, "ISO 不会直接进入桌面：确认后才复制系统并创建引导。", rgb(140, 160, 190));
    line(g, 640, "FAT16 引导分区上限 128 MB，系统文件不足 2 MB。", rgb(140, 160, 190));
    line(g, 680, "HYWOS Installer · 命令行安装模式", rgb(110, 130, 160));
}

unsafe fn screen_msg(g: &Gfx, title: &str, body: &str, color: u32) {
    g.clear(rgb(6, 8, 14));
    let mut x = 64;
    for ch in "HYWOS".bytes() {
        big2(g, x, 60, ch, rgb(150, 210, 255), 255);
        x += 32;
    }
    g.text(64 + 5 * 32 + 16, 76, "1.1.2 安装程序", rgb(220, 236, 255), 2);
    g.rect(64, 132, g.w as i32 - 128, 2, rgb(70, 110, 160), 200);
    g.text(64, 200, title, color, 2);
    let mut y = 260;
    for part in body.split('\n') {
        g.text(64, y, part, rgb(190, 205, 230), 2);
        y += 44;
    }
}

static mut PROG_G: *const Gfx = core::ptr::null();

unsafe fn draw_progress(p: u32) {
    if PROG_G.is_null() {
        return;
    }
    let g = &*PROG_G;
    g.clear(rgb(6, 8, 14));
    let mut x = 64;
    for ch in "HYWOS".bytes() {
        big2(g, x, 60, ch, rgb(150, 210, 255), 255);
        x += 32;
    }
    g.text(64 + 5 * 32 + 16, 76, "1.1.2 安装程序", rgb(220, 236, 255), 2);
    g.rect(64, 132, g.w as i32 - 128, 2, rgb(70, 110, 160), 200);
    g.text(64, 200, "正在安装 HYWOS…", rgb(150, 210, 255), 2);
    let bx = 64;
    let by = 290;
    let bw = g.w as i32 - 128;
    let bh = 34;
    g.round_rect(bx, by, bw, bh, 17, rgb(20, 28, 44), 255);
    let fill = (bw - 4) * p as i32 / 100;
    if fill > 4 {
        g.round_rect(bx + 2, by + 2, fill, bh - 4, 15, rgb(64, 140, 255), 255);
        g.round_rect(bx + 2, by + 2, fill, (bh - 4) / 2, 15, rgb(132, 182, 255), 70);
    }
    let mut t = [0u8; 8];
    let mut l = 0;
    l += crate::hw::num2(&mut t[l..], p, 1);
    t[l] = b'%';
    l += 1;
    g.text(bx + bw - 84, by + 9, core::str::from_utf8_unchecked(&t[..l]), rgb(224, 238, 255), 2);
    let stage = if p < 8 {
        "准备本地磁盘"
    } else if p < 76 {
        "复制全部系统文件到本地磁盘"
    } else if p < 90 {
        "写入目录与文件分配表"
    } else if p < 95 {
        "创建 FAT16 引导分区"
    } else if p < 100 {
        "写入引导记录"
    } else {
        "完成"
    };
    g.text(64, by + 62, stage, rgb(170, 190, 220), 2);
    g.text(64, 640, "请勿关闭电源或移除磁盘", rgb(140, 160, 190), 2);
}

static mut PROG_LAST: u32 = 0xFFFF_FFFF;

unsafe fn prog_cb(p: u32) {
    if p == PROG_LAST {
        return;
    }
    PROG_LAST = p;
    draw_progress(p);
    if p < 100 {
        bs_stall(120_000);
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

pub unsafe fn device_handle() -> Handle {
    if IMAGE.is_null() {
        return core::ptr::null_mut();
    }
    let mut li: *mut LoadedImage = core::ptr::null_mut();
    if !status_ok(bs_handle_protocol(
        IMAGE,
        &GUID_LOADED_IMAGE,
        &mut li as *mut *mut LoadedImage as *mut *mut c_void,
    )) || li.is_null()
    {
        return core::ptr::null_mut();
    }
    (*li).device_handle
}

unsafe fn show_diag(g: &Gfx) {
    let diag = diskfmt::diagnostics();
    let mut y = 400;
    g.text(64, y, "检测到的块设备：", rgb(180, 195, 220), 2);
    y += 44;
    let mut i = 0;
    while i < diag.len() && i < 5 {
        let d = &diag[i];
        let mut b = [0u8; 200];
        let mut l = 0;
        for c in "设备 ".as_bytes() { b[l] = *c; l += 1; }
        l += crate::hw::num2(&mut b[l..], (i + 1) as u32, 1);
        for c in "：".as_bytes() { b[l] = *c; l += 1; }
        l += crate::hw::num2(&mut b[l..], d.mb as u32, 1);
        for c in " MB · 块 ".as_bytes() { b[l] = *c; l += 1; }
        l += crate::hw::num2(&mut b[l..], d.bs, 1);
        for c in " · 只读 ".as_bytes() { b[l] = *c; l += 1; }
        l += crate::hw::num2(&mut b[l..], d.ro as u32, 1);
        for c in " · 分区 ".as_bytes() { b[l] = *c; l += 1; }
        l += crate::hw::num2(&mut b[l..], d.logical as u32, 1);
        for c in " · 就绪 ".as_bytes() { b[l] = *c; l += 1; }
        l += crate::hw::num2(&mut b[l..], d.present as u32, 1);
        g.text(64, y, core::str::from_utf8_unchecked(&b[..l]), rgb(160, 180, 210), 2);
        y += 44;
        i += 1;
    }
}

pub unsafe fn run(g: &Gfx) -> ! {
    let mut probe = diskfmt::pick_target();
    let mut tries = 0;
    while probe.is_none() && tries < 3 {
        bs_connect_all_storage();
        bs_stall(300_000);
        probe = diskfmt::pick_target();
        tries += 1;
    }
    let t = match probe {
        Some(t) => t,
        None => {
            screen_msg(
                g,
                "未找到可安装的硬盘",
                "请为虚拟机添加一块 ≥20MB 的可写磁盘\n（IDE / SATA / virtio-blk / NVMe 均可）。",
                rgb(255, 140, 130),
            );
            show_diag(g);
            loop {
                bs_stall(1_000_000);
            }
        }
    };
    let (cap, part) = diskfmt::target_info(&t);
    screen_confirm(g, cap, part);
    let mut chosen = false;
    loop {
        if let Some((_scan, ch)) = read_key() {
            if ch == 13 {
                chosen = true;
                break;
            }
            if ch == b'n' as u16 || ch == b'N' as u16 || _scan == 0x17 {
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
    screen_msg(g, "正在安装 HYWOS…", "先复制全部系统文件到本地磁盘\n再创建 FAT16 引导分区与引导记录", rgb(150, 210, 255));
    PROG_G = g;
    draw_progress(0);
    let src_vol = fs::source_vol();
    let kernel = fs::open_vol(src_vol, "\\EFI\\BOOT\\BOOTX64.EFI");
    let welcome = fs::open_vol(src_vol, "\\HYWOS\\SYSTEM\\WELCOME.TXT");
    let mut files: [diskfmt::SystemFile; 4] = [empty_file(), empty_file(), empty_file(), empty_file()];
    let mut n = 0usize;
    if let Some(f) = kernel {
        match fs::file_size(f) {
            Some(sz) => {
                files[n].dir = diskfmt::DIR_BOOT;
                files[n].name = *b"BOOTX64 EFI";
                files[n].size = sz;
                files[n].src = f;
                n += 1;
            }
            None => fs::close_f(f),
        }
    }
    if let Some(f) = welcome {
        match fs::file_size(f) {
            Some(sz) => {
                files[n].dir = diskfmt::DIR_SYSTEM;
                files[n].name = *b"WELCOME TXT";
                files[n].size = sz;
                files[n].src = f;
                n += 1;
            }
            None => fs::close_f(f),
        }
    }
    if n < 1 {
        screen_msg(g, "安装失败", "安装介质中缺少内核文件。\n请使用完整的 HYWOS ISO 重新安装。", rgb(255, 120, 110));
        loop {
            bs_stall(1_000_000);
        }
    }
    files[n].dir = diskfmt::DIR_HYWOS;
    files[n].name = *b"VERSION    ";
    files[n].data = b"HYWOS 1.1.2\n";
    files[n].size = files[n].data.len();
    n += 1;
    files[n].dir = diskfmt::DIR_HYWOS;
    files[n].name = *b"INSTALL TXT";
    files[n].data = b"HYWOS 1.1.2 installed\n";
    files[n].size = files[n].data.len();
    n += 1;
    let ok = diskfmt::install_system(&t, &files[..n], prog_cb);
    if !files[0].src.is_null() { fs::close_f(files[0].src); }
    if !files[1].src.is_null() { fs::close_f(files[1].src); }
    if ok {
        let (_cap, part) = diskfmt::target_info(&t);
        let mut b = [0u8; 200];
        let mut l = 0;
        for ch in "已复制全部系统文件到 ".as_bytes().iter() { b[l] = *ch; l += 1; }
        l += crate::hw::num2(&mut b[l..], part as u32, 1);
        for ch in " MB 本地磁盘\n".as_bytes().iter() { b[l] = *ch; l += 1; }
        for ch in "引导分区 : \\EFI\\BOOT\\BOOTX64.EFI\n".as_bytes().iter() { b[l] = *ch; l += 1; }
        for ch in "系统目录 : \\HYWOS\\SYSTEM 与安装标记".as_bytes().iter() { b[l] = *ch; l += 1; }
        screen_msg(
            g,
            "安装完成",
            core::str::from_utf8_unchecked(&b[..l]),
            rgb(120, 235, 150),
        );
        let mut y = 420;
        g.text(64, y, "请移除 ISO，从本地硬盘启动 HYWOS。", rgb(220, 235, 250), 2);
        y += 44;
        g.text(64, y, "安装标记 \\HYWOS\\INSTALL.TXT 可防止再次进入安装程序。", rgb(170, 190, 220), 2);
    } else {
        screen_msg(g, "安装失败", "无法写入目标磁盘。\n请确认磁盘可写且容量足够。", rgb(255, 120, 110));
    }
    let _ = fs::is_install_media();
    loop {
        bs_stall(1_000_000);
    }
}