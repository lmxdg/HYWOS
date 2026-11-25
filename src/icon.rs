use crate::font;
use crate::gfx::{lerp, rgb};

pub const APP_COUNT: usize = 20;
pub const ICON_SRC: usize = 256;
pub const ICON_SIZE: usize = 64;
pub const ICON_BYTES: usize = ICON_SIZE * ICON_SIZE * 4;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AppId {
    Terminal = 0,
    Files = 1,
    Settings = 2,
    Store = 3,
    About = 4,
    Browser = 5,
    Mail = 6,
    Photos = 7,
    Music = 8,
    Calendar = 9,
    Calculator = 10,
    Notes = 11,
    Maps = 12,
    Clock = 13,
    Camera = 14,
    Videos = 15,
    Contacts = 16,
    Weather = 17,
    Messages = 18,
    Security = 19,
}

impl AppId {
    pub fn from_index(i: usize) -> AppId {
        match i {
            0 => AppId::Terminal,
            1 => AppId::Files,
            2 => AppId::Settings,
            3 => AppId::Store,
            4 => AppId::About,
            5 => AppId::Browser,
            6 => AppId::Mail,
            7 => AppId::Photos,
            8 => AppId::Music,
            9 => AppId::Calendar,
            10 => AppId::Calculator,
            11 => AppId::Notes,
            12 => AppId::Maps,
            13 => AppId::Clock,
            14 => AppId::Camera,
            15 => AppId::Videos,
            16 => AppId::Contacts,
            17 => AppId::Weather,
            18 => AppId::Messages,
            _ => AppId::Security,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            AppId::Terminal => "终端",
            AppId::Files => "文件",
            AppId::Settings => "设置",
            AppId::Store => "应用商店",
            AppId::About => "关于本机",
            AppId::Browser => "浏览器",
            AppId::Mail => "邮件",
            AppId::Photos => "照片",
            AppId::Music => "音乐",
            AppId::Calendar => "日历",
            AppId::Calculator => "计算器",
            AppId::Notes => "备忘录",
            AppId::Maps => "地图",
            AppId::Clock => "时钟",
            AppId::Camera => "相机",
            AppId::Videos => "视频",
            AppId::Contacts => "通讯录",
            AppId::Weather => "天气",
            AppId::Messages => "信息",
            AppId::Security => "安全中心",
        }
    }

    pub fn id_str(self) -> &'static str {
        match self {
            AppId::Terminal => "terminal",
            AppId::Files => "files",
            AppId::Settings => "settings",
            AppId::Store => "store",
            AppId::About => "about",
            AppId::Browser => "browser",
            AppId::Mail => "mail",
            AppId::Photos => "photos",
            AppId::Music => "music",
            AppId::Calendar => "calendar",
            AppId::Calculator => "calculator",
            AppId::Notes => "notes",
            AppId::Maps => "maps",
            AppId::Clock => "clock",
            AppId::Camera => "camera",
            AppId::Videos => "videos",
            AppId::Contacts => "contacts",
            AppId::Weather => "weather",
            AppId::Messages => "messages",
            AppId::Security => "security",
        }
    }

    pub fn colors(self) -> (u32, u32) {
        match self {
            AppId::Terminal => (rgb(43, 47, 58), rgb(16, 18, 24)),
            AppId::Files => (rgb(91, 168, 255), rgb(28, 104, 224)),
            AppId::Settings => (rgb(136, 146, 166), rgb(70, 78, 96)),
            AppId::Store => (rgb(57, 192, 255), rgb(10, 99, 216)),
            AppId::About => (rgb(124, 108, 255), rgb(58, 46, 176)),
            AppId::Browser => (rgb(79, 209, 240), rgb(18, 119, 200)),
            AppId::Mail => (rgb(94, 168, 255), rgb(22, 86, 200)),
            AppId::Photos => (rgb(255, 200, 74), rgb(232, 88, 110)),
            AppId::Music => (rgb(255, 110, 158), rgb(180, 30, 106)),
            AppId::Calendar => (rgb(255, 255, 255), rgb(232, 236, 246)),
            AppId::Calculator => (rgb(90, 97, 114), rgb(42, 46, 58)),
            AppId::Notes => (rgb(255, 224, 102), rgb(232, 185, 60)),
            AppId::Maps => (rgb(110, 216, 166), rgb(30, 148, 104)),
            AppId::Clock => (rgb(62, 68, 86), rgb(21, 24, 31)),
            AppId::Camera => (rgb(108, 116, 134), rgb(44, 48, 58)),
            AppId::Videos => (rgb(255, 138, 94), rgb(200, 64, 46)),
            AppId::Contacts => (rgb(120, 196, 255), rgb(42, 108, 200)),
            AppId::Weather => (rgb(111, 182, 255), rgb(37, 104, 200)),
            AppId::Messages => (rgb(99, 224, 138), rgb(30, 158, 80)),
            AppId::Security => (rgb(86, 214, 158), rgb(16, 122, 88)),
        }
    }
}

