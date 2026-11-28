use crate::efi::*;
use crate::font;

#[inline]
pub const fn rgb(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub const BLACK: u32 = 0x000000;
pub const WHITE: u32 = 0xffffff;

#[inline]
pub fn lerp(a: u32, b: u32, t: u32) -> u32 {
    let t = if t > 255 { 255 } else { t };
    let ar = (a >> 16) & 0xff;
    let ag = (a >> 8) & 0xff;
    let ab = a & 0xff;
    let br = (b >> 16) & 0xff;
    let bg = (b >> 8) & 0xff;
    let bb = b & 0xff;
    let r = (ar * (255 - t) + br * t) / 255;
    let g = (ag * (255 - t) + bg * t) / 255;
    let bl = (ab * (255 - t) + bb * t) / 255;
    (r << 16) | (g << 8) | bl
}

#[repr(C)]
pub struct PixelBitmask {
    pub r: u32,
    pub g: u32,
    pub b: u32,
    pub reserved: u32,
}

#[repr(C)]
pub struct GopModeInfo {
    pub version: u32,
    pub hres: u32,
    pub vres: u32,
    pub pixel_format: u32,
    pub pixel_info: [PixelBitmask; 4],
    pub pixels_per_scanline: u32,
}

#[repr(C)]
pub struct GopMode {
    pub max_mode: u32,
    pub mode: u32,
    pub info: *mut GopModeInfo,
    pub size_of_info: usize,
    pub fb_base: u64,
    pub fb_size: usize,
}

#[repr(C)]
pub struct Gop {
    pub query_mode: usize,
    pub set_mode: usize,
    pub blt: usize,
    pub mode: *mut GopMode,
}

pub struct Gfx {
    pub fb: *mut u32,
    pub w: usize,
    pub h: usize,
    pub stride: usize,
    pub swap: bool,
    pub ok: bool,
    pub phys_fb: *mut u32,
    pub phys_w: usize,
    pub phys_h: usize,
    pub phys_stride: usize,
    pub scale: u32,
    pub direct: bool,
}

static mut SCR_W: i32 = 1024;
static mut SCR_H: i32 = 768;
static mut BRIGHTNESS: u8 = 100;

pub fn set_brightness(v: u8) {
    unsafe { BRIGHTNESS = if v > 100 { 100 } else { v }; }
}

pub fn brightness() -> u8 {
    unsafe { BRIGHTNESS }
}

static mut GOP: *mut Gop = core::ptr::null_mut();
static mut LOGICAL: [u32; 1920 * 1080] = [0; 1920 * 1080];

unsafe fn make_gfx(phys_fb: *mut u32, pw: usize, ph: usize, ps: usize, swap: bool, ok: bool, scale: u32) -> Gfx {
    SCR_W = pw as i32;
    SCR_H = ph as i32;
    let sc = if scale < 100 { 100 } else if scale > 150 { 150 } else { scale };
    if sc <= 100 || pw == 0 || ph == 0 {
        Gfx {
            fb: phys_fb, w: pw, h: ph, stride: ps, swap, ok,
            phys_fb, phys_w: pw, phys_h: ph, phys_stride: ps, scale: 100, direct: true,
        }
    } else {
        let lw = pw * 100 / sc as usize;
        let lh = ph * 100 / sc as usize;
        Gfx {
            fb: core::ptr::addr_of_mut!(LOGICAL) as *mut u32,
            w: lw, h: lh, stride: lw, swap, ok,
            phys_fb, phys_w: pw, phys_h: ph, phys_stride: ps, scale: sc, direct: false,
        }
    }
}

pub unsafe fn set_resolution(w: usize, h: usize, scale: u32) -> Option<Gfx> {
    if GOP.is_null() {
        return None;
    }
    let m0 = (*GOP).mode;
    if m0.is_null() { return None; }
    let max = (*m0).max_mode;
    let mut chosen: i32 = -1;
    let mut m = 0u32;
    while m < max {
        let f: extern "efiapi" fn(*mut Gop, u32, *mut *mut GopModeInfo, *mut usize) -> Status =
            core::mem::transmute((*GOP).query_mode);
        let mut info: *mut GopModeInfo = core::ptr::null_mut();
        let mut sz: usize = 0;
        if f(GOP, m, &mut info, &mut sz) == EFI_SUCCESS && !info.is_null() {
            let pf = (*info).pixel_format;
            if (pf == 0 || pf == 1) && (*info).hres as usize == w && (*info).vres as usize == h {
                chosen = m as i32;
                break;
            }
        }
        m += 1;
    }
    if chosen < 0 {
        return None;
    }
    let f: extern "efiapi" fn(*mut Gop, u32) -> Status = core::mem::transmute((*GOP).set_mode);
    if !status_ok(f(GOP, chosen as u32)) {
        return None;
    }
    let mode = (*GOP).mode;
    if mode.is_null() || (*mode).info.is_null() {
        return None;
    }
    let info = (*mode).info;
    let pw = (*info).hres as usize;
    let ph = (*info).vres as usize;
    let raw = (*info).pixels_per_scanline as usize;
    let ps = if raw < pw || raw > 16384 { pw } else { raw };
    let pf = (*info).pixel_format;
    let fb = (*mode).fb_base as *mut u32;
    Some(make_gfx(fb, pw, ph, ps, pf == 0, !fb.is_null() && pw > 0 && ph > 0, scale))
}

pub unsafe fn rescale(g: &Gfx, scale: u32) -> Gfx {
    make_gfx(g.phys_fb, g.phys_w, g.phys_h, g.phys_stride, g.swap, g.ok, scale)
}

pub unsafe fn apply_display(g: Gfx) -> Gfx {
    let w = crate::display::width() as usize;
    let h = crate::display::height() as usize;
    let sc = crate::display::scale();
    let mut ng = g;
    if w != ng.phys_w || h != ng.phys_h {
        if let Some(n) = set_resolution(w, h, sc) {
            ng = n;
        }
    }
    if sc != ng.scale {
        ng = rescale(&ng, sc);
    }
    ng
}

impl Gfx {
    pub fn screen_w() -> i32 {
        unsafe { SCR_W }
    }

    pub fn screen_h() -> i32 {
        unsafe { SCR_H }
    }
    pub fn empty() -> Gfx {
        Gfx {
            fb: core::ptr::null_mut(), w: 0, h: 0, stride: 0, swap: false, ok: false,
            phys_fb: core::ptr::null_mut(), phys_w: 0, phys_h: 0, phys_stride: 0,
            scale: 100, direct: true,
        }
    }

    pub unsafe fn from_device(fb: *mut u32, w: usize, h: usize, stride: usize, swap: bool, ok: bool) -> Gfx {
        SCR_W = w as i32;
        SCR_H = h as i32;
        Gfx {
            fb, w, h, stride, swap, ok,
            phys_fb: fb, phys_w: w, phys_h: h, phys_stride: stride,
            scale: 100, direct: true,
        }
    }

    pub fn flush(&self) {
        if !self.ok {
            return;
        }
        unsafe {
            if !self.direct && self.phys_w > 0 && self.phys_h > 0 && self.w > 0 && self.h > 0 {
                let src = core::ptr::addr_of!(LOGICAL) as *const u32;
                let mut y = 0usize;
                while y < self.phys_h {
                    let sy = y * self.h / self.phys_h;
                    let mut x = 0usize;
                    while x < self.phys_w {
                        let sx = x * self.w / self.phys_w;
                        *self.phys_fb.add(y * self.phys_stride + x) = *src.add(sy * self.w + sx);
                        x += 1;
                    }
                    y += 1;
                }
            }
            crate::video::flush();
        }
    }

    pub unsafe fn init() -> Gfx {
        let gop = bs_locate_protocol(&GUID_GOP) as *mut Gop;
        if gop.is_null() {
            debug_print("[gfx] no GOP");
            return Gfx::empty();
        }
        GOP = gop;
        let mode = (*gop).mode;
        if mode.is_null() {
            return Gfx::empty();
        }
        let max = (*mode).max_mode;
        let mut chosen: i32 = -1;
        let mut want_w = 1024usize;
        let mut want_h = 768usize;
        let mut m: u32 = 0;
        while m < max {
            let f: extern "efiapi" fn(*mut Gop, u32, *mut *mut GopModeInfo, *mut usize) -> Status =
                core::mem::transmute((*gop).query_mode);
            let mut info: *mut GopModeInfo = core::ptr::null_mut();
            let mut sz: usize = 0;
            if f(gop, m, &mut info, &mut sz) == EFI_SUCCESS && !info.is_null() {
                let w = (*info).hres as usize;
                let h = (*info).vres as usize;
                let pf = (*info).pixel_format;
                if (pf == 0 || pf == 1) && w == want_w && h == want_h {
                    chosen = m as i32;
                    break;
                }
                if (pf == 0 || pf == 1) && w >= 800 && h >= 600 && w <= 1920 && h <= 1080 {
                    if chosen < 0 || (w * h) < want_w * want_h {
                        want_w = w;
                        want_h = h;
                        chosen = m as i32;
                    }
                }
            }
            m += 1;
        }
        if chosen >= 0 {
            let f: extern "efiapi" fn(*mut Gop, u32) -> Status = core::mem::transmute((*gop).set_mode);
            let st = f(gop, chosen as u32);
            if !status_ok(st) {
                debug_print("[gfx] set_mode failed");
            }
        }
        let mode = (*gop).mode;
        if mode.is_null() || (*mode).info.is_null() {
            return Gfx::empty();
        }
        let info = (*mode).info;
        let w = (*info).hres as usize;
        let h = (*info).vres as usize;
        let raw_stride = (*info).pixels_per_scanline as usize;
        let stride = if raw_stride < w || raw_stride > 16384 { w } else { raw_stride };
        let pf = (*info).pixel_format;
        let fb = (*mode).fb_base as *mut u32;
        debug_print("[gfx] framebuffer ready");
        SCR_W = w as i32;
        SCR_H = h as i32;
        make_gfx(fb, w, h, stride, pf == 0, !fb.is_null() && w > 0 && h > 0, 100)
    }

    #[inline(always)]
    fn raw_index(&self, x: usize, y: usize) -> usize {
        y * self.stride + x
    }

    #[inline(always)]
    fn pack(&self, c: u32) -> u32 {
        if self.swap {
            ((c & 0xff) << 16) | (c & 0x00ff00) | ((c >> 16) & 0xff)
        } else {
            c
        }
    }

    #[inline(always)]
    fn unpack(&self, c: u32) -> u32 {
        if self.swap {
            ((c & 0xff) << 16) | (c & 0x00ff00) | ((c >> 16) & 0xff)
        } else {
            c
        }
    }

    #[inline(always)]
    fn dim_color(&self, c: u32) -> u32 {
        let b = unsafe { BRIGHTNESS };
        if b >= 100 { return c; }
        let r = ((c >> 16) & 0xff) * b as u32 / 100;
        let g = ((c >> 8) & 0xff) * b as u32 / 100;
        let bl = (c & 0xff) * b as u32 / 100;
        (r << 16) | (g << 8) | bl
    }

    #[inline(always)]
    pub fn put(&self, x: i32, y: i32, c: u32, a: u8) {
        if a == 0 || x < 0 || y < 0 { return; }
        let c = self.dim_color(c);
        let ux = x as usize;
        let uy = y as usize;
        if ux >= self.w || uy >= self.h { return; }
        let idx = self.raw_index(ux, uy);
        unsafe {
            let p = self.fb.add(idx);
            if a >= 250 {
                p.write(self.pack(c));
            } else {
                let dst = self.unpack(p.read());
                let t = a as u32;
                let r = (((c >> 16) & 0xff) * t + ((dst >> 16) & 0xff) * (255 - t)) / 255;
                let g = (((c >> 8) & 0xff) * t + ((dst >> 8) & 0xff) * (255 - t)) / 255;
                let b = ((c & 0xff) * t + (dst & 0xff) * (255 - t)) / 255;
                p.write(self.pack((r << 16) | (g << 8) | b));
            }
        }
    }

    pub fn clear(&self, c: u32) {
        if !self.ok { return; }
        let packed = self.pack(c);
        unsafe {
            let n = self.stride * self.h;
            if packed == 0 {
                bs_set_mem(self.fb as *mut core::ffi::c_void, n * 4, 0);
                return;
            }
            let s = core::slice::from_raw_parts_mut(self.fb, n);
            let mut i = 0;
            while i < s.len() {
                s[i] = packed;
                i += 1;
            }
        }
    }

    pub fn rect(&self, x: i32, y: i32, w: i32, h: i32, c: u32, a: u8) {
        if !self.ok || w <= 0 || h <= 0 || a == 0 { return; }
        let x0 = if x < 0 { 0 } else { x };
        let y0 = if y < 0 { 0 } else { y };
        let x1 = if x + w > self.w as i32 { self.w as i32 } else { x + w };
        let y1 = if y + h > self.h as i32 { self.h as i32 } else { y + h };
        if x1 <= x0 || y1 <= y0 { return; }
        if a >= 250 {
            let packed = self.pack(self.dim_color(c));
            let mut yy = y0;
            while yy < y1 {
                unsafe {
                    let s = core::slice::from_raw_parts_mut(
                        self.fb.add(self.raw_index(x0 as usize, yy as usize)),
                        (x1 - x0) as usize,
                    );
                    let mut i = 0;
                    while i < s.len() {
                        s[i] = packed;
                        i += 1;
                    }
                }
                yy += 1;
            }
        } else {
            let mut yy = y0;
            while yy < y1 {
                let mut xx = x0;
                while xx < x1 {
                    self.put(xx, yy, c, a);
                    xx += 1;
                }
                yy += 1;
            }
        }
    }

    pub fn vgrad(&self, x: i32, y: i32, w: i32, h: i32, top: u32, bot: u32) {
        if !self.ok || w <= 0 || h <= 0 { return; }
        let mut i = 0;
        while i < h {
            let t = (i * 255 / if h > 1 { h - 1 } else { 1 }) as u32;
            self.rect(x, y + i, w, 1, lerp(top, bot, t), 255);
            i += 1;
        }
    }

    pub fn round_rect(&self, x: i32, y: i32, w: i32, h: i32, r: i32, c: u32, a: u8) {
        if w <= 0 || h <= 0 { return; }
        let r = if r * 2 > w { w / 2 } else { r };
        let r = if r * 2 > h { h / 2 } else { r };
        if r <= 0 {
            self.rect(x, y, w, h, c, a);
            return;
        }
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
                let d = sqrtf(dx * dx + dy * dy);
                let cov = rf + 0.5 - d;
                if cov > 0.0 {
                    let aa = if cov >= 1.0 { a } else { (a as f32 * cov) as u8 };
                    self.put(x + cx, y + cy, c, aa);
                    self.put(x + w - 1 - cx, y + cy, c, aa);
                    self.put(x + cx, y + h - 1 - cy, c, aa);
                    self.put(x + w - 1 - cx, y + h - 1 - cy, c, aa);
                }
                cx += 1;
            }
            cy += 1;
        }
    }

    pub fn round_rect_outline(&self, x: i32, y: i32, w: i32, h: i32, r: i32, c: u32, a: u8) {
        let mut i = 0;
        while i < w {
            self.put(x + i, y, c, a);
            self.put(x + i, y + h - 1, c, a);
            i += 1;
        }
        let mut j = 0;
        while j < h {
            self.put(x, y + j, c, a);
            self.put(x + w - 1, y + j, c, a);
            j += 1;
        }
        let rf = r as f32;
        let mut cy = 0;
        while cy < r {
            let mut cx = 0;
            while cx < r {
                let dx = (r - cx) as f32 - 0.5;
                let dy = (r - cy) as f32 - 0.5;
                let d = sqrtf(dx * dx + dy * dy);
                let cov = 1.0 - fabsf(d - rf);
                if cov > 0.0 {
                    let aa = if cov >= 1.0 { a } else { (a as f32 * cov) as u8 };
                    self.put(x + cx, y + cy, c, aa);
                    self.put(x + w - 1 - cx, y + cy, c, aa);
                    self.put(x + cx, y + h - 1 - cy, c, aa);
                    self.put(x + w - 1 - cx, y + h - 1 - cy, c, aa);
                }
                cx += 1;
            }
            cy += 1;
        }
    }

    pub fn circle(&self, cx: i32, cy: i32, r: i32, c: u32, a: u8) {
        let r2 = r * r;
        let mut y = -r;
        while y <= r {
            let mut x = -r;
            while x <= r {
                let d2 = x * x + y * y;
                if d2 <= r2 {
                    let edge = (r2 - d2) as f32;
                    let cov = if edge > r as f32 * 2.0 { 1.0 } else { edge / (r as f32 * 2.0) };
                    let aa = if cov >= 1.0 { a } else { (a as f32 * cov) as u8 };
                    self.put(cx + x, cy + y, c, aa);
                }
                x += 1;
            }
            y += 1;
        }
    }

    pub fn ring(&self, cx: i32, cy: i32, r: i32, thick: i32, c: u32, a: u8) {
        let ro = r;
        let ri = r - thick;
        let mut y = -ro;
        while y <= ro {
            let mut x = -ro;
            while x <= ro {
                let d2 = x * x + y * y;
                if d2 <= ro * ro && d2 >= ri * ri {
                    self.put(cx + x, cy + y, c, a);
                }
                x += 1;
            }
            y += 1;
        }
    }

    pub fn line(&self, x0: i32, y0: i32, x1: i32, y1: i32, c: u32, a: u8) {
        let mut x = x0;
        let mut y = y0;
        let dx = if x1 > x0 { x1 - x0 } else { x0 - x1 };
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = if y1 > y0 { y0 - y1 } else { y1 - y0 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.put(x, y, c, a);
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

    pub fn tri(&self, ax: i32, ay: i32, bx: i32, by: i32, cx: i32, cy: i32, c: u32, a: u8) {
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
                let neg = (d1 < 0) || (d2 < 0) || (d3 < 0);
                let pos = (d1 > 0) || (d2 > 0) || (d3 > 0);
                if !(neg && pos) {
                    self.put(x, y, c, a);
                }
                x += 1;
            }
            y += 1;
        }
    }

    pub fn glyph(&self, x: i32, y: i32, ch: u8, c: u32, scale: i32, alpha: u8) {
        let idx = if ch >= 32 && ch <= 126 { (ch - 32) as usize } else { 0 };
        let bpg = font::FONT_8X16_BPG;
        let per_row = 4usize;
        let glyph = &font::FONT_8X16[idx * bpg..idx * bpg + bpg];
        let mut gy = 0;
        while gy < 16 {
            let mut gx = 0;
            while gx < 8 {
                let byte = glyph[gy * per_row + gx / 2];
                let nib = if gx % 2 == 0 { byte >> 4 } else { byte & 0x0f };
                if nib > 0 {
                    let a = ((nib as u32 * 17 * alpha as u32) / 255) as u8;
                    if scale == 1 {
                        self.put(x + gx as i32, y + gy as i32, c, a);
                    } else {
                        self.rect(x + gx as i32 * scale, y + gy as i32 * scale, scale, scale, c, a);
                    }
                }
                gx += 1;
            }
            gy += 1;
        }
    }

    pub fn text(&self, x: i32, y: i32, s: &str, c: u32, scale: i32) {
        self.text_a(x, y, s, c, scale, 255);
    }

    pub fn text_a(&self, x: i32, y: i32, s: &str, c: u32, scale: i32, alpha: u8) {
        let b = s.as_bytes();
        let mut i = 0usize;
        let mut px = x;
        while i < b.len() {
            let cp = next_utf8(b, &mut i);
            if cp < 128 {
                self.glyph(px, y, cp as u8, c, scale, alpha);
                px += 8 * scale;
            } else {
                self.cjk_glyph(px, y, cp, c, scale, alpha);
                px += 16 * scale;
            }
        }
    }

    pub fn cjk_glyph(&self, x: i32, y: i32, cp: u32, c: u32, scale: i32, alpha: u8) {
        let mut lo = 0usize;
        let mut hi = crate::cjk::CJK_COUNT;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if crate::cjk::CJK_CP[mid] < cp {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo >= crate::cjk::CJK_COUNT || crate::cjk::CJK_CP[lo] != cp {
            let s = if scale < 1 { 1 } else { scale };
            self.round_rect_outline(x + 1, y + 1, 16 * s - 2, 16 * s - 2, 3, c, alpha / 2);
            return;
        }
        let glyph = &crate::cjk::CJK_BITS[lo * crate::cjk::CJK_BPG..(lo + 1) * crate::cjk::CJK_BPG];
        let mut gy = 0;
        while gy < 16 {
            let mut gx = 0;
            while gx < 16 {
                let byte = glyph[gy * 8 + gx / 2];
                let nib = if gx % 2 == 0 { byte >> 4 } else { byte & 0x0f };
                if nib > 0 {
                    let a = ((nib as u32 * 17 * alpha as u32) / 255) as u8;
                    if scale == 1 {
                        self.put(x + gx as i32, y + gy as i32, c, a);
                    } else {
                        self.rect(x + gx as i32 * scale, y + gy as i32 * scale, scale, scale, c, a);
                    }
                }
                gx += 1;
            }
            gy += 1;
        }
    }

    pub fn text_center(&self, cx: i32, y: i32, s: &str, c: u32, scale: i32) {
        let w = text_width(s, scale);
        self.text(cx - w / 2, y, s, c, scale);
    }

    pub fn big_glyph(&self, x: i32, y: i32, ch: u8, c: u32, alpha: u8) {
        let idx = if ch >= 32 && ch <= 126 { (ch - 32) as usize } else { 0 };
        let bpg = font::FONT_16X32_BPG;
        let per_row = 8usize;
        let glyph = &font::FONT_16X32[idx * bpg..idx * bpg + bpg];
        let mut gy = 0;
        while gy < 32 {
            let mut gx = 0;
            while gx < 16 {
                let byte = glyph[gy * per_row + gx / 2];
                let nib = if gx % 2 == 0 { byte >> 4 } else { byte & 0x0f };
                if nib > 0 {
                    let a = ((nib as u32 * 17 * alpha as u32) / 255) as u8;
                    self.put(x + gx as i32, y + gy as i32, c, a);
                }
                gx += 1;
            }
            gy += 1;
        }
    }

    pub fn big_text(&self, x: i32, y: i32, s: &str, c: u32, alpha: u8) {
        let mut px = x;
        for b in s.bytes() {
            self.big_glyph(px, y, b, c, alpha);
            px += 16;
        }
    }

    pub fn dim(&self, alpha: u8) {
        if !self.ok || alpha == 0 { return; }
        let f = 255 - alpha as u32;
        unsafe {
            let s = core::slice::from_raw_parts_mut(self.fb, self.stride * self.h);
            let mut i = 0;
            while i < s.len() {
                let c = self.unpack(s[i]);
                let r = ((c >> 16) & 0xff) * f / 255;
                let g = ((c >> 8) & 0xff) * f / 255;
                let b = (c & 0xff) * f / 255;
                s[i] = self.pack((r << 16) | (g << 8) | b);
                i += 1;
            }
        }
    }

    pub fn blit_rgba(&self, x: i32, y: i32, dw: i32, dh: i32, src: &[u8], sw: i32, sh: i32) {
        if dw <= 0 || dh <= 0 || sw <= 0 || sh <= 0 { return; }
        let mut dy = 0;
        while dy < dh {
            let sy = (dy * sh / dh).min(sh - 1);
            let mut dx = 0;
            while dx < dw {
                let sx = (dx * sw / dw).min(sw - 1);
                let si = ((sy * sw + sx) * 4) as usize;
                let a = src[si + 3];
                if a > 0 {
                    let c = ((src[si] as u32) << 16) | ((src[si + 1] as u32) << 8) | src[si + 2] as u32;
                    self.put(x + dx, y + dy, c, a);
                }
                dx += 1;
            }
            dy += 1;
        }
    }

    pub fn snapshot(&self, dst: &mut [u32]) {
        if !self.ok { return; }
        let n = (self.stride * self.h).min(dst.len());
        unsafe {
            let s = core::slice::from_raw_parts(self.fb, self.stride * self.h);
            let mut i = 0;
            while i < n {
                dst[i] = self.unpack(s[i]);
                i += 1;
            }
        }
    }
}

