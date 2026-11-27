use crate::gfx::{lerp, rgb, text_width, Gfx};
use crate::logo::LOGO_RGBA;

const FRAMES_TOTAL: u32 = 72;
const LOGO_W: usize = 176;
const STAR_N: usize = 130;

static mut LOGO_BIG: [u32; LOGO_W * LOGO_W] = [0; LOGO_W * LOGO_W];

pub struct BootAnim {
    frame: u32,
    w: i32,
    h: i32,
    top: u32,
    bot: u32,
}

fn rnd(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed
}

fn build_logo() {
    let src = &LOGO_RGBA;
    let dst = unsafe { &mut *core::ptr::addr_of_mut!(LOGO_BIG) };
    let mut y = 0usize;
    while y < LOGO_W {
        let mut x = 0usize;
        while x < LOGO_W {
            let sx = x * 128 / LOGO_W;
            let sy = y * 128 / LOGO_W;
            let i = (sy * 128 + sx) * 4;
            let a = src[i + 3] as u32;
            let c = ((src[i] as u32) << 16) | ((src[i + 1] as u32) << 8) | src[i + 2] as u32;
            dst[y * LOGO_W + x] = (a << 24) | c;
            x += 1;
        }
        y += 1;
    }
}

fn background(g: &Gfx, top: u32, bot: u32) {
    g.vgrad(0, 0, g.w as i32, g.h as i32, top, bot);
    let mut seed = 0x9e3779b9u32;
    let mut i = 0;
    while i < STAR_N {
        let x = (rnd(&mut seed) % g.w as u32) as i32;
        let y = (rnd(&mut seed) % g.h as u32) as i32;
        let b = (rnd(&mut seed) % 60) as u8 + 40;
        let a = if b > 80 { 150 } else { 80 };
        g.put(x, y, rgb(b, b, (b as u32 + 25).min(255) as u8), a);
        i += 1;
    }
}

impl BootAnim {
    pub fn new(g: &Gfx) -> BootAnim {
        build_logo();
        let top = rgb(10, 14, 32);
        let bot = rgb(5, 5, 14);
        background(g, top, bot);
        BootAnim { frame: 0, w: g.w as i32, h: g.h as i32, top, bot }
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    pub fn step(&mut self, g: &Gfx) -> bool {
        self.frame += 1;
        let f = self.frame;
        if f > FRAMES_TOTAL {
            return true;
        }
        background(g, self.top, self.bot);

        let alpha: u8 = if f <= 34 {
            (f as u32 * 255 / 34) as u8
        } else if f <= 52 {
            255
        } else {
            ((FRAMES_TOTAL - f) as u32 * 255 / (FRAMES_TOTAL - 52)) as u8
        };
        let cx = self.w / 2;
        let grow = if f <= 34 { f } else { 34 };
        let sz = (104 + (LOGO_W as u32 - 104) * grow / 34) as i32;
        let top = self.h / 2 - sz / 2 - 24;
        let lx = cx - sz / 2;
        let big = unsafe { &*core::ptr::addr_of!(LOGO_BIG) };
        let mut y = 0i32;
        while y < sz {
            let sy = (y as usize) * LOGO_W / sz as usize;
            let mut x = 0i32;
            while x < sz {
                let sx = (x as usize) * LOGO_W / sz as usize;
                let p = big[sy * LOGO_W + sx];
                let sa = ((p >> 24) & 0xff) as u32;
                if sa > 0 {
                    let a = (sa * alpha as u32 / 255) as u8;
                    if a > 0 {
                        g.put(lx + x, top + y, p & 0xffffff, a);
                    }
                }
                x += 1;
            }
            y += 1;
        }

        let title = "HYWOS";
        let tw = text_width(title, 3);
        let ty = top + sz + 26;
        g.text_a(cx - tw / 2, ty, title, rgb(236, 246, 255), 3, alpha);

        self.frame >= FRAMES_TOTAL
    }
}
