use crate::gfx::{cosf, lerp, rgb, sinf, Gfx};

const STARS: usize = 140;
const FRAMES_TOTAL: u32 = 92;

#[derive(Clone, Copy)]
struct Star {
    x: f32,
    y: f32,
    angle: f32,
    radius: f32,
    speed: f32,
    size: i32,
    tint: u32,
    px: i32,
    py: i32,
    alive: bool,
}

pub struct BootAnim {
    frame: u32,
    stars: [Star; STARS],
    cx: i32,
    cy: i32,
    seed: u32,
    bg_top: u32,
    bg_bot: u32,
}

fn rnd(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed
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
        let cx = g.w as i32 / 2;
        let cy = g.h as i32 / 2 - 20;
        let bg_top = rgb(9, 13, 34);
        let bg_bot = rgb(6, 5, 16);
        background(g, bg_top, bg_bot);
        let mut seed = 0x1234567u32;
        let mut stars = [Star {
            x: 0.0,
            y: 0.0,
            angle: 0.0,
            radius: 0.0,
            speed: 0.0,
            size: 2,
            tint: 0,
            px: -100,
            py: -100,
            alive: false,
        }; STARS];
        let mut i = 0;
        while i < STARS {
            let ang = (rnd(&mut seed) % 6283) as f32 / 1000.0;
            let rad = 40.0 + (rnd(&mut seed) % 1000) as f32 * 0.55;
            let spd = 0.006 + (rnd(&mut seed) % 40) as f32 * 0.0006;
            let tint = match rnd(&mut seed) % 3 {
                0 => rgb(120, 220, 255),
                1 => rgb(190, 170, 255),
                _ => rgb(255, 255, 255),
            };
            stars[i] = Star {
                x: cx as f32 + cosf(ang) * rad,
                y: cy as f32 + sinf(ang) * rad * 0.62,
                angle: ang,
                radius: rad,
                speed: spd,
                size: 1 + (rnd(&mut seed) % 2) as i32,
                tint,
                px: -100,
                py: -100,
                alive: true,
            };
            i += 1;
        }
        BootAnim { frame: 0, stars, cx, cy, seed, bg_top, bg_bot }
    }

    fn erase(&self, g: &Gfx, s: &Star) {
        if s.px < -10 { return; }
        let t = ((s.py.max(0) as u32) * 255 / g.h.max(1) as u32) as u32;
        let bg = lerp(self.bg_top, self.bg_bot, t);
        let r = s.size + 4;
        g.circle(s.px, s.py, r, bg, 255);
        let tw = s.px - 6;
        if tw >= 0 {
            g.rect(tw, s.py - 1, 12, 1, bg, 255);
            g.rect(s.px - 1, s.py - 6, 1, 12, bg, 255);
        }
    }

    fn draw_star(&self, g: &Gfx, s: &Star, fade: u8) {
        g.circle(s.px, s.py, s.size + 3, s.tint, fade / 6);
        g.circle(s.px, s.py, s.size, s.tint, fade);
        g.rect(s.px - 5, s.py, 11, 1, s.tint, fade / 3);
        g.rect(s.px, s.py - 5, 1, 11, s.tint, fade / 3);
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    pub fn step(&mut self, g: &Gfx) -> bool {
        self.frame += 1;
        let f = self.frame;

        let mut i = 0;
        while i < STARS {
            let st = self.stars[i];
            self.erase(g, &st);
            i += 1;
        }

        if f <= 46 {
            let t = f as f32 / 46.0;
            let mut i = 0;
            while i < STARS {
                let mut s = self.stars[i];
                s.angle += s.speed * (1.0 + t * 2.2);
                s.radius *= 1.0 - 0.016 * t;
                if s.radius < 6.0 { s.radius = 6.0; }
                s.x = self.cx as f32 + cosf(s.angle) * s.radius;
                s.y = self.cy as f32 + sinf(s.angle) * s.radius * 0.62;
                s.px = s.x as i32;
                s.py = s.y as i32;
                self.stars[i] = s;
                let fade = 90 + (t * 120.0) as u32;
                self.draw_star(g, &s, if fade > 255 { 255 } else { fade as u8 });
                i += 1;
            }
            let t2 = f as u32;
            if t2 > 30 {
                let r = ((t2 - 30) * 7) as i32;
                g.ring(self.cx, self.cy, r.min(300), 2, rgb(110, 190, 255), (140 - r / 3).max(10) as u8);
            }
        } else if f <= 58 {
            let t = (f - 46) as f32 / 12.0;
            let mut i = 0;
            while i < STARS {
                let mut s = self.stars[i];
                s.angle += s.speed * 3.0;
                s.radius *= 0.97;
                s.x = self.cx as f32 + cosf(s.angle) * s.radius;
                s.y = self.cy as f32 + sinf(s.angle) * s.radius * 0.62;
                s.px = s.x as i32;
                s.py = s.y as i32;
                self.stars[i] = s;
                let fade = (255.0 * (1.0 - t * 0.8)) as u8;
                self.draw_star(g, &s, fade);
                i += 1;
            }
            let r = 70 + (t * 300.0) as i32;
            g.ring(self.cx, self.cy, r, 3, rgb(120, 220, 255), (160.0 * (1.0 - t)) as u8);
            g.ring(self.cx, self.cy, r / 2, 2, rgb(255, 110, 190), (120.0 * (1.0 - t)) as u8);
        } else {
            let mut i = 0;
            while i < STARS {
                let s = self.stars[i];
                self.draw_star(g, &s, 60);
                i += 1;
            }
        }

        if f == 47 {
            let halo = rgb(90, 190, 255);
            g.circle(self.cx, self.cy - 190, 120, halo, 18);
            g.circle(self.cx, self.cy - 190, 78, rgb(120, 220, 255), 26);
        }

        if f >= 48 {
            let mx = self.cx;
            let w = 16 * 5;
            let tw = w * 5;
            let tx = mx - tw / 2;
            let ty = self.cy - 24;
            let bg = self.bg_bot;
            if f == 48 {
                g.rect(tx - 8, ty - 10, tw + 16, 62, bg, 255);
            }
            let a = if f < 62 { ((f - 48) * 18).min(255) as u8 } else { 255 };
            let word = "HYWOS";
            let mut k = 0;
            for ch in word.bytes() {
                let idx = if ch >= 32 && ch <= 126 { (ch - 32) as usize } else { 0 };
                let glyph = &crate::font::FONT_16X32
                    [idx * crate::font::FONT_16X32_BPG..(idx + 1) * crate::font::FONT_16X32_BPG];
                let gx = tx + k * w;
                let mut gy = 0;
                while gy < 32 {
                    let mut gxx = 0;
                    while gxx < 16 {
                        let byte = glyph[gy * 8 + gxx / 2];
                        let nib = if gxx % 2 == 0 { byte >> 4 } else { byte & 0x0f };
                        if nib > 0 {
                            let aa = ((nib as u32 * 17 * a as u32) / 255) as u8;
                            g.rect(gx + gxx as i32 * 5, ty + gy as i32 * 2, 5, 2, rgb(235, 246, 255), aa);
                        }
                        gxx += 1;
                    }
                    gy += 1;
                }
                k += 1;
            }
            if f >= 62 {
                let sa = ((f - 62) * 14).min(255) as u8;
                let st = "星河 · 中文界面 · 64 位";
                let sw = crate::gfx::text_width(st, 1);
                g.text_a(self.cx - sw / 2, ty + 74, st, rgb(150, 210, 255), 1, sa);
            }
        }

        if f > FRAMES_TOTAL - 8 {
            let a = (((f - (FRAMES_TOTAL - 8)) * 32) as u8).min(220);
            g.dim(a);
        }

        self.frame >= FRAMES_TOTAL
    }
}