pub fn text_width(s: &str, scale: i32) -> i32 {
    let b = s.as_bytes();
    let mut i = 0usize;
    let mut w = 0i32;
    while i < b.len() {
        let cp = next_utf8(b, &mut i);
        w += if cp < 128 { 8 * scale } else { 16 * scale };
    }
    w
}

pub fn next_utf8(b: &[u8], i: &mut usize) -> u32 {
    if *i >= b.len() { return 0; }
    let c = b[*i];
    if c < 0x80 {
        *i += 1;
        return c as u32;
    }
    let (n, mut cp) = if c & 0xe0 == 0xc0 {
        (1usize, (c & 0x1f) as u32)
    } else if c & 0xf0 == 0xe0 {
        (2usize, (c & 0x0f) as u32)
    } else if c & 0xf8 == 0xf0 {
        (3usize, (c & 0x07) as u32)
    } else {
        *i += 1;
        return 0xfffd;
    };
    *i += 1;
    let mut k = 0;
    while k < n {
        if *i < b.len() && b[*i] & 0xc0 == 0x80 {
            cp = (cp << 6) | (b[*i] & 0x3f) as u32;
            *i += 1;
        } else {
            break;
        }
        k += 1;
    }
    cp
}

pub fn sqrtf(v: f32) -> f32 {
    if v <= 0.0 { return 0.0; }
    let mut x = v;
    let mut i = 0;
    while i < 18 {
        x = 0.5 * (x + v / x);
        i += 1;
    }
    x
}

pub fn fabsf(v: f32) -> f32 {
    if v < 0.0 { -v } else { v }
}

pub fn sinf(v: f32) -> f32 {
    let mut x = v;
    while x > 3.14159265 { x -= 6.2831853; }
    while x < -3.14159265 { x += 6.2831853; }
    let x2 = x * x;
    x * (1.0 - x2 / 6.0 + x2 * x2 / 120.0 - x2 * x2 * x2 / 5040.0)
}

pub fn cosf(v: f32) -> f32 {
    sinf(v + 1.5707963)
}