static mut ICON64: [[u8; ICON_BYTES]; APP_COUNT] = [[0; ICON_BYTES]; APP_COUNT];
static mut HIC: [u8; ICON_SRC * ICON_SRC * 4] = [0; ICON_SRC * ICON_SRC * 4];
static mut READY: [bool; APP_COUNT] = [false; APP_COUNT];

struct Canvas {
    p: *mut u8,
    w: usize,
    h: usize,
}

impl Canvas {
    fn clear(&self) {
        unsafe {
            let s = core::slice::from_raw_parts_mut(self.p, self.w * self.h * 4);
            let mut i = 0;
            while i < s.len() {
                s[i] = 0;
                i += 1;
            }
        }
    }

    fn blend(&self, x: i32, y: i32, c: u32, a: u8) {
        if a == 0 || x < 0 || y < 0 { return; }
        let ux = x as usize;
        let uy = y as usize;
        if ux >= self.w || uy >= self.h { return; }
        let i = (uy * self.w + ux) * 4;
        let sr = ((c >> 16) & 0xff) as u32;
        let sg = ((c >> 8) & 0xff) as u32;
        let sb = (c & 0xff) as u32;
        let sa = a as u32;
        unsafe {
            let p = self.p.add(i);
            let da = *p.add(3) as u32;
            let oa = sa + da * (255 - sa) / 255;
            if oa == 0 { return; }
            let dr = *p as u32;
            let dg = *p.add(1) as u32;
            let db = *p.add(2) as u32;
            let r = (sr * sa + dr * da * (255 - sa) / 255) / oa;
            let g = (sg * sa + dg * da * (255 - sa) / 255) / oa;
            let b = (sb * sa + db * da * (255 - sa) / 255) / oa;
            *p = r as u8;
            *p.add(1) = g as u8;
            *p.add(2) = b as u8;
            *p.add(3) = oa as u8;
        }
    }

    fn rect(&self, x: i32, y: i32, w: i32, h: i32, c: u32, a: u8) {
        let mut yy = 0;
        while yy < h {
            let mut xx = 0;
            while xx < w {
                self.blend(x + xx, y + yy, c, a);
                xx += 1;
            }
            yy += 1;
        }
    }

    fn vgrad(&self, x: i32, y: i32, w: i32, h: i32, top: u32, bot: u32) {
        let mut i = 0;
        while i < h {
            let t = (i * 255 / if h > 1 { h - 1 } else { 1 }) as u32;
            self.rect(x, y + i, w, 1, lerp(top, bot, t), 255);
            i += 1;
        }
    }

    fn round_rect(&self, x: i32, y: i32, w: i32, h: i32, r: i32, c: u32, a: u8) {
        let r = if r * 2 > w { w / 2 } else { r };
        let r = if r * 2 > h { h / 2 } else { r };
        self.rect(x + r, y, w - 2 * r, h, c, a);
        self.rect(x, y + r, r, h - 2 * r, c, a);
        self.rect(x + w - r, y + r, r, h - 2 * r, c, a);
        let rf = r as f32;
        let mut cy = 0;
        while cy < r {
            let mut cx = 0;
            while cx < r {
                let dx = (r - cx) as f32 - 0.5;
                let dy = (r - cy) as f32 - 0.5;
                let d = crate::gfx::sqrtf(dx * dx + dy * dy);
                let cov = rf + 0.5 - d;
                if cov > 0.0 {
                    let aa = if cov >= 1.0 { a } else { (a as f32 * cov) as u8 };
                    self.blend(x + cx, y + cy, c, aa);
                    self.blend(x + w - 1 - cx, y + cy, c, aa);
                    self.blend(x + cx, y + h - 1 - cy, c, aa);
                    self.blend(x + w - 1 - cx, y + h - 1 - cy, c, aa);
                }
                cx += 1;
            }
            cy += 1;
        }
        let mut ry = 0;
        while ry < r {
            self.blend(x + r, y + ry, c, a);
            self.blend(x + w - r - 1, y + ry, c, a);
            self.blend(x + r, y + h - 1 - ry, c, a);
            self.blend(x + w - r - 1, y + h - 1 - ry, c, a);
            ry += 1;
        }
    }

