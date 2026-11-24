use crate::gfx::{cosf, lerp, rgb, sinf, Gfx};

const PARTS: usize = 168;
const FRAMES_TOTAL: u32 = 114;

#[derive(Clone, Copy)]
struct Part {
    sx: f32,
    sy: f32,
    ang: f32,
    delay: f32,
    size: i32,
    tint: u32,
    x: f32,
    y: f32,
    px: i32,
    py: i32,
}

pub struct BootAnim {
    frame: u32,
    parts: [Part; PARTS],
    cx: i32,
    cy: i32,
    tx: i32,
    ty: i32,
    bg_top: u32,
    bg_bot: u32,
    w: i32,
    h: i32,
    title_cleared: bool,
}

fn rnd(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed
}

fn ease(t: f32) -> f32 {
    if t <= 0.0 {
        0.0
    } else if t >= 1.0 {
        1.0
    } else {
        t * t * (3.0 - 2.0 * t)
    }
}

fn background(g: &Gfx, top: u32, bot: u32) {
    g.vgrad(0, 0, g.w as i32, g.h as i32, top, bot);
    let mut i = 0;
    let mut seed = 0x9e3779b9u32;
    while i < 420 {
        let x = (rnd(&mut seed) % g.w as u32) as i32;
        let y = (rnd(&mut seed) % g.h as u32) as i32;
        let b = (rnd(&mut seed) % 70) as u8 + 30;
        let a = if b > 80 { 170 } else { 90 };
        g.put(x, y, rgb(b, b, (b as u32 + 30).min(255) as u8), a);
        i += 1;
    }
}

impl BootAnim {
    pub fn new(g: &Gfx) -> BootAnim {
        let w = g.w as i32;
        let h = g.h as i32;
        let cx = w / 2;
        let cy = h / 2 - 26;
        let bg_top = rgb(9, 13, 34);
        let bg_bot = rgb(6, 5, 16);
        background(g, bg_top, bg_bot);
        let tx = cx - 80;
        let ty = cy - 32;
        let mut seed = 0x51a3c7u32;
        let mut parts = [Part {
            sx: 0.0,
            sy: 0.0,
            ang: 0.0,
            delay: 0.0,
            size: 2,
            tint: 0,
            x: 0.0,
            y: 0.0,
            px: -1000,
            py: -1000,
        }; PARTS];
        let mut i = 0;
        while i < PARTS {
            let ang = (rnd(&mut seed) % 6283) as f32 / 1000.0;
            let sx = (rnd(&mut seed) % w as u32) as f32;
            let sy = (rnd(&mut seed) % h as u32) as f32;
            let tint = match rnd(&mut seed) % 4 {
                0 => rgb(120, 220, 255),
                1 => rgb(150, 140, 255),
                2 => rgb(255, 150, 220),
                _ => rgb(255, 255, 255),
            };
            parts[i] = Part {
                sx,
                sy,
                ang,
                delay: (i % 24) as f32 * 0.018,
                size: 1 + (rnd(&mut seed) % 3) as i32,
                tint,
                x: sx,
                y: sy,
                px: -1000,
                py: -1000,
            };
            i += 1;
        }
        BootAnim {
            frame: 0,
            parts,
            cx,
            cy,
            tx,
            ty,
            bg_top,
            bg_bot,
            w,
            h,
            title_cleared: false,
        }
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    fn bg_at(&self, y: i32) -> u32 {
        let t = ((y.max(0) as u32) * 255 / self.h.max(1) as u32) as u32;
        lerp(self.bg_top, self.bg_bot, t)
    }

    fn erase(&self, g: &Gfx, x: i32, y: i32, r: i32) {
        let c = self.bg_at(y);
        g.circle(x, y, r, c, 255);
    }

    fn target(&self, p: &Part, f: u32) -> (f32, f32, u8) {
        if f <= 50 {
            let t = (f as f32 / 50.0) - p.delay;
            let e = ease(t.max(0.0));
            let ang = p.ang + (1.0 - e) * 1.9;
            let rx = 142.0 * (0.86 + 0.14 * e);
            let ry = 104.0 * (0.86 + 0.14 * e);
            let tx = self.cx as f32 + cosf(ang) * rx;
            let ty = self.cy as f32 + sinf(ang) * ry;
            (p.sx + (tx - p.sx) * e, p.sy + (ty - p.sy) * e, 235)
        } else if f <= 72 {
            let t = ease((f - 50) as f32 / 22.0);
            let rx = 142.0 + (11.0 - 142.0) * t;
            let ry = 104.0 + (19.0 - 104.0) * t;
            let ox = self.cx as f32 + ((self.tx as f32 + 112.0) - self.cx as f32) * t;
            let oy = self.cy as f32 + ((self.ty as f32 + 32.0) - self.cy as f32) * t;
            (
                ox + cosf(p.ang) * rx,
                oy + sinf(p.ang) * ry,
                (235.0 - 90.0 * t) as u8,
            )
        } else {
            let ox = self.tx as f32 + 112.0;
            let oy = self.ty as f32 + 32.0;
            (
                ox + cosf(p.ang) * 11.0,
                oy + sinf(p.ang) * 19.0,
                150,
            )
        }
    }

    pub fn step(&mut self, g: &Gfx) -> bool {
        self.frame += 1;
        let f = self.frame;

        if f >= 73 {
            if !self.title_cleared {
                self.title_cleared = true;
            }
            let c0 = self.bg_at(self.ty);
            g.rect(self.tx - 18, self.ty - 14, 196, 96, c0, 255);
        }

        let mut i = 0;
        while i < PARTS {
            let p = self.parts[i];
            if p.px > -900 {
                self.erase(g, p.px, p.py, p.size + 3);
            }
            i += 1;
        }

        let mut i = 0;
        while i < PARTS {
            let mut p = self.parts[i];
            let (x, y, a) = self.target(&p, f);
            p.x = x;
            p.y = y;
            p.px = x as i32;
            p.py = y as i32;
            self.parts[i] = p;
            if a > 0 {
                g.circle(p.px, p.py, p.size + 2, p.tint, a / 8);
                g.circle(p.px, p.py, p.size, p.tint, a);
                if f <= 72 {
                    g.rect(p.px - 4, p.py, 9, 1, p.tint, a / 3);
                    g.rect(p.px, p.py - 4, 1, 9, p.tint, a / 3);
                }
            }
            i += 1;
        }

        if f >= 73 {
            let alpha = |start: u32| -> u8 {
                if f < start {
                    0
                } else {
                    ((f - start) * 22).min(255) as u8
                }
            };
            let c = rgb(236, 246, 255);
            g.big_glyph(self.tx, self.ty, b'H', c, alpha(73));
            g.big_glyph(self.tx + 16, self.ty, b'Y', c, alpha(76));
            g.big_glyph(self.tx + 32, self.ty, b'W', c, alpha(79));
            g.big_glyph(self.tx + 64, self.ty, b'S', c, alpha(82));
        }

        if f >= 100 {
            let a = ((f - 100) * 20).min(255) as u8;
            let st = "星河 · 中文界面 · 64 位";
            let sw = crate::gfx::text_width(st, 2);
            let sy = self.ty + 78;
            g.rect(self.cx - sw / 2 - 6, sy - 4, sw + 12, 40, self.bg_at(sy), 255);
            g.text_a(self.cx - sw / 2, sy, st, rgb(150, 210, 255), 2, a);
        }

        if f > FRAMES_TOTAL - 8 {
            let a = (((f - (FRAMES_TOTAL - 8)) * 32) as u8).min(220);
            g.dim(a);
        }

        self.frame >= FRAMES_TOTAL
    }
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
