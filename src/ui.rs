use crate::apps::{self, AppAction, AppState};
use crate::efi::*;
use crate::fs;
use crate::gfx::{rgb, text_width, Gfx};
use crate::hw;
use crate::icon::{icon, AppId};

pub const TB_H: i32 = 48;

pub const TB_APPS: [AppId; 6] = [
    AppId::Terminal,
    AppId::Files,
    AppId::Browser,
    AppId::Settings,
    AppId::Store,
    AppId::Photos,
];

pub const DESK_ICONS: [AppId; 4] = [
    AppId::Files,
    AppId::Terminal,
    AppId::Browser,
    AppId::Security,
];

pub const DOCK_APPS: [AppId; 19] = [
    AppId::Terminal,
    AppId::Files,
    AppId::Settings,
    AppId::Store,
    AppId::About,
    AppId::Browser,
    AppId::Mail,
    AppId::Photos,
    AppId::Music,
    AppId::Calendar,
    AppId::Calculator,
    AppId::Notes,
    AppId::Maps,
    AppId::Clock,
    AppId::Camera,
    AppId::Videos,
    AppId::Contacts,
    AppId::Messages,
    AppId::Security,
];

pub struct Win {
    pub id: AppId,
    pub open: bool,
    pub min: bool,
    pub max: bool,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub rx: i32,
    pub ry: i32,
    pub rw: i32,
    pub rh: i32,
    pub app: AppState,
}

static mut WINS: [Win; 3] = unsafe { core::mem::zeroed() };
pub const UI_MAX_PIX: usize = 1920 * 1080;
static mut BG: [u32; UI_MAX_PIX] = [0; UI_MAX_PIX];
static mut SNAP: [u32; UI_MAX_PIX] = [0; UI_MAX_PIX];
static mut BACK: [u32; UI_MAX_PIX] = [0; UI_MAX_PIX];
static mut BG_OK: bool = false;
static mut FOCUS: usize = 0;
static mut SHOT_MSG: [u8; 64] = [0; 64];
static mut SHOT_MSG_LEN: usize = 0;

pub struct Ui {
    pub front: Gfx,
    pub g: Gfx,
    pub input: hw::InputState,
    pub launchpad: bool,
    pub lp_t: i32,
    pub search: [u8; 24],
    pub search_len: usize,
    pub frame: u32,
    pub hover: i32,
    pub shot_flash: i32,
    pub drag: i32,
    pub drag_dx: i32,
    pub drag_dy: i32,
    pub menu: bool,
    pub menu_x: i32,
    pub menu_y: i32,
    pub desk_sel: i32,
    pub last_click_frame: u32,
    pub last_click_idx: i32,
    pub all_min: bool,
}

impl Ui {
    pub unsafe fn new(front: Gfx) -> Ui {
        let cx = front.w as i32 / 2;
        let cy = front.h as i32 / 2;
        let n = front.w * front.h;
        let back_ptr = core::ptr::addr_of_mut!(BACK) as *mut u32;
        let g = if n <= UI_MAX_PIX {
            Gfx { fb: back_ptr, w: front.w, h: front.h, stride: front.w, swap: false, ok: front.ok }
        } else {
            Gfx { fb: front.fb, w: front.w, h: front.h, stride: front.stride, swap: front.swap, ok: front.ok }
        };
        let mut ui = Ui {
            front,
            g,
            input: hw::InputState::new(cx, cy),
            launchpad: false,
            lp_t: 0,
            search: [0; 24],
            search_len: 0,
            frame: 0,
            hover: -1,
            shot_flash: 0,
            drag: -1,
            drag_dx: 0,
            drag_dy: 0,
            menu: false,
            menu_x: 0,
            menu_y: 0,
            desk_sel: -1,
            last_click_frame: 0,
            last_click_idx: -1,
            all_min: false,
        };
        ui.wallpaper();
        ui.snapshot_bg();
        ui.present();
        ui
    }

    fn present(&self) {
        unsafe {
            let n = self.g.w * self.g.h;
            if n == 0 { return; }
            if self.g.fb == self.front.fb {
                self.front.flush();
                return;
            }
            let src = core::slice::from_raw_parts(self.g.fb, n);
            let swap = self.front.swap;
            let fw = self.front.w;
            let fstride = self.front.stride;
            let fh = self.front.h;
            let dst = core::slice::from_raw_parts_mut(self.front.fb, fstride * fh);
            if fstride == fw {
                let mut i = 0;
                while i < n {
                    let c = src[i];
                    dst[i] = if swap {
                        ((c & 0xff) << 16) | (c & 0x00ff00) | ((c >> 16) & 0xff) | 0xff00_0000
                    } else {
                        c | 0xff00_0000
                    };
                    i += 1;
                }
            } else {
                let mut y = 0;
                while y < fh {
                    let mut x = 0;
                    while x < fw {
                        let c = src[y * fw + x];
                        dst[y * fstride + x] = if swap {
                            ((c & 0xff) << 16) | (c & 0x00ff00) | ((c >> 16) & 0xff) | 0xff00_0000
                        } else {
                            c | 0xff00_0000
                        };
                        x += 1;
                    }
                    y += 1;
                }
            }
        }
        self.front.flush();
    }

    fn w(&self) -> i32 { self.g.w as i32 }
    fn h(&self) -> i32 { self.g.h as i32 }

