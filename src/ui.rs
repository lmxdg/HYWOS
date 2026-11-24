use crate::apps::{self, AppAction, AppState};
use crate::efi::*;
use crate::fs;
use crate::gfx::{rgb, text_width, Gfx};
use crate::hw;
use crate::icon::{icon, AppId};

pub const DOCK_APPS: [AppId; 18] = [
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
];

pub struct Win {
    pub id: AppId,
    pub open: bool,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub app: AppState,
}

static mut WINS: [Win; 3] = unsafe { core::mem::zeroed() };
static mut BG: [u32; 1024 * 768] = [0; 1024 * 768];
static mut SNAP: [u32; 1024 * 768] = [0; 1024 * 768];
static mut BACK: [u32; 1024 * 768] = [0; 1024 * 768];
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
}

impl Ui {
    pub unsafe fn new(front: Gfx) -> Ui {
        let cx = front.w as i32 / 2;
        let cy = front.h as i32 / 2;
        let n = front.w * front.h;
        let back_ptr = core::ptr::addr_of_mut!(BACK) as *mut u32;
        let g = if n <= 1024 * 768 {
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
        g.vgrad(0, 0, w, h, rgb(9, 13, 30), rgb(6, 5, 14));
        g.circle(w * 3 / 4, h / 4, 360, rgb(44, 96, 190), 34);
        g.circle(w / 5, h * 3 / 4, 320, rgb(138, 60, 150), 24);
        g.circle(w * 2 / 3, h * 2 / 3, 400, rgb(26, 54, 120), 16);
        let mut seed = 0x2545f491u32;
        let mut i = 0;
        while i < 110 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let x = (seed % w as u32) as i32;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let y = (seed % (h as u32 / 2)) as i32;
            let b = ((seed >> 8) % 60) as u8 + 90;
            g.put(x, y, rgb(b, b, (b as u32 + 40).min(255) as u8), 120);
            i += 1;
        }
        let mut x = 0;
        while x < w {
            g.rect(x, 0, 1, h, rgb(255, 255, 255), 3);
            x += 64;
        }
        let mut y = 0;
        while y < h {
            g.rect(0, y, w, 1, rgb(255, 255, 255), 2);
            y += 64;
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

    fn dock_geom(&self) -> (i32, i32, i32, i32, i32, i32) {
        let w = self.w();
        let (iw, gap) = if w >= 1040 { (46, 5) } else if w >= 900 { (42, 5) } else { (34, 5) };
        let dw = 19 * iw + 18 * gap + 28;
        let dx = (w - dw) / 2;
        let dh = iw + 22;
        let dy = self.h() - dh - 10;
        (dx, dy, dw, dh, iw, gap)
    }

    fn dock_slot_at(&self, mx: i32, my: i32) -> i32 {
        let (dx, dy, dw, dh, iw, gap) = self.dock_geom();
        if mx < dx || mx >= dx + dw || my < dy || my >= dy + dh { return -1; }
        let rel = mx - (dx + 10);
        if rel < 0 { return -1; }
        let slot = rel / (iw + gap);
        if slot < 0 || slot >= 19 { -1 } else { slot }
    }

    unsafe fn draw_dock(&self) {
        let g = &self.g;
        let (dx, dy, dw, dh, iw, gap) = self.dock_geom();
        g.round_rect(dx - 8, dy - 6, dw + 16, dh + 12, 28, 0x000000, 76);
        g.round_rect(dx, dy, dw, dh, 24, rgb(15, 18, 30), 214);
        g.round_rect(dx, dy, dw, 1, 0, rgb(255, 255, 255), 46);
        g.round_rect(dx + 1, dy + 1, dw - 2, dh / 2, 22, rgb(255, 255, 255), 10);
        let mut i = 0;
        while i < 19 {
            let ix = dx + 14 + i * (iw + gap);
            let iy = dy + (dh - iw) / 2;
            if i < 18 {
                let ic = icon(DOCK_APPS[i as usize]);
                if self.hover == i as i32 {
                    g.round_rect(ix - 4, iy - 4, iw + 8, iw + 8, 13, rgb(255, 255, 255), 44);
                    g.blit_rgba(ix - 3, iy - 3, iw + 6, iw + 6, ic, 64, 64);
                } else {
                    g.blit_rgba(ix, iy, iw, iw, ic, 64, 64);
                }
            } else {
                let cx = ix + iw / 2;
                let cy = iy + iw / 2;
                if self.hover == 18 {
                    g.round_rect(ix - 4, iy - 4, iw + 8, iw + 8, 13, rgb(255, 255, 255), 44);
                }
                g.round_rect(ix, iy, iw, iw, 11, rgb(30, 36, 54), 245);
                let unit = iw / 5;
                let mut r = 0;
                while r < 3 {
                    let mut c = 0;
                    while c < 3 {
                        g.circle(cx - unit + c * unit, cy - unit + r * unit, 2, rgb(224, 234, 255), 235);
                        c += 1;
                    }
                    r += 1;
                }
            }
            i += 1;
        }
        if self.hover >= 0 && self.hover < 19 {
            let label = if self.hover == 18 { "启动台" } else { DOCK_APPS[self.hover as usize].name() };
            let lw = text_width(label, 1) + 22;
            let lx = dx + 14 + self.hover * (iw + gap) + iw / 2 - lw / 2;
            g.round_rect(lx, dy - 32, lw, 24, 9, rgb(8, 10, 18), 180);
            g.text_center(lx + lw / 2, dy - 27, label, rgb(240, 246, 255), 1);
        }
    }

    unsafe fn draw_status(&self) {
        let g = &self.g;
        let w = self.w();
        g.rect(0, 0, w, 30, rgb(9, 12, 24), 205);
        g.rect(0, 30, w, 1, rgb(255, 255, 255), 16);
        g.circle(18, 15, 7, rgb(110, 200, 255), 255);
        g.circle(18, 15, 3, rgb(230, 246, 255), 255);
        g.text(32, 7, "HYWOS 1.0.7", rgb(232, 242, 255), 1);
        g.text(32 + text_width("HYWOS 1.0.7", 1) + 10, 7, "星河", rgb(146, 198, 255), 1);
        let c = hw::cpu();
        let mut b = [0u8; 64];
        let mut l = 0;
        l += hw::num2(&mut b[l..], c.enabled as u32, 1);
        for ch in " 核".as_bytes().iter() { b[l] = *ch; l += 1; }
        g.text(w - 352, 7, core::str::from_utf8_unchecked(&b[..l]), rgb(150, 180, 214), 1);
        let net = hw::net();
        if net.present {
            let mut mb = [0u8; 40];
            let mut ml = 0;
            let mut i = 0;
            while i < 6 {
                let hi = net.mac[i] >> 4;
                let lo = net.mac[i] & 0xf;
                mb[ml] = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 }; ml += 1;
                mb[ml] = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 }; ml += 1;
                if i < 5 { mb[ml] = b':'; ml += 1; }
                i += 1;
            }
            g.ring(w - 296, 15, 7, 2, rgb(120, 220, 180), 220);
            g.text(w - 280, 7, core::str::from_utf8_unchecked(&mb[..ml]), rgb(138, 168, 200), 1);
        } else {
            g.text(w - 300, 7, "网络：未发现网卡", rgb(150, 132, 142), 1);
        }
        let mut tb = [0u8; 16];
        let mut tl = 0;
        match hw::cached_time() {
            Some(t) => {
                if t.hour > 23 || t.minute > 59 || t.month == 0 {
                    for ch in b"--:--".iter() { tb[tl] = *ch; tl += 1; }
                } else {
                    tl += hw::num2(&mut tb[tl..], t.hour as u32, 2);
                    tb[tl] = b':'; tl += 1;
                    tl += hw::num2(&mut tb[tl..], t.minute as u32, 2);
                }
            }
            None => {
                for ch in b"--:--".iter() { tb[tl] = *ch; tl += 1; }
            }
        }
        g.text(w - 112, 7, core::str::from_utf8_unchecked(&tb[..tl]), rgb(236, 244, 255), 1);
        g.round_rect(w - 64, 10, 22, 11, 3, rgb(120, 160, 200), 200);
        g.rect(w - 41, 13, 2, 5, rgb(120, 160, 200), 200);
        g.rect(w - 62, 12, 14, 7, rgb(110, 220, 150), 240);
        let msg = &*core::ptr::addr_of!(SHOT_MSG);
        let ml = SHOT_MSG_LEN;
        if ml > 0 {
            g.text(250, 7, core::str::from_utf8_unchecked(&msg[..ml]), rgb(140, 230, 170), 1);
        }
    }