    fn circle(&self, cx: i32, cy: i32, r: i32, c: u32, a: u8) {
        let r2 = r * r;
        let mut y = -r;
        while y <= r {
            let mut x = -r;
            while x <= r {
                let d2 = x * x + y * y;
                if d2 <= r2 {
                    let cov = ((r2 - d2) as f32 / (2.0 * r as f32)).min(1.0);
                    let aa = if cov >= 1.0 { a } else { (a as f32 * cov) as u8 };
                    self.blend(cx + x, cy + y, c, aa);
                }
                x += 1;
            }
            y += 1;
        }
    }

    fn ring(&self, cx: i32, cy: i32, r: i32, thick: i32, c: u32, a: u8) {
        let ro = r;
        let ri = r - thick;
        let mut y = -ro;
        while y <= ro {
            let mut x = -ro;
            while x <= ro {
                let d2 = x * x + y * y;
                if d2 <= ro * ro && d2 >= ri * ri {
                    self.blend(cx + x, cy + y, c, a);
                }
                x += 1;
            }
            y += 1;
        }
    }

    fn line(&self, x0: i32, y0: i32, x1: i32, y1: i32, c: u32, a: u8) {
        let mut x = x0;
        let mut y = y0;
        let dx = if x1 > x0 { x1 - x0 } else { x0 - x1 };
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = if y1 > y0 { y0 - y1 } else { y1 - y0 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.blend(x, y, c, a);
            if x == x1 && y == y1 { break; }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    fn thick_line(&self, x0: i32, y0: i32, x1: i32, y1: i32, w: i32, c: u32, a: u8) {
        let mut i = 0;
        while i < w {
            self.line(x0 + i, y0, x1 + i, y1, c, a);
            i += 1;
        }
    }

    fn big_text(&self, x: i32, y: i32, s: &str, c: u32, scale: i32, alpha: u8) {
        let mut px = x;
        for ch in s.bytes() {
            let idx = if ch >= 32 && ch <= 126 { (ch - 32) as usize } else { 0 };
            let glyph = &font::FONT_16X32[idx * font::FONT_16X32_BPG..(idx + 1) * font::FONT_16X32_BPG];
            let mut gy = 0;
            while gy < 32 {
                let mut gx = 0;
                while gx < 16 {
                    let byte = glyph[gy * 8 + gx / 2];
                    let nib = if gx % 2 == 0 { byte >> 4 } else { byte & 0x0f };
                    if nib > 0 {
                        let a = ((nib as u32 * 17 * alpha as u32) / 255) as u8;
                        self.rect(px + gx as i32 * scale, y + gy as i32 * scale, scale, scale, c, a);
                    }
                    gx += 1;
                }
                gy += 1;
            }
            px += 16 * scale;
        }
    }
}

fn draw_symbol(cv: &Canvas, app: AppId) {
    let w = rgb(255, 255, 255);
    let cx = 128i32;
    let cy = 128i32;
    match app {
        AppId::Terminal => {
            cv.round_rect(56, 62, 144, 132, 14, rgb(10, 14, 20), 210);
            cv.big_text(74, 96, ">_", w, 5, 235);
        }
        AppId::Files => {
            cv.round_rect(64, 96, 128, 96, 12, rgb(255, 255, 255), 235);
            cv.round_rect(64, 84, 60, 26, 10, rgb(255, 255, 255), 235);
            cv.rect(64, 96, 128, 18, rgb(226, 236, 255), 255);
        }
        AppId::Settings => {
            let mut i = 0;
            while i < 8 {
                let ang = i as f32 * 0.785398;
                let x = cx + (crate::gfx::cosf(ang) * 74.0) as i32;
                let y = cy + (crate::gfx::sinf(ang) * 74.0) as i32;
                cv.circle(x, y, 18, w, 240);
                i += 1;
            }
            cv.circle(cx, cy, 58, w, 240);
            cv.circle(cx, cy, 30, rgb(84, 90, 106), 255);
        }
        AppId::Store => {
            cv.round_rect(76, 104, 104, 92, 14, w, 240);
            cv.ring(cx, 104, 40, 9, w, 240);
            cv.circle(cx, 158, 9, rgb(30, 110, 220), 255);
        }
        AppId::About => {
            cv.ring(cx, cy, 64, 10, w, 240);
            cv.circle(cx, 100, 12, w, 240);
            cv.rect(cx - 9, 122, 18, 60, w, 240);
        }
        AppId::Browser => {
            cv.ring(cx, cy, 70, 8, w, 240);
            cv.ring(cx, cy, 70, 2, w, 150);
            cv.line(cx - 70, cy, cx + 70, cy, w, 200);
            let mut i = 0;
            while i < 2 {
                let ex = if i == 0 { 44 } else { 30 };
                cv.ring(cx, cy, ex, 3, w, 190);
                i += 1;
            }
        }
        AppId::Mail => {
            cv.round_rect(66, 94, 124, 84, 10, w, 235);
            cv.thick_line(74, 102, cx, cy + 10, 6, rgb(150, 180, 235), 255);
            cv.thick_line(cx, cy + 10, 182, 102, 6, rgb(150, 180, 235), 255);
        }
        AppId::Photos => {
            cv.tri_shape(64, 180, 110, 96, 156, 180, w, 235);
            cv.tri_shape(110, 180, 162, 112, 200, 180, w, 200);
            cv.circle(176, 92, 22, rgb(255, 240, 190), 255);
        }
        AppId::Music => {
            cv.rect(150, 70, 12, 96, w, 235);
            cv.tri_shape(150, 70, 200, 56, 200, 88, w, 235);
            cv.circle(126, 166, 28, w, 235);
        }
        AppId::Calendar => {
            cv.round_rect(70, 84, 116, 108, 10, rgb(255, 255, 255), 250);
            cv.rect(70, 84, 116, 22, rgb(232, 64, 66), 255);
            let mut r = 0;
            while r < 3 {
                let mut c = 0;
                while c < 3 {
                    cv.circle(96 + c * 34, 128 + r * 26, 6, rgb(190, 196, 210), 255);
                    c += 1;
                }
                r += 1;
            }
        }
        AppId::Calculator => {
            cv.round_rect(74, 76, 108, 112, 12, rgb(255, 255, 255), 240);
            cv.rect(88, 90, 80, 24, rgb(60, 66, 82), 255);
            let mut r = 0;
            while r < 3 {
                let mut c = 0;
                while c < 3 {
                    cv.round_rect(88 + c * 28, 126 + r * 20, 20, 14, 4, rgb(120, 128, 146), 255);
                    c += 1;
                }
                r += 1;
            }
        }
        AppId::Notes => {
            cv.round_rect(74, 78, 108, 108, 10, rgb(255, 255, 255), 240);
            cv.rect(74, 78, 108, 12, rgb(240, 190, 60), 255);
            let mut i = 0;
            while i < 4 {
                cv.rect(92, 108 + i * 18, 70 - i * 8, 5, rgb(200, 204, 214), 255);
                i += 1;
            }
        }
        AppId::Maps => {
            cv.line(70, 170, 110, 96, w, 230);
            cv.line(110, 96, 150, 140, w, 230);
            cv.line(150, 140, 190, 86, w, 230);
            cv.circle(150, 96, 20, rgb(255, 255, 255), 255);
            cv.circle(150, 96, 9, rgb(220, 60, 60), 255);
        }
        AppId::Clock => {
            cv.circle(cx, cy, 76, rgb(250, 250, 252), 250);
            cv.line(cx, cy, cx, cy - 48, rgb(40, 44, 56), 255);
            cv.thick_line(cx, cy, cx + 38, cy + 14, 5, rgb(40, 44, 56), 255);
            cv.circle(cx, cy, 7, rgb(226, 72, 60), 255);
        }
        AppId::Camera => {
            cv.round_rect(64, 92, 128, 92, 16, rgb(40, 46, 58), 255);
            cv.circle(cx, 138, 34, rgb(18, 22, 30), 255);
            cv.circle(cx, 138, 24, rgb(110, 190, 255), 255);
            cv.circle(cx - 6, 130, 8, rgb(235, 246, 255), 220);
            cv.round_rect(92, 78, 26, 16, 5, rgb(40, 46, 58), 255);
        }
        AppId::Videos => {
            cv.circle(cx, cy, 72, rgb(255, 255, 255), 220);
            cv.tri_shape(cx - 22, cy - 34, cx - 22, cy + 34, cx + 40, cy, rgb(214, 68, 56), 255);
        }
        AppId::Contacts => {
            cv.circle(cx, 112, 30, w, 240);
            cv.circle(cx, 178, 56, w, 240);
            cv.rect(cx - 56, 196, 112, 20, rgb(40, 108, 200), 255);
        }
        AppId::Weather => {
            cv.circle(108, 106, 38, rgb(255, 214, 96), 255);
            cv.circle(150, 150, 34, w, 245);
            cv.circle(186, 154, 26, w, 245);
            cv.round_rect(118, 148, 92, 38, 18, w, 245);
        }
        AppId::Messages => {
            cv.round_rect(68, 82, 120, 88, 26, w, 240);
            cv.tri_shape(104, 164, 104, 198, 146, 168, w, 240);
            cv.circle(104, 126, 8, rgb(40, 168, 78), 255);
            cv.circle(128, 126, 8, rgb(40, 168, 78), 255);
            cv.circle(152, 126, 8, rgb(40, 168, 78), 255);
        }
        AppId::Security => {
            cv.round_rect(84, 66, 88, 100, 28, w, 255);
            cv.round_rect(84, 110, 88, 56, 20, w, 255);
            cv.tri_shape(84, 146, 172, 146, 128, 208, w, 255);
            cv.thick_line(102, 126, 122, 148, 8, rgb(22, 92, 64), 255);
            cv.thick_line(122, 148, 158, 100, 8, rgb(22, 92, 64), 255);
            cv.circle(128, 96, 9, rgb(255, 255, 255), 70);
        }
    }
}

trait TriShape {
    fn tri_shape(&self, ax: i32, ay: i32, bx: i32, by: i32, cx: i32, cy: i32, c: u32, a: u8);
}

impl TriShape for Canvas {
    fn tri_shape(&self, ax: i32, ay: i32, bx: i32, by: i32, cx: i32, cy: i32, c: u32, a: u8) {
        let minx = ax.min(bx).min(cx);
        let maxx = ax.max(bx).max(cx);
        let miny = ay.min(by).min(cy);
        let maxy = ay.max(by).max(cy);
        let mut y = miny;
        while y <= maxy {
            let mut x = minx;
            while x <= maxx {
                let d1 = (x - bx) * (ay - by) - (ax - bx) * (y - by);
                let d2 = (x - cx) * (by - cy) - (bx - cx) * (y - cy);
                let d3 = (x - ax) * (cy - ay) - (cx - ax) * (y - ay);
                let neg = d1 < 0 || d2 < 0 || d3 < 0;
                let pos = d1 > 0 || d2 > 0 || d3 > 0;
                if !(neg && pos) {
                    self.blend(x, y, c, a);
                }
                x += 1;
            }
            y += 1;
        }
    }
}

unsafe fn build(app: AppId) {
    let hic = &mut *core::ptr::addr_of_mut!(HIC);
    let cv = Canvas { p: hic.as_mut_ptr(), w: ICON_SRC, h: ICON_SRC };
    cv.clear();
    let (top, bot) = app.colors();
    cv.round_rect(20, 20, 216, 216, 60, top, 255);
    cv.vgrad(26, 26, 204, 204, top, bot);
    cv.round_rect(20, 20, 216, 216, 60, top, 0);
    cv.round_rect(20, 20, 216, 216, 60, 0, 0);
    draw_symbol(&cv, app);
    let cache = &mut *core::ptr::addr_of_mut!(ICON64);
    let out = &mut cache[app as usize];
    let mut ty = 0;
    while ty < ICON_SIZE {
        let mut tx = 0;
        while tx < ICON_SIZE {
            let mut sa: u32 = 0;
            let mut sr: u32 = 0;
            let mut sg: u32 = 0;
            let mut sb: u32 = 0;
            let mut sy = 0;
            while sy < 4 {
                let mut sx = 0;
                while sx < 4 {
                    let px = tx * 4 + sx;
                    let py = ty * 4 + sy;
                    let i = (py * ICON_SRC + px) * 4;
                    let a = hic[i + 3] as u32;
                    sa += a;
                    sr += hic[i] as u32 * a;
                    sg += hic[i + 1] as u32 * a;
                    sb += hic[i + 2] as u32 * a;
                    sx += 1;
                }
                sy += 1;
            }
            let o = (ty * ICON_SIZE + tx) * 4;
            if sa > 0 {
                out[o] = (sr / sa) as u8;
                out[o + 1] = (sg / sa) as u8;
                out[o + 2] = (sb / sa) as u8;
                out[o + 3] = (sa / 16) as u8;
            } else {
                out[o] = 0;
                out[o + 1] = 0;
                out[o + 2] = 0;
                out[o + 3] = 0;
            }
            tx += 1;
        }
        ty += 1;
    }
}

pub unsafe fn icon(app: AppId) -> &'static [u8] {
    let i = app as usize;
    let ready = &mut *core::ptr::addr_of_mut!(READY);
    if !ready[i] {
        build(app);
        ready[i] = true;
    }
    let base = core::ptr::addr_of!(ICON64) as *const u8;
    core::slice::from_raw_parts(base.add(i * ICON_BYTES), ICON_BYTES)
}

pub unsafe fn preload() {
    let mut i = 0;
    while i < APP_COUNT {
        let _ = icon(AppId::from_index(i));
        i += 1;
    }
}