    fn wallpaper(&self) {
        let g = &self.g;
        let w = self.w();
        let h = self.h();
        g.vgrad(0, 0, w, h, rgb(10, 15, 28), rgb(4, 6, 12));
        let cx = w * 3 / 4;
        let cy = h * 2 / 5;
        g.ring(cx, cy, 250, 1, rgb(74, 140, 214), 44);
        g.ring(cx, cy, 360, 1, rgb(58, 108, 170), 26);
        g.ring(cx, cy, 470, 1, rgb(46, 84, 136), 16);
        g.circle(w / 6, h * 4 / 5, 250, rgb(38, 88, 168), 12);
        g.circle(w / 6, h * 4 / 5, 170, rgb(52, 114, 206), 10);
        g.circle(w * 5 / 6, h * 4 / 5, 210, rgb(26, 64, 124), 10);
        let tw = text_width("HYWOS", 6);
        g.text_a(w / 2 - tw / 2, h / 2 - 44, "HYWOS", rgb(118, 168, 238), 6, 14);
        let sw = text_width("自研 64 位 UEFI 系统", 2);
        g.text_a(w / 2 - sw / 2, h / 2 + 26, "自研 64 位 UEFI 系统", rgb(96, 132, 190), 2, 34);
        let mut seed = 0x2545f491u32;
        let mut i = 0;
        while i < 60 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = (seed % w as u32) as i32;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let y = (seed % h as u32) as i32;
            let b = ((seed >> 8) % 50) as u8 + 70;
            g.put(x, y, rgb(b, b, (b as u32 + 50).min(255) as u8), 90);
            i += 1;
        }
    }

    unsafe fn snapshot_bg(&mut self) {
        let n = self.g.w * self.g.h;
        let bg = &mut *core::ptr::addr_of_mut!(BG);
        BG_OK = n <= bg.len();
        if BG_OK {
            self.g.snapshot(&mut bg[..n]);
        }
    }

    fn restore_bg(&self) {
        unsafe {
            if !BG_OK {
                self.wallpaper();
                return;
            }
            let w = self.g.w;
            let h = self.g.h;
            let stride = self.g.stride;
            let swap = self.g.swap;
            let bg = &*core::ptr::addr_of!(BG);
            let dst = core::slice::from_raw_parts_mut(self.g.fb, stride * h);
            if stride == w {
                let mut i = 0;
                while i < w * h {
                    dst[i] = if swap {
                        ((bg[i] & 0xff) << 16) | (bg[i] & 0x00ff00) | ((bg[i] >> 16) & 0xff)
                    } else {
                        bg[i]
                    };
                    i += 1;
                }
            } else {
                let mut yy = 0;
                while yy < h {
                    let mut xx = 0;
                    while xx < w {
                        let c = bg[yy * w + xx];
                        dst[yy * stride + xx] = if swap {
                            ((c & 0xff) << 16) | (c & 0x00ff00) | ((c >> 16) & 0xff)
                        } else {
                            c
                        };
                        xx += 1;
                    }
                    yy += 1;
                }
            }
        }
    }

    fn run_slots(&self) -> ([i32; 3], usize) {
        let wins = unsafe { &*core::ptr::addr_of!(WINS) };
        let mut out = [-1i32; 3];
        let mut n = 0usize;
        let mut i = 0;
        while i < 3 {
            if wins[i].open {
                let mut fixed = false;
                let mut k = 0;
                while k < TB_APPS.len() {
                    if TB_APPS[k] == wins[i].id { fixed = true; }
                    k += 1;
                }
                if !fixed {
                    out[n] = i as i32;
                    n += 1;
                }
            }
            i += 1;
        }
        (out, n)
    }

    fn tb_group(&self) -> (i32, i32, i32) {
        let (_, rn) = self.run_slots();
        let n = 1 + TB_APPS.len() as i32 + rn as i32;
        let iw = 40i32;
        let gap = 4i32;
        let gw = n * iw + (n - 1) * gap;
        let x0 = (self.w() - gw) / 2;
        (x0, iw, gap)
    }

    fn tb_slot_at(&self, mx: i32, my: i32) -> i32 {
        let h = self.h();
        if my < h - TB_H { return -2; }
        if mx >= self.w() - 8 { return -3; }
        let (x0, iw, gap) = self.tb_group();
        if mx < x0 { return -1; }
        let rel = mx - x0;
        let slot = rel / (iw + gap);
        if rel - slot * (iw + gap) >= iw { return -1; }
        let (_, rn) = self.run_slots();
        let total = 1 + TB_APPS.len() as i32 + rn as i32;
        if slot >= 0 && slot < total { slot } else { -1 }
    }

    fn draw_start_mark(&self, cx: i32, cy: i32, sz: i32) {
        let g = &self.g;
        let u = sz / 2;
        let c = rgb(96, 186, 255);
        g.rect(cx - u, cy - u, u - 1, u - 1, c, 255);
        g.rect(cx + 1, cy - u, u - 1, u - 1, c, 255);
        g.rect(cx - u, cy + 1, u - 1, u - 1, c, 255);
        g.rect(cx + 1, cy + 1, u - 1, u - 1, c, 255);
    }

    unsafe fn draw_taskbar(&self) {
        let g = &self.g;
        let w = self.w();
        let h = self.h();
        let y0 = h - TB_H;
        g.rect(0, y0, w, TB_H, rgb(28, 28, 32), 246);
        g.rect(0, y0, w, 1, rgb(255, 255, 255), 34);
        let (x0, iw, gap) = self.tb_group();
        let cy = y0 + TB_H / 2;
        let (runs, rn) = self.run_slots();
        let wins = &*core::ptr::addr_of!(WINS);
        let mut i = 0;
        while i < 1 + TB_APPS.len() as i32 + rn as i32 {
            let x = x0 + i * (iw + gap);
            let hovered = self.hover == i;
            if hovered { g.round_rect(x, y0 + 4, iw, TB_H - 8, 6, rgb(255, 255, 255), 28); }
            let ix = x + (iw - 32) / 2;
            let iy = cy - 16;
            let mut running = false;
            if i == 0 {
                self.draw_start_mark(x + iw / 2, cy, 18);
            } else if i <= TB_APPS.len() as i32 {
                let id = TB_APPS[(i - 1) as usize];
                g.blit_rgba(ix, iy, 32, 32, icon(id), 64, 64);
                let mut k = 0;
                while k < 3 {
                    if wins[k].open && wins[k].id == id { running = true; }
                    k += 1;
                }
            } else {
                let idx = runs[(i - 1 - TB_APPS.len() as i32) as usize];
                if idx >= 0 {
                    let wv = &wins[idx as usize];
                    g.blit_rgba(ix, iy, 32, 32, icon(wv.id), 64, 64);
                    running = true;
                }
            }
            if i > 0 && running {
                let focused = FOCUS == (if i <= TB_APPS.len() as i32 {
                    let id = TB_APPS[(i - 1) as usize];
                    let mut f = 99usize;
                    let mut k = 0;
                    while k < 3 { if wins[k].open && wins[k].id == id { f = k; } k += 1; }
                    f
                } else {
                    runs[(i - 1 - TB_APPS.len() as i32) as usize] as usize
                });
                g.round_rect(x + iw / 2 - 7, y0 + TB_H - 5, 14, 3, 0, rgb(96, 186, 255), if focused { 255 } else { 140 });
            }
            i += 1;
        }

        let net = hw::net();
        let ncol = if net.present { rgb(120, 220, 180) } else { rgb(150, 130, 140) };
        g.ring(w - 190, cy, 8, 2, ncol, 220);
        g.line(w - 184, cy - 4, w - 176, cy - 4, ncol, 220);
        g.line(w - 184, cy, w - 177, cy, ncol, 220);
        g.line(w - 184, cy + 4, w - 178, cy + 4, ncol, 220);

        g.round_rect(w - 166, cy - 6, 6, 12, 1, rgb(210, 220, 236), 220);
        g.tri(w - 159, cy - 4, w - 159, cy + 4, w - 153, cy + 7, rgb(210, 220, 236), 220);
        g.tri(w - 152, cy + 7, w - 146, cy + 7, w - 149, cy + 1, rgb(210, 220, 236), 220);

        let mut tb = [0u8; 8];
        let mut d2 = [0u8; 8];
        let mut tl = 0usize;
        let mut dl = 0usize;
        match hw::cached_time() {
            Some(t) if t.hour <= 23 && t.minute <= 59 => {
                tl += hw::num2(&mut tb[tl..], t.hour as u32, 2);
                tb[tl] = b':'; tl += 1;
                tl += hw::num2(&mut tb[tl..], t.minute as u32, 2);
                if t.month >= 1 && t.month <= 12 && t.day >= 1 && t.day <= 31 {
                    dl += hw::num2(&mut d2[dl..], t.month as u32, 2);
                    d2[dl] = b'-'; dl += 1;
                    dl += hw::num2(&mut d2[dl..], t.day as u32, 2);
                }
            }
            _ => {
                for ch in b"--:--".iter() { tb[tl] = *ch; tl += 1; }
            }
        }
        g.text(w - 132, y0 + 9, core::str::from_utf8_unchecked(&tb[..tl]), rgb(236, 244, 255), 1);
        if dl > 0 {
            g.text(w - 132, y0 + 26, core::str::from_utf8_unchecked(&d2[..dl]), rgb(170, 182, 200), 1);
        }
        g.round_rect(w - 64, cy - 5, 18, 10, 3, rgb(150, 160, 180), 200);
        g.rect(w - 45, cy - 2, 2, 4, rgb(150, 160, 180), 200);
        g.rect(w - 62, cy - 4, 12, 8, rgb(110, 220, 150), 240);
        g.rect(w - 8, y0 + 6, 1, TB_H - 12, rgb(255, 255, 255), 26);

        let msg = &*core::ptr::addr_of!(SHOT_MSG);
        let ml = SHOT_MSG_LEN;
        if ml > 0 {
            g.text(w - 330, y0 + 17, core::str::from_utf8_unchecked(&msg[..ml]), rgb(140, 230, 170), 1);
        }
        if self.hover >= 0 && self.hover <= TB_APPS.len() as i32 {
            let label = if self.hover == 0 { "开始" } else { TB_APPS[(self.hover - 1) as usize].name() };
            let lw = text_width(label, 1) + 22;
            let lx = x0 + self.hover * (iw + gap) + iw / 2 - lw / 2;
            g.round_rect(lx, y0 - 34, lw, 24, 6, rgb(48, 48, 54), 240);
            g.text_center(lx + lw / 2, y0 - 29, label, rgb(238, 244, 252), 1);
        }
    }

    unsafe fn draw_window(&self, i: usize, focus: bool) {
        let wins = &mut *core::ptr::addr_of_mut!(WINS);
        if !wins[i].open || wins[i].min { return; }
        apps::draw(&mut wins[i].app, wins[i].id, &self.g, wins[i].x, wins[i].y, wins[i].w, wins[i].h, focus);
    }

    fn draw_cursor(&self) {
        let g = &self.g;
        let x = self.input.x;
        let y = self.input.y;
        if self.input.down {
            g.circle(x, y, 9, rgb(255, 255, 255), 60);
        }
        let pts = [(0i32, 0i32), (0, 18), (5, 13), (9, 20), (12, 18), (8, 11), (14, 11)];
        let mut i = 0;
        while i < pts.len() {
            let j = if i + 1 < pts.len() { i + 1 } else { 0 };
            g.tri(x + pts[i].0 + 1, y + pts[i].1 + 1, x + pts[j].0 + 1, y + pts[j].1 + 1, x + 6, y + 9, 0x000000, 90);
            i += 1;
        }
        i = 0;
        while i < pts.len() {
            let j = if i + 1 < pts.len() { i + 1 } else { 0 };
            g.tri(x + pts[i].0, y + pts[i].1, x + pts[j].0, y + pts[j].1, x + 6, y + 9, rgb(250, 252, 255), 245);
            i += 1;
        }
    }

    fn filtered(&self) -> ([AppId; 20], usize) {
        let mut out = [AppId::Terminal; 20];
        let mut n = 0;
        let mut i = 0;
        while i < 20 {
            let a = AppId::from_index(i);
            if self.matches(a) { out[n] = a; n += 1; }
            i += 1;
        }
        (out, n)
    }

    fn matches(&self, a: AppId) -> bool {
        if self.search_len == 0 { return true; }
        let q = &self.search[..self.search_len];
        let mut name = [0u8; 32];
        let mut nl = 0;
        for c in a.name().bytes() {
            name[nl] = if c >= b'A' && c <= b'Z' { c + 32 } else { c };
            nl += 1;
        }
        let mut id = [0u8; 24];
        let mut il = 0;
        for c in a.id_str().bytes() {
            id[il] = if c >= b'A' && c <= b'Z' { c + 32 } else { c };
            il += 1;
        }
        if self.contains(&name[..nl], q) { return true; }
        self.contains(&id[..il], q)
    }

    fn contains(&self, hay: &[u8], needle: &[u8]) -> bool {
        if needle.len() > hay.len() { return false; }
        let mut i = 0;
        while i + needle.len() <= hay.len() {
            let mut k = 0;
            while k < needle.len() && hay[i + k] == needle[k] { k += 1; }
            if k == needle.len() { return true; }
            i += 1;
        }
        false
    }

    fn start_geom(&self) -> (i32, i32, i32, i32) {
        let pw = 600i32;
        let ph = 560i32;
        let px = (self.w() - pw) / 2;
        let py = self.h() - TB_H - ph - 8;
        (px, py, pw, ph)
    }

    unsafe fn draw_start(&self) {
        let g = &self.g;
        let (px, py, pw, ph) = self.start_geom();
        g.round_rect(px - 6, py - 6, pw + 12, ph + 12, 16, 0x000000, 60);
        g.round_rect(px, py, pw, ph, 12, rgb(40, 40, 45), 250);
        g.round_rect_outline(px, py, pw, ph, 12, rgb(255, 255, 255), 36);
        g.round_rect(px + 40, py + 22, pw - 80, 34, 17, rgb(52, 52, 58), 255);
        g.ring(px + 62, py + 39, 7, 2, rgb(180, 190, 210), 220);
        g.line(px + 68, py + 45, px + 74, py + 51, rgb(180, 190, 210), 220);
        if self.search_len > 0 {
            let q = core::str::from_utf8_unchecked(&self.search[..self.search_len]);
            g.text(px + 84, py + 32, q, rgb(238, 244, 252), 1);
            if (self.frame / 18) % 2 == 0 {
                g.rect(px + 84 + text_width(q, 1), py + 32, 6, 15, rgb(120, 190, 255), 220);
            }
        } else {
            g.text(px + 84, py + 32, "搜索应用", rgb(160, 170, 188), 1);
        }
        g.text(px + 40, py + 70, "固定", rgb(196, 204, 218), 1);
        let (apps, n) = self.filtered();
        let cols = 6i32;
        let cw = 96i32;
        let chh = 92i32;
        let gx = px + 30;
        let gy = py + 96;
        let mut i = 0;
        while i < n {
            let col = (i as i32 % cols) as i32;
            let row = (i as i32 / cols) as i32;
            let cx = gx + col * cw;
            let cy = gy + row * chh;
            if self.hover == 100 + i as i32 {
                g.round_rect(cx - 2, cy - 6, cw - 6, chh - 8, 8, rgb(255, 255, 255), 26);
            }
            g.blit_rgba(cx + 22, cy, 52, 52, icon(apps[i]), 64, 64);
            let nw = text_width(apps[i].name(), 1);
            g.text_a(cx + 48 - nw / 2, cy + 58, apps[i].name(), rgb(232, 238, 248), 1, 235);
            i += 1;
        }
        if n == 0 {
            g.text_center(px + pw / 2, py + 210, "没有匹配的应用", rgb(200, 210, 228), 1);
        }
        let by = py + ph - 48;
        g.circle(px + 46, by + 18, 15, rgb(84, 148, 226), 255);
        g.text(px + 44, by + 12, "U", rgb(255, 255, 255), 1);
        g.text(px + 70, by + 10, "user", rgb(236, 242, 252), 1);
        g.text(px + 70, by + 25, "HYWOS 桌面", rgb(160, 170, 188), 1);
        let sx = px + pw - 92;
        if self.hover == 200 { g.round_rect(sx - 6, by + 4, 34, 28, 6, rgb(255, 255, 255), 26); }
        g.ring(sx + 11, by + 18, 8, 2, rgb(214, 222, 236), 230);
        g.circle(sx + 11, by + 18, 3, rgb(214, 222, 236), 230);
        let px2 = px + pw - 46;
        if self.hover == 201 { g.round_rect(px2 - 6, by + 4, 34, 28, 6, rgb(255, 255, 255), 26); }
        g.round_rect(px2 + 2, by + 10, 16, 14, 2, rgb(214, 222, 236), 230);
        g.rect(px2 + 8, by + 7, 4, 3, rgb(214, 222, 236), 230);
        g.text_center(px + pw / 2, py + ph - 4, "HYWOS 1.1.5", rgb(120, 130, 148), 1);
    }

    unsafe fn draw_desk_icons(&self) {
        let g = &self.g;
        let mut i = 0;
        while i < DESK_ICONS.len() {
            let x = 22;
            let y = 18 + i as i32 * 96;
            if self.desk_sel == i as i32 {
                g.round_rect(x - 8, y - 8, 64, 92, 6, rgb(96, 156, 226), 60);
                g.round_rect_outline(x - 8, y - 8, 64, 92, 6, rgb(140, 190, 246), 140);
            }
            g.blit_rgba(x, y, 48, 48, icon(DESK_ICONS[i]), 64, 64);
            let nm = DESK_ICONS[i].name();
            let nw = text_width(nm, 1);
            let lx = x + 24 - nw / 2;
            g.round_rect(lx - 3, y + 52, nw + 6, 18, 4, 0x000000, 90);
            g.text(lx, y + 55, nm, rgb(240, 246, 255), 1);
            i += 1;
        }
    }

    fn desk_icon_at(&self, mx: i32, my: i32) -> Option<usize> {
        let mut i = 0usize;
        while i < DESK_ICONS.len() {
            let x = 22 - 8;
            let y = 18 + i as i32 * 96 - 8;
            if mx >= x && mx < x + 64 && my >= y && my < y + 92 { return Some(i); }
            i += 1;
        }
        None
    }

    unsafe fn click_taskbar(&mut self, action: &mut AppAction) {
        let slot = self.tb_slot_at(self.input.x, self.input.y);
        if slot == -3 {
            let wins = &mut *core::ptr::addr_of_mut!(WINS);
            let (runs, rn) = self.run_slots();
            let mut any = false;
            let mut k = 0;
            while k < rn {
                let i = runs[k] as usize;
                if wins[i].open && !wins[i].min { any = true; }
                k += 1;
            }
            let mut k2 = 0;
            while k2 < rn {
                let i = runs[k2] as usize;
                wins[i].min = any;
                k2 += 1;
            }
            self.all_min = any;
            if any {
                let mut last = usize::MAX;
                let mut k3 = 0;
                while k3 < 3 {
                    if wins[k3].open && !wins[k3].min { last = k3; }
                    k3 += 1;
                }
                FOCUS = if last != usize::MAX { last } else { 0 };
            }
        } else if slot == 0 {
            self.launchpad = !self.launchpad;
            self.lp_t = 0;
            self.search_len = 0;
            self.menu = false;
        } else if slot >= 1 && slot <= TB_APPS.len() as i32 {
            self.open_app(TB_APPS[(slot - 1) as usize]);
        } else if slot >= 7 {
            let (runs, rn) = self.run_slots();
            let si = (slot - 7) as usize;
            if si < rn {
                let idx = runs[si] as usize;
                let wins = &mut *core::ptr::addr_of_mut!(WINS);
                if wins[idx].open {
                    if !wins[idx].min && FOCUS == idx {
                        wins[idx].min = true;
                        let mut last = usize::MAX;
                        let mut k = 0;
                        while k < 3 {
                            if wins[k].open && !wins[k].min { last = k; }
                            k += 1;
                        }
                        FOCUS = if last != usize::MAX { last } else { 0 };
                    } else {
                        wins[idx].min = false;
                        FOCUS = idx;
                    }
                }
            }
        }
        let _ = action;
    }

    unsafe fn open_app(&mut self, id: AppId) {
        let wins = &mut *core::ptr::addr_of_mut!(WINS);
        let mut found = usize::MAX;
        let mut i = 0;
        while i < 3 {
            if wins[i].open && wins[i].id == id { found = i; }
            i += 1;
        }
        if found == usize::MAX {
            let mut slot = usize::MAX;
            let mut k = 0;
            while k < 3 {
                if !wins[k].open { slot = k; break; }
                k += 1;
            }
            if slot == usize::MAX { slot = FOCUS; }
            let (mut w, mut h) = apps::app_size(id);
            let max_h = self.h() - TB_H - 70;
            if h > max_h { h = max_h; }
            if w > self.w() - 40 { w = self.w() - 40; }
            wins[slot].id = id;
            wins[slot].open = true;
            wins[slot].w = w;
            wins[slot].h = h;
            wins[slot].x = (self.w() - w) / 2;
            wins[slot].y = 40 + slot as i32 * 18;
            apps::init_state(&mut wins[slot].app, id);
            FOCUS = slot;
        } else {
            FOCUS = found;
            wins[found].min = false;
        }
        self.launchpad = false;
        self.lp_t = 0;
        self.search_len = 0;
    }

    unsafe fn toggle_max(&self, idx: usize) {
        let wins = &mut *core::ptr::addr_of_mut!(WINS);
        if wins[idx].max {
            wins[idx].x = wins[idx].rx;
            wins[idx].y = wins[idx].ry;
            wins[idx].w = wins[idx].rw;
            wins[idx].h = wins[idx].rh;
            wins[idx].max = false;
        } else {
            wins[idx].rx = wins[idx].x;
            wins[idx].ry = wins[idx].y;
            wins[idx].rw = wins[idx].w;
            wins[idx].rh = wins[idx].h;
            wins[idx].x = 0;
            wins[idx].y = 0;
            wins[idx].w = self.w();
            wins[idx].h = self.h() - TB_H;
            wins[idx].max = true;
        }
    }

    unsafe fn close_window(&mut self, idx: usize) {
        let wins = &mut *core::ptr::addr_of_mut!(WINS);
        wins[idx].open = false;
        let mut last = 0usize;
        let mut any = false;
        let mut i = 0;
        while i < 3 {
            if wins[i].open { last = i; any = true; }
            i += 1;
        }
        FOCUS = if any { last } else { 0 };
    }

    unsafe fn hit_window(&self, mx: i32, my: i32) -> Option<usize> {
        let wins = &*core::ptr::addr_of!(WINS);
        if wins[FOCUS].open && mx >= wins[FOCUS].x && mx < wins[FOCUS].x + wins[FOCUS].w && my >= wins[FOCUS].y && my < wins[FOCUS].y + wins[FOCUS].h {
            return Some(FOCUS);
        }
        let mut i = 0;
        while i < 3 {
            let w = &wins[i];
            if w.open && mx >= w.x && mx < w.x + w.w && my >= w.y && my < w.y + w.h {
                return Some(i);
            }
            i += 1;
        }
        None
    }

    unsafe fn take_screenshot(&mut self) {
        if self.g.w * self.g.h > UI_MAX_PIX { return; }
        let n = self.g.w * self.g.h;
        let buf = core::slice::from_raw_parts(self.g.fb, self.g.stride * self.g.h);
        let snap = &mut *core::ptr::addr_of_mut!(SNAP);
        let mut y = 0;
        while y < self.g.h {
            let mut x = 0;
            while x < self.g.w {
                let c = buf[y * self.g.stride + x];
                snap[y * self.g.w + x] = if self.g.swap {
                    ((c & 0xff) << 16) | (c & 0x00ff00) | ((c >> 16) & 0xff)
                } else {
                    c
                };
                x += 1;
            }
            y += 1;
        }
        if let Some(num) = fs::screenshot(&snap[..n], self.g.w, self.g.h) {
            let msg = b"Saved: shot-0000.bmp";
            let out = &mut *core::ptr::addr_of_mut!(SHOT_MSG);
            let mut i = 0;
            for c in msg.iter() { out[i] = *c; i += 1; }
            out[24] = b'0' + (num % 10) as u8;
            SHOT_MSG_LEN = i;
        }
    }

    pub unsafe fn tick(&mut self) -> bool {
        self.frame += 1;
        apps::TICK = self.frame;
        let w = self.w();
        let h = self.h();
        self.input.poll(w, h);
        let mut action = AppAction::None;
        if self.drag >= 0 {
            if self.input.down {
                let wins = &mut *core::ptr::addr_of_mut!(WINS);
                let idx = self.drag as usize;
                if idx < 3 && wins[idx].open {
                    let mut nx = self.input.x - self.drag_dx;
                    let mut ny = self.input.y - self.drag_dy;
                    if nx < 40 - wins[idx].w { nx = 40 - wins[idx].w; }
                    if nx > w - 80 { nx = w - 80; }
                    if ny < 0 { ny = 0; }
                    if ny > h - TB_H - 60 { ny = h - TB_H - 60; }
                    wins[idx].x = nx;
                    wins[idx].y = ny;
                    wins[idx].max = false;
                }
            } else {
                self.drag = -1;
            }
        }

        if self.launchpad {
            if self.lp_t < 16 { self.lp_t += 1; }
            self.hover = -1;
            let (px, py, pw, ph) = self.start_geom();
            if self.input.x >= px && self.input.x < px + pw && self.input.y >= py && self.input.y < py + ph {
                let by = py + ph - 48;
                if self.input.y >= by && self.input.y < by + 34 {
                    if self.input.x > px + pw - 92 && self.input.x < px + pw - 58 { self.hover = 200; }
                    if self.input.x > px + pw - 46 && self.input.x < px + pw - 12 { self.hover = 201; }
                } else {
                    let (_, n) = self.filtered();
                    let gx = px + 30;
                    let gy = py + 96;
                    let mut i = 0;
                    while i < n {
                        let cx = gx + (i as i32 % 6) * 96;
                        let cy = gy + (i as i32 / 6) * 92;
                        if self.input.x >= cx - 2 && self.input.x < cx + 90
                            && self.input.y >= cy - 6 && self.input.y < cy + 84
                        {
                            self.hover = 100 + i as i32;
                        }
                        i += 1;
                    }
                }
            }
            if self.input.clicked {
                if self.input.y >= h - TB_H {
                    self.launchpad = false;
                    self.lp_t = 0;
                    self.search_len = 0;
                    self.click_taskbar(&mut action);
                } else if self.hover == 200 {
                    self.open_app(AppId::Settings);
                } else if self.hover == 201 {
                    self.launchpad = false;
                    self.lp_t = 0;
                    self.search_len = 0;
                } else if self.hover >= 100 {
                    let (apps, n) = self.filtered();
                    let idx = (self.hover - 100) as usize;
                    if idx < n { self.open_app(apps[idx]); }
                } else {
                    let (px2, py2, pw2, ph2) = self.start_geom();
                    if !(self.input.x >= px2 && self.input.x < px2 + pw2
                        && self.input.y >= py2 && self.input.y < py2 + ph2)
                    {
                        self.launchpad = false;
                        self.lp_t = 0;
                        self.search_len = 0;
                    }
                }
            }
        } else if self.menu {
            if self.input.clicked || self.input.right_clicked {
                if self.input.clicked {
                    let mut pick = 255u8;
                    let rel = self.input.y - self.menu_y - 8;
                    if rel >= 0 {
                        let i = rel / 32;
                        if i < 5 && self.input.x > self.menu_x + 6 && self.input.x < self.menu_x + 196 {
                            pick = i as u8;
                        }
                    }
                    self.menu = false;
                    match pick {
                        0 => { self.open_app(AppId::Terminal); }
                        1 => { self.open_app(AppId::Files); }
                        2 => { self.open_app(AppId::Settings); }
                        3 => { self.open_app(AppId::About); }
                        4 => { action = AppAction::Screenshot; }
                        _ => {}
                    }
                } else {
                    self.menu = false;
                }
            }
        } else {
            self.hover = self.tb_slot_at(self.input.x, self.input.y);
            if self.input.right_clicked {
                if self.hit_window(self.input.x, self.input.y).is_none()
                    && self.input.y >= 0 && self.input.y < h - TB_H - 8
                    && self.input.x >= 0 && self.input.x < w
                {
                    let mut mx = self.input.x;
                    let mut my = self.input.y;
                    if mx > w - 200 { mx = w - 200; }
                    if my > h - TB_H - 196 { my = h - TB_H - 196; }
                    self.menu = true;
                    self.menu_x = mx;
                    self.menu_y = my;
                }
            }
            if self.input.clicked {
                if self.input.y >= h - TB_H {
                    self.click_taskbar(&mut action);
                } else if let Some(di) = self.desk_icon_at(self.input.x, self.input.y) {
                    self.desk_sel = di as i32;
                    if self.last_click_idx == di as i32 && self.frame.saturating_sub(self.last_click_frame) < 36 {
                        self.last_click_idx = -1;
                        self.open_app(DESK_ICONS[di]);
                    } else {
                        self.last_click_idx = di as i32;
                        self.last_click_frame = self.frame;
                    }
                } else if let Some(idx) = self.hit_window(self.input.x, self.input.y) {
                    FOCUS = idx;
                    let wins = &mut *core::ptr::addr_of_mut!(WINS);
                    let wx = wins[idx].x;
                    let wy = wins[idx].y;
                    let ww = wins[idx].w;
                    if self.input.y < wy + apps::HEADER_H {
                        if self.input.x > wx + ww - 44 && self.input.x < wx + ww - 16 {
                            action = AppAction::Close;
                        } else if self.input.x > wx + ww - 70 && self.input.x < wx + ww - 42 {
                            self.toggle_max(idx);
                        } else if self.input.x > wx + ww - 96 && self.input.x < wx + ww - 68 {
                            wins[idx].min = true;
                            let mut last = usize::MAX;
                            let mut k = 0;
                            while k < 3 {
                                if wins[k].open && !wins[k].min { last = k; }
                                k += 1;
                            }
                            FOCUS = if last != usize::MAX { last } else { 0 };
                        } else {
                            self.drag = idx as i32;
                            self.drag_dx = self.input.x - wx;
                            self.drag_dy = self.input.y - wy;
                        }
                    } else {
                        action = apps::click(&mut wins[idx].app, wins[idx].id, self.input.x, self.input.y, wx, wy, ww, wins[idx].h);
                    }
                } else {
                    self.hover = -1;
                    self.desk_sel = -1;
                }
            }
        }

        let mut open_any = false;
        {
            let wins = &*core::ptr::addr_of!(WINS);
            let mut i = 0;
            while i < 3 {
                if wins[i].open && !wins[i].min { open_any = true; }
                i += 1;
            }
        }
        let desktop = !open_any && !self.launchpad;
        while let Some((scan, ch)) = read_key() {
            if scan == 0x5b {
                self.launchpad = !self.launchpad;
                self.lp_t = 0;
                self.search_len = 0;
                self.menu = false;
                continue;
            }
            if desktop && scan == 0x0b {
                self.launchpad = true;
                self.lp_t = 16;
                self.search_len = 0;
                continue;
            }
            if desktop && scan == 0x11 { self.open_app(AppId::Terminal); continue; }
            if desktop && ch == 13 && self.desk_sel >= 0 {
                let d = self.desk_sel;
                self.desk_sel = -1;
                self.open_app(DESK_ICONS[d as usize]);
                continue;
            }
            if desktop && (scan == 0x48 || scan == 0x50) {
                if scan == 0x48 {
                    if self.desk_sel <= 0 { self.desk_sel = DESK_ICONS.len() as i32 - 1; }
                    else { self.desk_sel -= 1; }
                } else {
                    if self.desk_sel >= DESK_ICONS.len() as i32 - 1 { self.desk_sel = 0; }
                    else { self.desk_sel += 1; }
                }
                continue;
            }
            if desktop && scan == 0x3c {
                action = AppAction::Screenshot;
                continue;
            }
            if scan == 0x3e {
                action = AppAction::Close;
                continue;
            }
            if !desktop && scan == 0x0f {
                let wins = &*core::ptr::addr_of!(WINS);
                let mut nxt = FOCUS;
                let mut k = 0;
                while k < 3 {
                    let cand = (FOCUS + k + 1) % 3;
                    if wins[cand].open && !wins[cand].min { nxt = cand; break; }
                    k += 1;
                }
                FOCUS = nxt;
                continue;
            }
            if scan == 0x44 || scan == 0x5d {
                self.menu = true;
                self.menu_x = self.w() / 2 - 95;
                self.menu_y = self.h() / 2 - 90;
                continue;
            }
            if scan == 0x57 && !desktop {
                self.toggle_max(FOCUS);
                continue;
            }
            if self.menu && (scan == 0x01 || (scan == 0x17 && ch == 0)) {
                self.menu = false;
                continue;
            }
            if self.menu && ch >= '1' as u16 && ch <= '5' as u16 {
                let pick = (ch - '1' as u16) as u8;
                self.menu = false;
                match pick {
                    0 => { self.open_app(AppId::Terminal); }
                    1 => { self.open_app(AppId::Files); }
                    2 => { self.open_app(AppId::Settings); }
                    3 => { self.open_app(AppId::About); }
                    4 => { action = AppAction::Screenshot; }
                    _ => {}
                }
                continue;
            }
            if self.launchpad {
                if scan == 0x01 || (scan == 0x17 && ch == 0) {
                    self.launchpad = false;
                    self.lp_t = 0;
                    self.search_len = 0;
                } else if ch == 13 {
                    let (apps, n) = self.filtered();
                    if n > 0 { self.open_app(apps[0]); }
                } else if scan == 0x08 || ch == 8 {
                    if self.search_len > 0 { self.search_len -= 1; }
                } else if ch >= 32 && ch < 127 && self.search_len < 20 {
                    let c = ch as u8;
                    self.search[self.search_len] = if c >= b'A' && c <= b'Z' { c + 32 } else { c };
                    self.search_len += 1;
                }
            } else {
                let wins = &mut *core::ptr::addr_of_mut!(WINS);
                let mut fi = usize::MAX;
                let mut i = 0;
                while i < 3 {
                    if wins[i].open { fi = i; }
                    i += 1;
                }
                if fi != usize::MAX {
                    let a = apps::key(&mut wins[fi].app, wins[fi].id, scan, ch);
                    if a != AppAction::None { action = a; }
                }
            }
        }

        match action {
            AppAction::Close => self.close_window(FOCUS),
            AppAction::Screenshot => {
                self.take_screenshot();
                self.shot_flash = 8;
            }
            AppAction::OpenUpdate => {
                let mut found = false;
                let wins = &*core::ptr::addr_of!(WINS);
                let mut i = 0;
                while i < 3 {
                    if wins[i].open && wins[i].id == AppId::Settings { FOCUS = i; found = true; }
                    i += 1;
                }
                if !found { self.open_app(AppId::Settings); }
                let wins = &mut *core::ptr::addr_of_mut!(WINS);
                wins[FOCUS].app.settings.page = 2;
            }
            _ => {}
        }

        self.draw();
        if self.shot_flash > 0 {
            self.g.rect(0, 0, w, h, rgb(255, 255, 255), (self.shot_flash * 12) as u8);
            self.shot_flash -= 1;
        }
        self.draw_cursor();
        self.present();
        true
    }

    unsafe fn draw(&self) {
        self.restore_bg();
        self.draw_desk_icons();
        let wins = &*core::ptr::addr_of!(WINS);
        let mut focus = FOCUS;
        if focus >= 3 || !wins[focus].open || wins[focus].min { focus = usize::MAX; }
        let mut i = 0;
        while i < 3 {
            if i != focus { self.draw_window(i, false); }
            i += 1;
        }
        if focus < 3 { self.draw_window(focus, true); }
        self.draw_taskbar();
        if self.menu {
            self.draw_menu();
        }
        if self.launchpad {
            self.draw_start();
        }
    }

    unsafe fn draw_menu(&self) {
        let g = &self.g;
        let x = self.menu_x;
        let y = self.menu_y;
        let items = ["打开终端", "文件管理器", "系统设置", "关于 HYWOS", "截取屏幕"];
        let iw = 190i32;
        let ih = 8 + 5 * 32 + 6;
        g.round_rect(x + 3, y + 3, iw, ih, 10, 0x000000, 70);
        g.round_rect(x, y, iw, ih, 10, rgb(22, 26, 38), 246);
        g.round_rect_outline(x, y, iw, ih, 10, rgb(255, 255, 255), 40);
        let mut i = 0;
        while i < 5 {
            let iy = y + 8 + i * 32;
            if self.input.x > x && self.input.x < x + iw && self.input.y >= iy && self.input.y < iy + 30 {
                g.round_rect(x + 6, iy, iw - 12, 30, 7, rgb(64, 120, 210), 150);
            }
            g.text(x + 18, iy + 8, items[i as usize], rgb(230, 238, 250), 1);
            i += 1;
        }
    }
}

pub unsafe fn window_count() -> usize {
    let wins = &*core::ptr::addr_of!(WINS);
    let mut n = 0usize;
    let mut i = 0;
    while i < 3 {
        if wins[i].open { n += 1; }
        i += 1;
    }
    n
}