    unsafe fn draw_window(&self, i: usize, focus: bool) {
        let wins = &mut *core::ptr::addr_of_mut!(WINS);
        if !wins[i].open { return; }
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

    fn filtered(&self) -> ([AppId; 19], usize) {
        let mut out = [AppId::Terminal; 19];
        let mut n = 0;
        let mut i = 0;
        while i < 19 {
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

    unsafe fn draw_launchpad(&self) {
        let g = &self.g;
        let t = self.lp_t;
        let dim = if t < 16 { (t * 13) as u8 } else { 208 };
        g.dim(dim);
        g.text_center(self.w() / 2, 46, "全部应用", rgb(236, 244, 255), 2);
        let (apps, n) = self.filtered();
        let cols = 6usize;
        let iw = 108i32;
        let ih = 100i32;
        let gx = (self.w() - cols as i32 * iw) / 2;
        let gy = 94;
        let mut i = 0;
        while i < n {
            let app = apps[i];
            let col = (i % cols) as i32;
            let row = (i / cols) as i32;
            let delay = row * 2 + col;
            let local = (t - delay).max(0).min(10);
            let scale = 60 + local * 4;
            let alpha = (local * 25).min(255) as u8;
            let cx = gx + col * iw + iw / 2;
            let cy = gy + row * ih;
            let sz = scale * 64 / 100;
            let ic = icon(app);
            let draw_w = if t >= 16 { 64 } else { sz };
            g.blit_rgba(cx - draw_w / 2, cy + (64 - draw_w) / 2, draw_w, draw_w, ic, 64, 64);
            if t > 6 + delay {
                let nw = text_width(app.name(), 1);
                g.text_a(cx - nw / 2, cy + 70, app.name(), rgb(240, 246, 255), 1, alpha.min(220));
            }
            i += 1;
        }
        let sw = 430;
        let sx = (self.w() - sw) / 2;
        let sy = self.h() - 98;
        g.round_rect(sx, sy, sw, 42, 21, rgb(18, 24, 40), 226);
        g.round_rect_outline(sx, sy, sw, 42, 21, rgb(255, 255, 255), 46);
        g.ring(sx + 24, sy + 21, 8, 2, rgb(180, 200, 226), 200);
        g.line(sx + 30, sy + 27, sx + 36, sy + 33, rgb(180, 200, 226), 200);
        if self.search_len > 0 {
            let s = core::str::from_utf8_unchecked(&self.search[..self.search_len]);
            g.text(sx + 46, sy + 13, s, rgb(236, 244, 255), 1);
        } else {
            g.text(sx + 46, sy + 13, "搜索应用", rgb(140, 156, 180), 1);
        }
        if (self.frame / 18) % 2 == 0 {
            let q = core::str::from_utf8_unchecked(&self.search[..self.search_len]);
            let cx = sx + 46 + text_width(q, 1);
            g.rect(cx, sy + 13, 6, 15, rgb(160, 200, 255), 200);
        }
        if n == 0 {
            g.text_center(self.w() / 2, sy - 44, "没有匹配的应用", rgb(200, 212, 232), 1);
        }
        g.text_center(self.w() / 2, sy + 54, "按 Esc 关闭 · 回车启动第一个结果", rgb(150, 168, 196), 1);
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
            let max_h = self.h() - 150;
            if h > max_h { h = max_h; }
            if w > self.w() - 40 { w = self.w() - 40; }
            wins[slot].id = id;
            wins[slot].open = true;
            wins[slot].w = w;
            wins[slot].h = h;
            wins[slot].x = (self.w() - w) / 2;
            wins[slot].y = 60 + slot as i32 * 18;
            apps::init_state(&mut wins[slot].app, id);
            FOCUS = slot;
        } else {
            FOCUS = found;
        }
        self.launchpad = false;
        self.lp_t = 0;
        self.search_len = 0;
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
        if self.g.w > 1024 || self.g.h > 768 { return; }
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

        if self.launchpad {
            if self.lp_t < 16 { self.lp_t += 1; }
            if self.input.clicked {
                let (apps, n) = self.filtered();
                let cols = 6usize;
                let iw = 108i32;
                let ih = 118i32;
                let gx = (w - cols as i32 * iw) / 2;
                let gy = 94;
                let mut hit = false;
                let mut i = 0;
                while i < n {
                    let col = (i % cols) as i32;
                    let row = (i / cols) as i32;
                    let cx = gx + col * iw;
                    let cy = gy + row * ih;
                    if self.input.x >= cx + 10 && self.input.x < cx + iw - 10 && self.input.y >= cy && self.input.y < cy + 92 {
                        self.open_app(apps[i]);
                        hit = true;
                        break;
                    }
                    i += 1;
                }
                if !hit && self.input.y < h - 130 {
                    self.launchpad = false;
                    self.lp_t = 0;
                    self.search_len = 0;
                }
            }
        } else {
            self.hover = self.dock_slot_at(self.input.x, self.input.y);
            if self.input.clicked {
                if self.input.y < 28 {
                } else if self.input.y > h - 90 {
                    let slot = self.dock_slot_at(self.input.x, self.input.y);
                    if slot >= 0 && slot < 18 {
                        self.open_app(DOCK_APPS[slot as usize]);
                    } else if slot == 18 {
                        self.launchpad = true;
                        self.lp_t = 0;
                        self.search_len = 0;
                    }
                } else if let Some(idx) = self.hit_window(self.input.x, self.input.y) {
                    FOCUS = idx;
                    let wins = &mut *core::ptr::addr_of_mut!(WINS);
                    action = apps::click(&mut wins[idx].app, wins[idx].id, self.input.x, self.input.y, wins[idx].x, wins[idx].y, wins[idx].w, wins[idx].h);
                } else {
                    self.hover = -1;
                }
            }
        }

        while let Some((scan, ch)) = read_key() {
            if scan == 0x0b {
                self.launchpad = true;
                self.lp_t = 16;
                self.search_len = 0;
                continue;
            }
            if scan == 0x0c { self.open_app(AppId::Weather); continue; }
            if scan == 0x0d {
                self.open_app(AppId::Settings);
                let wins = &mut *core::ptr::addr_of_mut!(WINS);
                wins[FOCUS].app.settings.page = 1;
                continue;
            }
            if scan == 0x0e { self.open_app(AppId::Store); continue; }
            if scan == 0x0f { self.open_app(AppId::About); continue; }
            if scan == 0x10 { self.open_app(AppId::Files); continue; }
            if scan == 0x11 { self.open_app(AppId::Terminal); continue; }
            if scan == 0x12 { self.open_app(AppId::Files); continue; }
            if scan == 0x16 || scan == 0x1d {
                action = AppAction::Screenshot;
                continue;
            }
            if self.launchpad {
                if scan == 0x17 {
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
                if scan == 0x17 {
                    action = AppAction::Close;
                    continue;
                }
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
        let focus = FOCUS;
        let mut i = 0;
        while i < 3 {
            if i != focus { self.draw_window(i, false); }
            i += 1;
        }
        if focus < 3 { self.draw_window(focus, true); }
        if !self.launchpad {
            self.draw_dock();
        }
        self.draw_status();
        if self.launchpad {
            self.draw_launchpad();
        }
    }
}