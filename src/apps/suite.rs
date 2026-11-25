#![allow(dead_code)]

use crate::efi::*;
use crate::fs::*;
use crate::gfx::*;
use crate::hw;
use crate::icon::{icon, AppId};

const TXT: u32 = rgb(44, 50, 66);
const SUB: u32 = rgb(126, 134, 154);
const BLUE: u32 = rgb(60, 116, 240);
const GREEN: u32 = rgb(30, 170, 96);
const RED: u32 = rgb(200, 70, 64);
const CARD: u32 = rgb(245, 247, 252);
const LINE: u32 = rgb(214, 220, 232);

fn cat(dst: &mut [u8], l: &mut usize, s: &str) {
    for c in s.bytes() {
        if *l < dst.len() { dst[*l] = c; *l += 1; }
    }
}

fn catn(dst: &mut [u8], l: &mut usize, v: u32) {
    if *l < dst.len() { *l += hw::num2(&mut dst[*l..], v, 1); }
}

unsafe fn path_of<'a>(p: &'a [u8], n: usize) -> &'a str {
    core::str::from_utf8_unchecked(&p[..n])
}

fn panel(g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    g.round_rect(x, y, w, h, 12, CARD, 255);
    g.round_rect_outline(x, y, w, h, 12, LINE, 160);
}

fn btn(g: &Gfx, x: i32, y: i32, w: i32, h: i32, label: &str, primary: bool) {
    if primary {
        g.round_rect(x, y, w, h, h / 2, BLUE, 255);
        g.text_center(x + w / 2, y + (h - 16) / 2, label, rgb(255, 255, 255), 1);
    } else {
        g.round_rect(x, y, w, h, h / 2, rgb(228, 233, 243), 255);
        g.text_center(x + w / 2, y + (h - 16) / 2, label, rgb(56, 62, 80), 1);
    }
}

fn bar(g: &Gfx, x: i32, y: i32, w: i32, h: i32, p: u32, c: u32) {
    g.round_rect(x, y, w, h, h / 2, rgb(224, 229, 240), 255);
    let fw = (w - 4) * p.min(100) as i32 / 100;
    if fw > 2 { g.round_rect(x + 2, y + 2, fw, h - 4, (h - 4) / 2, c, 255); }
}

unsafe fn editbox(g: &Gfx, x: i32, y: i32, w: i32, buf: &[u8], n: usize, hint: &str) {
    g.round_rect(x, y, w, 28, 8, rgb(255, 255, 255), 255);
    g.round_rect_outline(x, y, w, 28, 8, LINE, 220);
    if n == 0 {
        g.text(x + 10, y + 6, hint, rgb(160, 168, 184), 1);
    } else {
        let s = core::str::from_utf8_unchecked(&buf[..n]);
        g.text(x + 10, y + 6, s, TXT, 1);
    }
}

unsafe fn status_line(g: &Gfx, x: i32, y: i32, msg: &[u8], n: usize, ok: bool) {
    if n == 0 { return; }
    g.text(x, y, core::str::from_utf8_unchecked(&msg[..n]), if ok { GREEN } else { RED }, 1);
}

pub struct Scan {
    pub mode: u8,
    pub progress: u32,
    pub files: u32,
    pub threats: u32,
    pub log: [[u8; 44]; 6],
    pub logn: usize,
    pub cur: [u8; 40],
    pub cur_len: usize,
    pub total: u32,
    pub lib: u32,
}

impl Scan {
    fn empty() -> Scan {
        Scan {
            mode: 0, progress: 0, files: 0, threats: 0,
            log: [[0; 44]; 6], logn: 0, cur: [0; 40], cur_len: 0, total: 0, lib: 0,
        }
    }
    fn log(&mut self, s: &str) {
        if self.logn >= 6 {
            let mut i = 1;
            while i < 6 { self.log[i - 1] = self.log[i]; i += 1; }
            self.logn = 5;
        }
        let line = &mut self.log[self.logn];
        let mut i = 0;
        for b in s.bytes() { if i < 42 { line[i] = b; i += 1; } }
        self.logn += 1;
    }
}

pub struct Suite {
    pub sel: usize,
    pub edit: [u8; 640],
    pub elen: usize,
    pub editing: bool,
    pub calc_in: [u8; 64],
    pub calc_len: usize,
    pub calc_out: [u8; 64],
    pub calc_olen: usize,
    pub month_off: i32,
    pub sw_run: bool,
    pub sw_base: u32,
    pub sw_at: u32,
    pub zoom: i32,
    pub pan: i32,
    pub idx: usize,
    pub playing: bool,
    pub frame_t: u32,
    pub url: [u8; 96],
    pub url_len: usize,
    pub page: [u8; 1600],
    pub page_len: usize,
    pub msg: [u8; 72],
    pub msg_len: usize,
    pub ok: bool,
    pub scan: Scan,
    pub mon: [u8; 40],
    pub mon_n: usize,
    pub mon_t: u32,
    pub boot_secs: u32,
    pub cam_t: u32,
    pub shoot: bool,
}

impl Suite {
    pub fn empty() -> Suite {
        Suite {
            sel: 0, edit: [0; 640], elen: 0, editing: false,
            calc_in: [0; 64], calc_len: 0, calc_out: [0; 64], calc_olen: 0,
            month_off: 0, sw_run: false, sw_base: 0, sw_at: 0,
            zoom: 2, pan: 0, idx: 0, playing: false, frame_t: 0,
            url: [0; 96], url_len: 0, page: [0; 1600], page_len: 0,
            msg: [0; 72], msg_len: 0, ok: true,
            scan: Scan::empty(), mon: [0; 40], mon_n: 0, mon_t: 0, boot_secs: 0,
            cam_t: 0, shoot: false,
        }
    }

    pub unsafe fn init(&mut self, id: AppId) {
        let e = Suite::empty();
        *self = e;
        seed_home();
        self.boot_secs = hw::uptime_secs();
        match id {
            AppId::Calculator => {
                cat(&mut self.calc_out, &mut self.calc_olen, "0");
            }
            AppId::Security => {
                self.scan.lib = load_db();
                self.scan.log("[安全中心] 实时监控已启用");
                let mut k = 0;
                while k < 8 { self.sample_mon(); k += 1; }
            }
            AppId::Browser => {
                self.load_home();
            }
            AppId::Notes => {
                self.note_open(0);
            }
            _ => {}
        }
        self.msg("就绪", true);
    }

    fn msg(&mut self, s: &str, ok: bool) {
        self.msg_len = 0;
        cat(&mut self.msg, &mut self.msg_len, s);
        self.ok = ok;
    }

    // ---------- 安全中心 ----------
    unsafe fn sample_mon(&mut self) {
        let (total, free) = hw::mem_kb();
        let pct = if total > 0 { (free * 100 / total) as u8 } else { 0 };
        if self.mon_n < 40 {
            self.mon[self.mon_n] = pct;
            self.mon_n += 1;
        } else {
            let mut i = 1;
            while i < 40 { self.mon[i - 1] = self.mon[i]; i += 1; }
            self.mon[39] = pct;
        }
    }

    unsafe fn scan_start(&mut self, full: bool) {
        self.scan.mode = if full { 2 } else { 1 };
        self.scan.progress = 0;
        self.scan.files = 0;
        self.scan.threats = 0;
        self.scan.logn = 0;
        self.scan.cur_len = 0;
        QN = 0;
        QIDX = 0;
        if full {
            walk_add(sys_vol(), "\\", 0);
            if usr_vol() != sys_vol() { walk_add(usr_vol(), "\\", 0); }
        } else {
            add_path(sys_vol(), "\\EFI\\BOOT\\BOOTX64.EFI");
            add_path(sys_vol(), "\\HYWOS\\VERSION");
            add_path(sys_vol(), "\\HYWOS\\INSTALL.TXT");
            add_path(sys_vol(), "\\HYWOS\\SYSTEM\\WELCOME.TXT");
            add_path(usr_vol(), "\\home\\user\\.apps\\_permissions");
            add_path(usr_vol(), "\\updates\\latest.json");
        }
        self.scan.total = QN as u32;
        self.scan.log(if full { "[全盘查杀] 开始扫描" } else { "[快速查杀] 开始扫描" });
        if QN == 0 {
            self.scan.mode = 3;
            self.scan.progress = 100;
            self.scan.log("没有可扫描的文件");
        }
    }

    unsafe fn scan_step(&mut self) {
        if self.scan.mode != 1 && self.scan.mode != 2 { return; }
        if QIDX >= QN {
            self.scan.mode = 3;
            self.scan.progress = 100;
            let mut b = [0u8; 44];
            let mut l = 0;
            cat(&mut b, &mut l, "完成：");
            catn(&mut b, &mut l, self.scan.files);
            cat(&mut b, &mut l, " 个文件，威胁 ");
            catn(&mut b, &mut l, self.scan.threats);
            self.scan.log(core::str::from_utf8_unchecked(&b[..l]));
            return;
        }
        let vol = QVOL[QIDX] as usize;
        let plen = QLEN[QIDX] as usize;
        let p = path_of(&QPATH[QIDX], plen);
        QIDX += 1;
        self.scan.cur_len = 0;
        let mut t = 0usize;
        while t < plen && t < 38 { self.scan.cur[t] = QPATH[QIDX - 1][t]; t += 1; }
        self.scan.cur_len = t;
        if let Some((h, sz)) = hash_file(vol, p) {
            self.scan.files += 1;
            if let Some(name) = db_match(h) {
                self.scan.threats += 1;
                let mut b = [0u8; 44];
                let mut l = 0;
                cat(&mut b, &mut l, "发现 ");
                cat(&mut b, &mut l, name);
                cat(&mut b, &mut l, " (");
                catn(&mut b, &mut l, sz as u32 / 1024 + 1);
                cat(&mut b, &mut l, " KB)");
                self.scan.log(core::str::from_utf8_unchecked(&b[..l]));
            }
        }
        self.scan.progress = QIDX as u32 * 100 / if QN == 0 { 1 } else { QN as u32 };
        self.scan.mode = if self.scan.mode == 1 { 1 } else { 2 };
    }

    unsafe fn security(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        self.scan_step();
        self.mon_t += 1;
        if self.mon_t % 24 == 0 { self.sample_mon(); }
        let top = y + 46;
        let cw = (w - 40) / 4;
        let (total, free) = hw::mem_kb();
        let cards = [
            ("处理器", 0, 0),
            ("内存", 1, 0),
            ("运行时间", 2, 0),
            ("实时防护", 3, 0),
        ];
        let mut i = 0;
        while i < 4 {
            let cx = x + 14 + i as i32 * (cw + 6);
            panel(g, cx, top, cw, 70);
            g.text(cx + 12, top + 10, cards[i].0, SUB, 1);
            let mut b = [0u8; 32];
            let mut l = 0;
            match i {
                0 => { catn(&mut b, &mut l, hw::cores() as u32); cat(&mut b, &mut l, " 核"); }
                1 => {}
                2 => {
                    let s = hw::uptime_secs();
                    catn(&mut b, &mut l, s / 3600);
                    cat(&mut b, &mut l, ":");
                    let m = (s / 60) % 60;
                    if m < 10 { cat(&mut b, &mut l, "0"); }
                    catn(&mut b, &mut l, m);
                    cat(&mut b, &mut l, ":");
                    let ss = s % 60;
                    if ss < 10 { cat(&mut b, &mut l, "0"); }
                    catn(&mut b, &mut l, ss);
                }
                _ => { cat(&mut b, &mut l, "已开启"); }
            }
            if i == 1 {
                let mut m1 = [0u8; 24];
                let mut l1 = 0;
                cat(&mut m1, &mut l1, "可用 ");
                catn(&mut m1, &mut l1, (free / 1024) as u32);
                cat(&mut m1, &mut l1, " MB");
                g.text(cx + 12, top + 30, core::str::from_utf8_unchecked(&m1[..l1]), TXT, 1);
                let mut m2 = [0u8; 24];
                let mut l2 = 0;
                cat(&mut m2, &mut l2, "总计 ");
                catn(&mut m2, &mut l2, (total / 1024) as u32);
                cat(&mut m2, &mut l2, " MB");
                g.text(cx + 12, top + 50, core::str::from_utf8_unchecked(&m2[..l2]), SUB, 1);
            } else {
                g.text(cx + 12, top + 34, core::str::from_utf8_unchecked(&b[..l]), if i == 3 { GREEN } else { TXT }, 2);
            }
            i += 1;
        }

        let by = top + 84;
        btn(g, x + 16, by, 120, 34, "快速查杀", true);
        btn(g, x + 148, by, 120, 34, "全盘查杀", true);
        btn(g, x + 280, by, 120, 34, "重载病毒库", false);
        let mut b = [0u8; 40];
        let mut l = 0;
        cat(&mut b, &mut l, "病毒库：");
        catn(&mut b, &mut l, self.scan.lib);
        cat(&mut b, &mut l, " 条签名");
        g.text(x + 416, by + 9, core::str::from_utf8_unchecked(&b[..l]), SUB, 1);

        let py = by + 48;
        let label = match self.scan.mode {
            1 => "快速查杀进行中",
            2 => "全盘查杀进行中",
            3 => "扫描完成",
            _ => "实时监控中",
        };
        g.text(x + 18, py, label, TXT, 1);
        bar(g, x + 18, py + 22, w - 36, 18, self.scan.progress, if self.scan.threats > 0 { RED } else { BLUE });
        if self.scan.cur_len > 0 {
            g.text(x + 18, py + 46, core::str::from_utf8_unchecked(&self.scan.cur[..self.scan.cur_len]), SUB, 1);
        }

        let cy = py + 78;
        panel(g, x + 16, cy, w - 32, 74);
        g.text(x + 28, cy + 6, "内存可用率趋势", SUB, 1);
        let gw = w - 80;
        let gh = 42;
        let mut k = 1usize;
        while k < self.mon_n {
            let x0 = x + 38 + (k as i32 - 1) * gw / 39;
            let x1 = x + 38 + k as i32 * gw / 39;
            let y0 = cy + 60 - self.mon[k - 1] as i32 * gh / 100;
            let y1 = cy + 60 - self.mon[k] as i32 * gh / 100;
            g.line(x0, y0, x1, y1, GREEN, 220);
            k += 1;
        }

        let ly = cy + 84;
        g.text(x + 18, ly, "安全日志", SUB, 1);
        g.text(x + w - 200, ly, "Q 快速查杀  F 全盘  R 重载库", SUB, 1);
        let mut j = 0;
        while j < self.scan.logn {
            let line = &self.scan.log[j];
            let mut n = 0;
            while n < 42 && line[n] != 0 { n += 1; }
            g.text(x + 18, ly + 20 + j as i32 * 18, core::str::from_utf8_unchecked(&line[..n]), TXT, 1);
            j += 1;
        }
    }

    unsafe fn security_click(&mut self, lx: i32, ly: i32, w: i32) -> bool {
        let top = 46;
        let by = top + 84;
        if ly > by && ly < by + 38 {
            if lx > 16 && lx < 136 { self.scan_start(false); return true; }
            if lx > 148 && lx < 268 { self.scan_start(true); return true; }
            if lx > 280 && lx < 400 { self.scan.lib = load_db(); self.msg("病毒库已重载", true); return true; }
        }
        let _ = w;
        false
    }

    // ---------- 计算器 ----------
    fn eval(&self) -> Option<f64> {
        let s = &self.calc_in[..self.calc_len];
        let mut p = Parser { s, i: 0 };
        let v = p.expr()?;
        p.skip();
        if p.i != s.len() { return None; }
        Some(v)
    }

    unsafe fn calculator(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32, _id: AppId) {
        let top = y + 50;
        g.round_rect(x + 16, top, w - 32, 48, 10, rgb(24, 28, 38), 255);
        let e = if self.calc_len > 0 { core::str::from_utf8_unchecked(&self.calc_in[..self.calc_len]) } else { "0" };
        g.text(x + 28, top + 15, e, rgb(235, 242, 255), 2);
        let o = core::str::from_utf8_unchecked(&self.calc_out[..self.calc_olen]);
        g.text(x + 16, top + 58, "结果", SUB, 1);
        g.text(x + 62, top + 52, o, TXT, 2);
        let keys = ["7", "8", "9", "/", "4", "5", "6", "*", "1", "2", "3", "-", "0", ".", "=", "+"];
        let bw = (w - 40 - 18) / 4;
        let bh = 42;
        let mut i = 0;
        while i < 16 {
            let c = (i % 4) as i32;
            let r = (i / 4) as i32;
            let bx = x + 16 + c * (bw + 6);
            let by = top + 92 + r * (bh + 6);
            let primary = keys[i] == "=";
            btn(g, bx, by, bw, bh, keys[i], primary);
            i += 1;
        }
        status_line(g, x + 18, y + h - 24, &self.msg, self.msg_len, self.ok);
    }

    unsafe fn calculator_key(&mut self, ch: u16, scan: u16) -> bool {
        let c = ch as u8;
        if ch == 13 {
            if let Some(v) = self.eval() {
                self.calc_olen = 0;
                fmt_f64(&mut self.calc_out, &mut self.calc_olen, v);
                self.msg("计算完成", true);
            } else {
                self.msg("表达式无效", false);
            }
            return true;
        }
        if ch == 8 || scan == 0x17 {
            if self.calc_len > 0 { self.calc_len -= 1; }
            return true;
        }
        if c == b'c' || c == b'C' {
            self.calc_len = 0;
            self.calc_olen = 0;
            cat(&mut self.calc_out, &mut self.calc_olen, "0");
            return true;
        }
        if (c >= b'0' && c <= b'9') || c == b'.' || c == b'+' || c == b'-' || c == b'*' || c == b'/' || c == b'(' || c == b')' {
            if self.calc_len < 60 { self.calc_in[self.calc_len] = c; self.calc_len += 1; }
            return true;
        }
        false
    }

    unsafe fn calculator_click(&mut self, lx: i32, ly: i32, w: i32) -> bool {
        let top = 50;
        let bw = (w - 58) / 4;
        let bh = 42;
        let bx = lx - 16;
        let by = ly - (top + 92);
        if bx < 0 || by < 0 { return false; }
        let c = bx / (bw + 6);
        let r = by / (bh + 6);
        if c > 3 || r > 3 { return false; }
        let i = (r * 4 + c) as usize;
        let keys = ["7", "8", "9", "/", "4", "5", "6", "*", "1", "2", "3", "-", "0", ".", "=", "+"];
        let k = keys[i];
        if k == "=" {
            if let Some(v) = self.eval() {
                self.calc_olen = 0;
                fmt_f64(&mut self.calc_out, &mut self.calc_olen, v);
                self.msg("计算完成", true);
            } else {
                self.msg("表达式无效", false);
            }
        } else {
            for b in k.bytes() { if self.calc_len < 60 { self.calc_in[self.calc_len] = b; self.calc_len += 1; } }
        }
        true
    }

    // ---------- 日历 ----------
    unsafe fn calendar(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        let now = hw::get_time();
        let mut year = now.map(|t| t.year as i32).unwrap_or(2026);
        let mut month = now.map(|t| t.month as i32).unwrap_or(1);
        month += self.month_off;
        while month < 1 { month += 12; year -= 1; }
        while month > 12 { month -= 12; year += 1; }
        let top = y + 50;
        let mut b = [0u8; 24];
        let mut l = 0;
        catn(&mut b, &mut l, year as u32);
        cat(&mut b, &mut l, " 年 ");
        catn(&mut b, &mut l, month as u32);
        cat(&mut b, &mut l, " 月");
        g.text_center(x + w / 2, top, core::str::from_utf8_unchecked(&b[..l]), TXT, 2);
        btn(g, x + 16, top - 4, 34, 28, "<", false);
        btn(g, x + w - 50, top - 4, 34, 28, ">", false);
        let wd = ["日", "一", "二", "三", "四", "五", "六"];
        let cw = (w - 32) / 7;
        let mut i = 0;
        while i < 7 {
            g.text_center(x + 16 + i as i32 * cw + cw / 2, top + 40, wd[i], SUB, 1);
            i += 1;
        }
        let first = weekday(year, month, 1);
        let days = days_in_month(year, month);
        let today = now.map(|t| t.day as i32).unwrap_or(0);
        let today_m = now.map(|t| t.month as i32).unwrap_or(-1);
        let today_y = now.map(|t| t.year as i32).unwrap_or(-1);
        let mut d = 0;
        while d < days {
            let idx = first + d;
            let col = idx % 7;
            let row = idx / 7;
            let cx = x + 16 + col * cw;
            let cy = top + 58 + row * 34;
            let cur = d + 1 == today && month == today_m && year == today_y;
            if cur { g.round_rect(cx + 3, cy - 3, cw - 6, 30, 8, BLUE, 255); }
            let cc = if cur { rgb(255, 255, 255) } else { TXT };
            let mut t = [0u8; 8];
            let mut tl = 0;
            catn(&mut t, &mut tl, (d + 1) as u32);
            g.text_center(cx + cw / 2, cy, core::str::from_utf8_unchecked(&t[..tl]), cc, 1);
            d += 1;
        }
        g.text(x + 16, y + h - 22, "左 / 右 键切换月份   Enter 返回本月", SUB, 1);
    }

    // ---------- 时钟 ----------
    unsafe fn clock(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        let t = hw::get_time();
        let (hh, mm, ss) = t.map(|v| (v.hour as i32, v.minute as i32, v.second as i32)).unwrap_or((0, 0, 0));
        let cx = x + w / 2 - 110;
        let cy = y + h / 2 - 10;
        g.circle(cx, cy, 66, rgb(255, 255, 255), 255);
        g.circle(cx, cy, 66, LINE, 255);
        g.circle(cx, cy, 62, rgb(250, 251, 255), 255);
        let mut i = 0;
        while i < 12 {
            let a = i as f32 * 0.5235988 - 1.5707963;
            let x0 = cx + (cosf(a) * 54.0) as i32;
            let y0 = cy + (sinf(a) * 54.0) as i32;
            let x1 = cx + (cosf(a) * 60.0) as i32;
            let y1 = cy + (sinf(a) * 60.0) as i32;
            g.line(x0, y0, x1, y1, SUB, 255);
            i += 1;
        }
        hand(g, cx, cy, (hh % 12) as f32 / 12.0 + mm as f32 / 720.0, 34.0, 4, rgb(40, 46, 60));
        hand(g, cx, cy, mm as f32 / 60.0 + ss as f32 / 3600.0, 48.0, 3, rgb(40, 46, 60));
        hand(g, cx, cy, ss as f32 / 60.0, 52.0, 1, RED);
        g.circle(cx, cy, 4, RED, 255);
        let mut b = [0u8; 16];
        let mut l = 0;
        if hh < 10 { cat(&mut b, &mut l, "0"); }
        catn(&mut b, &mut l, hh as u32);
        cat(&mut b, &mut l, ":");
        if mm < 10 { cat(&mut b, &mut l, "0"); }
        catn(&mut b, &mut l, mm as u32);
        cat(&mut b, &mut l, ":");
        if ss < 10 { cat(&mut b, &mut l, "0"); }
        catn(&mut b, &mut l, ss as u32);
        g.text_center(cx, cy + 84, core::str::from_utf8_unchecked(&b[..l]), TXT, 2);

        let sx = x + w - 240;
        g.text(sx, y + 60, "秒表", SUB, 1);
        let el = if self.sw_run { hw::uptime_secs().saturating_sub(self.sw_base) } else { self.sw_at };
        let mut s = [0u8; 16];
        let mut sl = 0;
        catn(&mut s, &mut sl, el / 60);
        cat(&mut s, &mut sl, ":");
        let sv = el % 60;
        if sv < 10 { cat(&mut s, &mut sl, "0"); }
        catn(&mut s, &mut sl, sv);
        g.text(sx, y + 82, core::str::from_utf8_unchecked(&s[..sl]), TXT, 3);
        btn(g, sx, y + 140, 96, 34, if self.sw_run { "停止" } else { "开始" }, true);
        btn(g, sx + 106, y + 140, 88, 34, "重置", false);
        g.text(sx, y + 190, "空格 开始/停止   R 重置", SUB, 1);
    }

    // ---------- 浏览器 ----------
    unsafe fn load_home(&mut self) {
        if let Some(n) = read_file(sys_vol(), "\\HYWOS\\SYSTEM\\WELCOME.TXT") {
            let b = rdbuf();
            let c = if n < self.page.len() { n } else { self.page.len() };
            let mut i = 0;
            while i < c { self.page[i] = b[i]; i += 1; }
            self.page_len = c;
        } else {
            let txt = "HYWOS 浏览器\n\n本地页面加载正常。\n按 U 输入网址，Enter 获取。\n";
            self.page_len = 0;
            cat(&mut self.page, &mut self.page_len, txt);
        }
    }

    unsafe fn browser(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        if self.editing {
            editbox(g, x + 16, y + 48, w - 120, &self.url, self.url_len, "输入网址");
            btn(g, x + w - 96, y + 48, 80, 28, "访问", true);
        } else {
            g.text(x + 16, y + 52, if self.url_len > 0 { core::str::from_utf8_unchecked(&self.url[..self.url_len]) } else { "本地页面" }, SUB, 1);
            btn(g, x + w - 96, y + 42, 80, 28, "网址", false);
        }
        g.rect(x + 16, y + 82, w - 32, 1, LINE, 220);
        let top = y + 92;
        let line_h = 18;
        let max = (h - 130) / line_h;
        let mut lines = 0i32;
        let mut col = 0i32;
        let mut start = 0usize;
        let mut i = 0usize;
        while i <= self.page_len && lines < max {
            let c = if i < self.page_len { self.page[i] } else { b'\n' };
            if c == b'\n' || col >= 62 {
                if lines < max {
                    let s = core::str::from_utf8_unchecked(&self.page[start..i]);
                    g.text(x + 18, top + lines * line_h, s, TXT, 1);
                }
                lines += 1;
                col = 0;
                start = i + 1;
            } else if c >= 0x20 || c >= 0x80 {
                col += 1;
            }
            i += 1;
        }
        status_line(g, x + 18, y + h - 24, &self.msg, self.msg_len, self.ok);
    }

    unsafe fn browser_key(&mut self, ch: u16, scan: u16) -> bool {
        if ch == b'u' as u16 || ch == b'U' as u16 {
            self.editing = true;
            self.msg("输入网址后按 Enter", true);
            return true;
        }
        if !self.editing { return false; }
        if ch == 13 {
            self.editing = false;
            if self.url_len == 0 { return true; }
            let u = core::str::from_utf8_unchecked(&self.url[..self.url_len]);
            let mut buf = [0u8; 1600];
            match crate::http::get(u, &mut buf) {
                Ok(n) => {
                    self.page_len = 0;
                    let mut k = 0;
                    while k < n && k < 1500 {
                        let c = buf[k];
                        if c == b'\r' { k += 1; continue; }
                        self.page[self.page_len] = if c < 0x20 && c != b'\n' { b' ' } else { c };
                        self.page_len += 1;
                        k += 1;
                    }
                    self.msg("页面已加载", true);
                }
                Err(_) => self.msg("无法访问该网址，请检查网络或固件 HTTP", false),
            }
            return true;
        }
        if ch == 8 || scan == 0x17 {
            if self.url_len > 0 { self.url_len -= 1; } else { self.editing = false; }
            return true;
        }
        let c = ch as u8;
        if c >= 0x20 && c < 0x7f && self.url_len < 90 {
            self.url[self.url_len] = c;
            self.url_len += 1;
        }
        true
    }

    // ---------- 备忘录 ----------
    unsafe fn notes(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32, id: AppId) {
        let top = y + 48;
        let side = 180;
        g.round_rect(x + 12, top, side, h - 70, 10, rgb(238, 242, 250), 255);
        let mut ents: [DirEntry; 32] = core::mem::zeroed();
        let n = list_dir(usr_vol(), "\\home\\user\\Notes", &mut ents);
        g.text(x + 22, top + 8, "备忘录", SUB, 1);
        let mut k = 0;
        while k < n && k < 10 {
            let name = ents[k].name.as_str();
            if name.ends_with(".txt") {
                let yy = top + 30 + k as i32 * 28;
                if k == self.sel { g.round_rect(x + 18, yy - 6, side - 12, 26, 7, rgb(206, 222, 250), 255); }
                g.text(x + 26, yy, name, TXT, 1);
            }
            k += 1;
        }
        btn(g, x + 20, y + h - 42, 76, 28, "新建", false);
        btn(g, x + 104, y + h - 42, 76, 28, "保存", true);
        let ex = x + side + 26;
        g.round_rect(ex - 8, top - 2, w - side - 40, 26, 8, rgb(255, 255, 255), 255);
        g.text(ex, top + 4, if self.editing { "编辑中 · Ctrl+S 保存" } else { "Enter 打开" }, SUB, 1);
        let max = (h - 120) / 20;
        let mut line = 0;
        let mut col = 0;
        let mut start = 0;
        let mut i = 0usize;
        while i <= self.elen && line < max {
            let c = if i < self.elen { self.edit[i] } else { b'\n' };
            if c == b'\n' || col >= 48 {
                let s = core::str::from_utf8_unchecked(&self.edit[start..i]);
                g.text(ex, top + 36 + line * 20, s, TXT, 1);
                line += 1;
                col = 0;
                start = i + 1;
            } else { col += 1; }
            i += 1;
        }
        status_line(g, ex, y + h - 24, &self.msg, self.msg_len, self.ok);
        let _ = id;
    }

    unsafe fn note_open(&mut self, idx: usize) {
        let mut ents: [DirEntry; 32] = core::mem::zeroed();
        let n = list_dir(usr_vol(), "\\home\\user\\Notes", &mut ents);
        let mut k = 0;
        let mut pick = usize::MAX;
        let mut count = 0;
        while k < n {
            if ents[k].name.as_str().ends_with(".txt") {
                if count == idx { pick = k; break; }
                count += 1;
            }
            k += 1;
        }
        if pick == usize::MAX { return; }
        let full = path_join("\\home\\user\\Notes", ents[pick].name.as_str());
        if let Some(sz) = read_file(usr_vol(), full.as_str()) {
            let b = rdbuf();
            self.elen = if sz < self.edit.len() { sz } else { self.edit.len() };
            let mut i = 0;
            while i < self.elen { self.edit[i] = b[i]; i += 1; }
            self.editing = true;
            self.msg("已打开", true);
        }
    }

    unsafe fn notes_key(&mut self, ch: u16, scan: u16) -> bool {
        if ch == 19 {
            if self.elen == 0 { return true; }
            let mut name = Str80::empty();
            name.push_str("\\home\\user\\Notes\\note");
            let mut b = [0u8; 8];
            let mut l = 0;
            catn(&mut b, &mut l, self.sel as u32 + 1);
            name.push_str(core::str::from_utf8_unchecked(&b[..l]));
            name.push_str(".txt");
            if write_file(usr_vol(), name.as_str(), &self.edit[..self.elen]) {
                self.msg("已保存", true);
            } else {
                self.msg("保存失败", false);
            }
            return true;
        }
        if !self.editing { return false; }
        if ch == 13 {
            if self.elen < self.edit.len() { self.edit[self.elen] = b'\n'; self.elen += 1; }
            return true;
        }
        if ch == 8 || scan == 0x17 {
            if self.elen > 0 { self.elen -= 1; }
            return true;
        }
        let c = ch as u8;
        if c >= 0x20 && c < 0x7f && self.elen < self.edit.len() - 1 {
            self.edit[self.elen] = c;
            self.elen += 1;
            return true;
        }
        false
    }

    unsafe fn notes_click(&mut self, lx: i32, ly: i32, x: i32, y: i32, w: i32, h: i32) -> bool {
        let top = y + 48;
        if lx > x + 12 && lx < x + 12 + 180 && ly > top {
            let idx = ((ly - top - 30 + 6) / 28).max(0) as usize;
            if idx < 10 {
                self.sel = idx;
                self.note_open(idx);
            }
            return true;
        }
        if ly > y + h - 46 && ly < y + h - 14 {
            if lx > x + 20 && lx < x + 96 {
                self.elen = 0;
                self.editing = true;
                self.msg("新建备忘录", true);
                return true;
            }
            if lx > x + 104 && lx < x + 180 {
                self.notes_key(19, 0);
                return true;
            }
        }
        let _ = w;
        false
    }

    // ---------- 照片 / 视频 ----------
    unsafe fn photos(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32, video: bool) {
        let dir = if video { "\\home\\user\\Videos" } else { "\\home\\user\\Pictures" };
        let mut files: [Str80; 32] = core::mem::zeroed();
        let n = collect(dir, ".bmp", &mut files);
        g.text(x + 16, y + 50, if video { "视频（BMP 序列）" } else { "照片" }, SUB, 1);
        let mut b = [0u8; 24];
        let mut l = 0;
        catn(&mut b, &mut l, (self.idx + 1).min(n.max(1)) as u32);
        cat(&mut b, &mut l, " / ");
        catn(&mut b, &mut l, n as u32);
        g.text(x + w - 96, y + 50, core::str::from_utf8_unchecked(&b[..l]), SUB, 1);
        if n == 0 {
            g.text_center(x + w / 2, y + h / 2, if video { "Videos 目录暂无 BMP 序列" } else { "Pictures 目录暂无照片" }, SUB, 2);
            g.text_center(x + w / 2, y + h / 2 + 34, "可将 BMP 文件放入对应目录", SUB, 1);
            return;
        }
        if video && self.playing {
            self.frame_t += 1;
            if self.frame_t % 30 == 0 { self.idx = (self.idx + 1) % n; }
        }
        if self.idx >= n { self.idx = 0; }
        let full = &files[self.idx];
        if let Some((iw, ih)) = load_bmp(usr_vol(), full.as_str()) {
            let iy = y + 70;
            let ih2 = h - 108;
            fit_img(g, x + 16, iy, w - 32, ih2, iw, ih);
        } else {
            g.text_center(x + w / 2, y + h / 2, "无法解码该 BMP", RED, 1);
        }
        let hint = if video {
            if self.playing { "空格 暂停   左 / 右 键切换" } else { "空格 播放   左 / 右 键切换" }
        } else {
            "左 / 右 键切换照片"
        };
        g.text(x + 16, y + h - 22, hint, SUB, 1);
        status_line(g, x + w - 220, y + h - 22, &self.msg, self.msg_len, self.ok);
    }

    unsafe fn photos_key(&mut self, ch: u16, scan: u16, video: bool) -> bool {
        if scan == 0x04 || scan == 0x03 {
            let dir = if video { "\\home\\user\\Videos" } else { "\\home\\user\\Pictures" };
            let mut files: [Str80; 32] = core::mem::zeroed();
            let n = collect(dir, ".bmp", &mut files);
            if n > 0 {
                if scan == 0x04 { self.idx = (self.idx + n - 1) % n; } else { self.idx = (self.idx + 1) % n; }
            }
            return true;
        }
        if video && ch == 32 {
            self.playing = !self.playing;
            self.frame_t = 0;
            return true;
        }
        false
    }

    // ---------- 音乐 ----------
    unsafe fn music(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        let mut files: [Str80; 32] = core::mem::zeroed();
        let n = collect("\\home\\user\\Music", ".wav", &mut files);
        g.text(x + 16, y + 50, "音乐", SUB, 1);
        let mut k = 0;
        while k < n && k < 8 {
            let yy = y + 76 + k as i32 * 28;
            if k == self.sel { g.round_rect(x + 14, yy - 6, w - 28, 26, 7, rgb(214, 228, 252), 255); }
            g.text(x + 24, yy, files[k].as_str(), TXT, 1);
            k += 1;
        }
        if n == 0 {
            g.text_center(x + w / 2, y + h / 2 - 20, "Music 目录暂无 WAV", SUB, 2);
        }
        btn(g, x + 16, y + h - 60, 110, 34, "播放", true);
        btn(g, x + 136, y + h - 60, 110, 34, "停止", false);
        btn(g, x + 256, y + h - 60, 130, 34, "音阶演示", false);
        g.text(x + 16, y + h - 90, if crate::audio::playing() { "正在播放" } else { "就绪" }, if crate::audio::playing() { GREEN } else { SUB }, 1);
        status_line(g, x + 400, y + h - 50, &self.msg, self.msg_len, self.ok);
        let _ = h;
    }

    unsafe fn music_click(&mut self, lx: i32, ly: i32, y: i32, h: i32) -> bool {
        if ly > y + h - 64 && ly < y + h - 26 {
            if lx > 16 && lx < 126 {
                self.play_selected();
                return true;
            }
            if lx > 136 && lx < 246 {
                crate::audio::stop();
                self.msg("已停止", true);
                return true;
            }
            if lx > 256 && lx < 386 {
                if crate::audio::tone_seq() { self.msg("播放音阶", true); } else { self.msg("音频设备不可用", false); }
                return true;
            }
        }
        let idx = ((ly - (y + 76) + 6) / 28).max(0) as usize;
        if ly > y + 70 && idx < 8 {
            self.sel = idx;
            return true;
        }
        false
    }

    unsafe fn play_selected(&mut self) {
        let mut files: [Str80; 32] = core::mem::zeroed();
        let n = collect("\\home\\user\\Music", ".wav", &mut files);
        if n == 0 { self.msg("没有可播放的 WAV", false); return; }
        if self.sel >= n { self.sel = 0; }
        let full = &files[self.sel];
        if let Some(sz) = read_file(usr_vol(), full.as_str()) {
            let b = rdbuf();
            if crate::audio::play_wav(&b[..sz]) {
                self.msg(full.as_str(), true);
            } else {
                self.msg("WAV 格式不支持（需 PCM 16 位）", false);
            }
        } else {
            self.msg("无法读取文件", false);
        }
    }

    // ---------- 地图 ----------
    unsafe fn maps(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        let top = y + 48;
        g.round_rect(x + 12, top, w - 24, h - 78, 12, rgb(15, 34, 62), 255);
        let cx = x + w / 2 + self.pan;
        let cy = top + (h - 78) / 2;
        let sc = 14 + self.zoom * 3;
        let mut lon = -180;
        while lon <= 180 {
            let px = cx + lon * sc / 10;
            g.line(px, top + 4, px, top + h - 82, rgb(50, 90, 140), 120);
            lon += 30;
        }
        let mut lat = -60;
        while lat <= 60 {
            let py = cy - lat * sc / 10;
            g.line(x + 14, py, x + w - 16, py, rgb(50, 90, 140), 120);
            lat += 30;
        }
        // 简化大陆轮廓
        let bx = x + 16;
        let by = top + 6;
        let bw = w - 32;
        let bh = h - 88;
        continent(g, cx, cy, sc, &ASIA, rgb(48, 108, 76), bx, by, bw, bh);
        continent(g, cx, cy, sc, &EUROPE, rgb(48, 108, 76), bx, by, bw, bh);
        continent(g, cx, cy, sc, &AFRICA, rgb(48, 108, 76), bx, by, bw, bh);
        continent(g, cx, cy, sc, &AMERICA, rgb(48, 108, 76), bx, by, bw, bh);
        continent(g, cx, cy, sc, &AUS, rgb(48, 108, 76), bx, by, bw, bh);
        let cities = [
            ("北京", 116.4f32, 39.9f32), ("上海", 121.5, 31.2), ("东京", 139.7, 35.7),
            ("伦敦", -0.1, 51.5), ("纽约", -74.0, 40.7), ("悉尼", 151.2, -33.9),
            ("开罗", 31.2, 30.0), ("里约", -43.2, -22.9),
        ];
        let mut i = 0;
        while i < cities.len() {
            let px = cx + (cities[i].1 * sc as f32 / 10.0) as i32;
            let py = cy - (cities[i].2 * sc as f32 / 10.0) as i32;
            if px > x + 20 && px < x + w - 20 && py > top + 8 && py < top + h - 90 {
                g.circle(px, py, 4, rgb(255, 196, 84), 255);
                g.text(px + 8, py - 6, cities[i].0, rgb(228, 238, 252), 1);
            }
            i += 1;
        }
        g.text(x + 18, y + h - 24, "方向键平移   + / - 缩放", SUB, 1);
        let mut b = [0u8; 24];
        let mut l = 0;
        cat(&mut b, &mut l, "缩放 ");
        catn(&mut b, &mut l, (self.zoom + 1) as u32);
        g.text(x + w - 80, y + h - 24, core::str::from_utf8_unchecked(&b[..l]), SUB, 1);
    }

    unsafe fn maps_key(&mut self, scan: u16, ch: u16) -> bool {
        match scan {
            0x04 => { self.pan += 24; return true; }
            0x03 => { self.pan -= 24; return true; }
            0x01 => { self.pan = self.pan; return true; }
            _ => {}
        }
        let c = ch as u8;
        if c == b'+' || c == b'=' { if self.zoom < 6 { self.zoom += 1; } return true; }
        if c == b'-' { if self.zoom > 0 { self.zoom -= 1; } return true; }
        false
    }

    // ---------- 相机 ----------
    unsafe fn camera(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        let top = y + 48;
        self.cam_t += 1;
        if self.shoot { self.shoot = false; self.camera_shot(g); }
        g.round_rect(x + 16, top, w - 32, h - 114, 12, rgb(12, 16, 24), 255);
        let vx = x + 34;
        let vy = top + 18;
        let vw = w - 68;
        let vh = h - 150;
        g.round_rect(vx, vy, vw, vh, 8, rgb(20, 28, 40), 255);
        let t = hw::get_time();
        let mut b = [0u8; 32];
        let mut l = 0;
        if let Some(v) = t {
            catn(&mut b, &mut l, v.year as u32);
            cat(&mut b, &mut l, "-");
            if v.month < 10 { cat(&mut b, &mut l, "0"); }
            catn(&mut b, &mut l, v.month as u32);
            cat(&mut b, &mut l, "-");
            if v.day < 10 { cat(&mut b, &mut l, "0"); }
            catn(&mut b, &mut l, v.day as u32);
            cat(&mut b, &mut l, " ");
            if v.hour < 10 { cat(&mut b, &mut l, "0"); }
            catn(&mut b, &mut l, v.hour as u32);
            cat(&mut b, &mut l, ":");
            if v.minute < 10 { cat(&mut b, &mut l, "0"); }
            catn(&mut b, &mut l, v.minute as u32);
            cat(&mut b, &mut l, ":");
            if v.second < 10 { cat(&mut b, &mut l, "0"); }
            catn(&mut b, &mut l, v.second as u32);
        }
        g.text(vx + 12, vy + 10, core::str::from_utf8_unchecked(&b[..l]), rgb(220, 235, 250), 1);
        g.text(vx + 12, vy + vh - 22, "HYWOS CAMERA", rgb(150, 210, 255), 1);
        let mut i = 1;
        while i < 3 {
            g.line(vx + vw * i / 3, vy, vx + vw * i / 3, vy + vh, rgb(255, 255, 255), 24);
            g.line(vx, vy + vh * i / 3, vx + vw, vy + vh * i / 3, rgb(255, 255, 255), 24);
            i += 1;
        }
        let mut cx = 0;
        while cx < 4 {
            let px = vx + 8 + cx * (vw - 36) / 3;
            g.rect(px, vy + 8, 20, 3, rgb(255, 255, 255), 200);
            g.rect(px, vy + 8, 3, 20, rgb(255, 255, 255), 200);
            g.rect(px, vy + vh - 11, 20, 3, rgb(255, 255, 255), 200);
            g.rect(px + 17, vy + vh - 28, 3, 20, rgb(255, 255, 255), 200);
            cx += 1;
        }
        btn(g, x + w / 2 - 70, y + h - 54, 140, 34, "拍照保存", true);
        status_line(g, x + 18, y + h - 24, &self.msg, self.msg_len, self.ok);
        if self.cam_t % 6 == 0 {
            g.circle(vx + vw - 16, vy + 16, 5, RED, 255);
        }
    }

    unsafe fn camera_shot(&mut self, g: &Gfx) {
        let w = g.stride;
        let h = g.h;
        let n = w * h;
        let px = core::slice::from_raw_parts(g.fb, n);
        match crate::fs::screenshot(px, w, h) {
            Some(k) => {
                let mut b = [0u8; 48];
                let mut l = 0;
                cat(&mut b, &mut l, "已保存 Screenshots/shot-");
                catn(&mut b, &mut l, k);
                cat(&mut b, &mut l, ".bmp");
                self.msg(core::str::from_utf8_unchecked(&b[..l]), true);
            }
            None => self.msg("保存失败", false),
        }
    }

    // ---------- 通讯录 / 信息 / 邮件 ----------
    unsafe fn contacts(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        self.lines_view(g, x, y, w, h, "\\home\\user\\Contacts\\contacts.txt", "通讯录", "姓名|电话|邮箱");
    }

    unsafe fn messages(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        self.lines_view(g, x, y, w, h, "\\home\\user\\Messages\\threads.txt", "信息", "对象|内容|时间");
    }

    unsafe fn mail(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
        self.lines_view(g, x, y, w, h, "\\home\\user\\Mail\\inbox.txt", "邮件", "发件人|主题|时间");
    }

    unsafe fn lines_view(&mut self, g: &Gfx, x: i32, y: i32, w: i32, h: i32, path: &str, title: &str, hint: &str) {
        let top = y + 48;
        g.text(x + 16, y + 50, title, SUB, 1);
        let n = if let Some(sz) = read_file(usr_vol(), path) {
            let b = rdbuf();
            let mut lines: [Range; 24] = core::mem::zeroed();
            let mut ln = 0usize;
            let mut start = 0usize;
            let mut i = 0usize;
            while i <= sz && ln < 24 {
                if i == sz || b[i] == b'\n' {
                    lines[ln] = Range { a: start, b: i };
                    ln += 1;
                    start = i + 1;
                }
                i += 1;
            }
            LINES_N = ln;
            let mut k = 0;
            while k < ln && k < 8 {
                let s = core::str::from_utf8_unchecked(&b[lines[k].a..lines[k].b]);
                let yy = top + 22 + k as i32 * 26;
                if k == self.sel { g.round_rect(x + 14, yy - 6, w - 28, 24, 7, rgb(214, 228, 252), 255); }
                g.text(x + 24, yy, s, TXT, 1);
                k += 1;
            }
            ln
        } else { 0 };
        if n == 0 {
            g.text_center(x + w / 2, y + h / 2 - 20, "暂无记录", SUB, 2);
        }
        g.text(x + 16, y + h - 96, hint, SUB, 1);
        editbox(g, x + 16, y + h - 74, w - 160, &self.edit, self.elen, "输入内容，Enter 保存");
        btn(g, x + w - 132, y + h - 76, 116, 30, "保存", true);
        status_line(g, x + 18, y + h - 24, &self.msg, self.msg_len, self.ok);
    }

    unsafe fn lines_save(&mut self, path: &str) {
        if self.elen == 0 { self.msg("输入不能为空", false); return; }
        let mut line = [0u8; 200];
        let mut ln = 0;
        let mut i = 0;
        while i < self.elen && ln < 190 { line[ln] = self.edit[i]; ln += 1; i += 1; }
        line[ln] = b'\n';
        ln += 1;
        if append_file(usr_vol(), path, &line[..ln]) {
            self.msg("已保存", true);
            self.elen = 0;
        } else {
            self.msg("保存失败", false);
        }
    }

    unsafe fn lines_click(&mut self, lx: i32, ly: i32, x: i32, y: i32, w: i32, h: i32, path: &str) -> bool {
        if ly > y + h - 80 && ly < y + h - 40 {
            if lx > x + w - 136 && lx < x + w - 12 {
                self.lines_save(path);
                return true;
            }
        }
        let top = y + 48;
        let idx = ((ly - top - 22 + 6) / 26).max(0) as usize;
        if ly > top + 16 && idx < 8 { self.sel = idx; return true; }
        false
    }

}

// ---------- 模块级辅助 ----------
#[derive(Clone, Copy)]
struct Range { a: usize, b: usize }
static mut LINES_N: usize = 0;

struct Parser<'a> { s: &'a [u8], i: usize }

impl<'a> Parser<'a> {
    fn skip(&mut self) {
        while self.i < self.s.len() && (self.s[self.i] == b' ' || self.s[self.i] == b'\t') { self.i += 1; }
    }
    fn expr(&mut self) -> Option<f64> {
        let mut v = self.term()?;
        loop {
            self.skip();
            if self.i < self.s.len() && (self.s[self.i] == b'+' || self.s[self.i] == b'-') {
                let op = self.s[self.i];
                self.i += 1;
                let r = self.term()?;
                v = if op == b'+' { v + r } else { v - r };
            } else { return Some(v); }
        }
    }
    fn term(&mut self) -> Option<f64> {
        let mut v = self.factor()?;
        loop {
            self.skip();
            if self.i < self.s.len() && (self.s[self.i] == b'*' || self.s[self.i] == b'/') {
                let op = self.s[self.i];
                self.i += 1;
                let r = self.factor()?;
                if op == b'/' && r == 0.0 { return None; }
                v = if op == b'*' { v * r } else { v / r };
            } else { return Some(v); }
        }
    }
    fn factor(&mut self) -> Option<f64> {
        self.skip();
        if self.i >= self.s.len() { return None; }
        if self.s[self.i] == b'-' { self.i += 1; return self.factor().map(|v| -v); }
        if self.s[self.i] == b'(' {
            self.i += 1;
            let v = self.expr()?;
            self.skip();
            if self.i < self.s.len() && self.s[self.i] == b')' { self.i += 1; return Some(v); }
            return None;
        }
        let mut v = 0.0f64;
        let mut any = false;
        while self.i < self.s.len() && self.s[self.i] >= b'0' && self.s[self.i] <= b'9' {
            v = v * 10.0 + (self.s[self.i] - b'0') as f64;
            self.i += 1;
            any = true;
        }
        if self.i < self.s.len() && self.s[self.i] == b'.' {
            self.i += 1;
            let mut f = 0.1;
            while self.i < self.s.len() && self.s[self.i] >= b'0' && self.s[self.i] <= b'9' {
                v += (self.s[self.i] - b'0') as f64 * f;
                f *= 0.1;
                self.i += 1;
                any = true;
            }
        }
        if any { Some(v) } else { None }
    }
}

fn fmt_f64(dst: &mut [u8], l: &mut usize, v: f64) {
    if v.is_nan() || v.is_infinite() { cat(dst, l, "错误"); return; }
    let mut neg = false;
    let mut x = v;
    if x < 0.0 { neg = true; x = -x; }
    if neg { cat(dst, l, "-"); }
    let ip = x as i64;
    catn(dst, l, ip as u32);
    let frac = ((x - ip as f64) * 100.0 + 0.5) as i64;
    if frac > 0 { cat(dst, l, "."); if frac < 10 { cat(dst, l, "0"); } catn(dst, l, frac as u32); }
}

fn weekday(y: i32, m: i32, d: i32) -> i32 {
    let t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let mut yy = y;
    if m < 3 { yy -= 1; }
    (yy + yy / 4 - yy / 100 + yy / 400 + t[(m - 1) as usize] + d) % 7
}

fn days_in_month(y: i32, m: i32) -> i32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 29 } else { 28 },
    }
}

fn hand(g: &Gfx, cx: i32, cy: i32, frac: f32, len: f32, th: i32, c: u32) {
    let a = frac * 6.2831853 - 1.5707963;
    let ex = cx + (cosf(a) * len) as i32;
    let ey = cy + (sinf(a) * len) as i32;
    let mut k = -th;
    while k <= th {
        g.line(cx, cy + k, ex, ey + k, c, 255);
        k += 1;
    }
}

static mut QPATH: [[u8; 80]; 400] = [[0; 80]; 400];
static mut QLEN: [u8; 400] = [0; 400];
static mut QVOL: [u8; 400] = [0; 400];
static mut QN: usize = 0;
static mut QIDX: usize = 0;

unsafe fn add_path(vol: usize, s: &str) {
    if QN >= 400 { return; }
    let b = s.as_bytes();
    if b.len() > 78 { return; }
    let mut i = 0;
    while i < b.len() { QPATH[QN][i] = b[i]; i += 1; }
    QLEN[QN] = b.len() as u8;
    QVOL[QN] = vol as u8;
    QN += 1;
}

unsafe fn walk_add(vol: usize, dir: &str, depth: usize) {
    if depth > 4 || QN >= 400 { return; }
    let mut ents: [DirEntry; 32] = core::mem::zeroed();
    let n = list_dir(vol, dir, &mut ents);
    let mut i = 0;
    while i < n && QN < 400 {
        let name = ents[i].name.as_str();
        if name != "." && name != ".." {
            let full = path_join(dir, name);
            if ents[i].is_dir {
                walk_add(vol, full.as_str(), depth + 1);
            } else {
                add_path(vol, full.as_str());
            }
        }
        i += 1;
    }
}

unsafe fn hash_file(vol: usize, path: &str) -> Option<(u32, u64)> {
    let f = open_vol(vol, path)?;
    let sz = file_size(f)?;
    let mut buf = [0u8; 4096];
    let n = read_at(f, 0, &mut buf).unwrap_or(0);
    close_f(f);
    let mut h = 2166136261u32;
    let mut i = 0;
    while i < n {
        h ^= buf[i] as u32;
        h = h.wrapping_mul(16777619);
        i += 1;
    }
    Some((h ^ (sz as u32), sz as u64))
}

#[derive(Clone, Copy)]
struct Sig { h: u32, name: [u8; 24], n: usize }
static mut SIGS: [Sig; 16] = [Sig { h: 0, name: [0; 24], n: 0 }; 16];
static mut SN: usize = 0;

unsafe fn hex8(b: &[u8]) -> Option<u32> {
    if b.len() < 8 { return None; }
    let mut v = 0u32;
    let mut i = 0;
    while i < 8 {
        let c = b[i];
        let d = if c >= b'0' && c <= b'9' { c - b'0' } else if c >= b'a' && c <= b'f' { c - b'a' + 10 } else if c >= b'A' && c <= b'F' { c - b'A' + 10 } else { return None; };
        v = (v << 4) | d as u32;
        i += 1;
    }
    Some(v)
}

unsafe fn parse_db(b: &[u8], n: usize) {
    let mut i = 0;
    while i < n && SN < 16 {
        let mut e = i;
        while e < n && b[e] != b'\n' && b[e] != b'\r' { e += 1; }
        if e - i >= 9 {
            if let Some(h) = hex8(&b[i..e]) {
                let mut k = i + 8;
                while k < e && (b[k] == b'|' || b[k] == b',' || b[k] == b' ') { k += 1; }
                let s = &mut SIGS[SN];
                s.h = h;
                s.n = 0;
                while k < e && s.n < 23 { s.name[s.n] = b[k]; s.n += 1; k += 1; }
                if s.n > 0 { SN += 1; }
            }
        }
        i = e + 1;
    }
}

unsafe fn load_db() -> u32 {
    SN = 0;
    let db = "\\HYWOS\\SECURITY\\VIRUS.DB";
    if let Some(n) = read_file(usr_vol(), db) {
        let b = rdbuf();
        parse_db(b, n);
    }
    if SN == 0 && sys_vol() != usr_vol() {
        if let Some(n) = read_file(sys_vol(), db) {
            let b = rdbuf();
            parse_db(b, n);
        }
    }
    SN as u32
}

unsafe fn db_match(h: u32) -> Option<&'static str> {
    let mut i = 0;
    while i < SN {
        let s = &SIGS[i];
        if s.h == h {
            return Some(core::str::from_utf8_unchecked(&s.name[..s.n]));
        }
        i += 1;
    }
    None
}

static mut IMG: [u32; 1024 * 768] = [0; 1024 * 768];

unsafe fn load_bmp(vol: usize, path: &str) -> Option<(usize, usize)> {
    let n = read_file(vol, path)?;
    let b = rdbuf();
    if n < 54 || &b[0..2] != b"BM" { return None; }
    let off = (b[10] as usize) | ((b[11] as usize) << 8) | ((b[12] as usize) << 16) | ((b[13] as usize) << 24);
    let w = i32::from_le_bytes([b[18], b[19], b[20], b[21]]);
    let h = i32::from_le_bytes([b[22], b[23], b[24], b[25]]);
    if w <= 0 || h == 0 || w > 1024 || h.abs() > 768 { return None; }
    let bpp = b[28] as usize;
    if bpp != 24 { return None; }
    let ww = w as usize;
    let hh = h.unsigned_abs() as usize;
    let row = ((ww * 3 + 3) / 4) * 4;
    if off + row * hh > n { return None; }
    let img = &mut *core::ptr::addr_of_mut!(IMG);
    let mut y = 0;
    while y < hh {
        let sy = if h > 0 { hh - 1 - y } else { y };
        let base = off + sy * row;
        let mut x = 0;
        while x < ww {
            let o = base + x * 3;
            let bb = b[o] as u32;
            let gg = b[o + 1] as u32;
            let rr = b[o + 2] as u32;
            img[y * ww + x] = (rr << 16) | (gg << 8) | bb;
            x += 1;
        }
        y += 1;
    }
    Some((ww, hh))
}

fn fit_img(g: &Gfx, x: i32, y: i32, w: i32, h: i32, iw: usize, ih: usize) {
    if iw == 0 || ih == 0 { return; }
    let mut dw = w;
    let mut dh = dw * ih as i32 / iw as i32;
    if dh > h { dh = h; dw = dh * iw as i32 / ih as i32; }
    let ox = x + (w - dw) / 2;
    let oy = y + (h - dh) / 2;
    let img = unsafe { &*core::ptr::addr_of!(IMG) };
    let mut yy = 0;
    while yy < dh {
        let sy = (yy * ih as i32 / dh) as usize;
        let mut xx = 0;
        while xx < dw {
            let sx = (xx * iw as i32 / dw) as usize;
            let c = img[sy * iw + sx];
            g.put(ox + xx, oy + yy, c, 255);
            xx += 1;
        }
        yy += 1;
    }
}

unsafe fn collect(dir: &str, ext: &str, out: &mut [Str80; 32]) -> usize {
    let mut ents: [DirEntry; 32] = core::mem::zeroed();
    let n = list_dir(usr_vol(), dir, &mut ents);
    let mut k = 0;
    let mut c = 0;
    while k < n && c < 32 {
        let name = ents[k].name.as_str();
        let ok = if ext == ".bmp" { name.ends_with(".bmp") || name.ends_with(".BMP") } else { name.ends_with(".wav") || name.ends_with(".WAV") };
        if !ents[k].is_dir && ok {
            out[c] = path_join(dir, name);
            c += 1;
        }
        k += 1;
    }
    c
}

const ASIA: [(f32, f32); 8] = [(30.0, 70.0), (100.0, 75.0), (145.0, 60.0), (150.0, 40.0), (125.0, 20.0), (100.0, 5.0), (70.0, 20.0), (45.0, 40.0)];
const EUROPE: [(f32, f32); 6] = [(-10.0, 60.0), (30.0, 70.0), (45.0, 55.0), (30.0, 40.0), (0.0, 36.0), (-10.0, 45.0)];
const AFRICA: [(f32, f32); 6] = [(-15.0, 35.0), (30.0, 32.0), (45.0, 10.0), (30.0, -30.0), (10.0, -34.0), (-10.0, 0.0)];
const AMERICA: [(f32, f32); 7] = [(-160.0, 68.0), (-60.0, 60.0), (-80.0, 25.0), (-60.0, -10.0), (-70.0, -50.0), (-110.0, -20.0), (-125.0, 20.0)];
const AUS: [(f32, f32); 5] = [(115.0, -20.0), (150.0, -22.0), (153.0, -37.0), (130.0, -32.0), (114.0, -32.0)];

fn continent(g: &Gfx, cx: i32, cy: i32, sc: i32, pts: &[(f32, f32)], c: u32, bx: i32, by: i32, bw: i32, bh: i32) {
    let clampx = |v: i32| -> i32 { if v < bx { bx } else if v > bx + bw { bx + bw } else { v } };
    let clampy = |v: i32| -> i32 { if v < by { by } else if v > by + bh { by + bh } else { v } };
    let mut i = 0;
    while i < pts.len() {
        let j = (i + 1) % pts.len();
        let x0 = clampx(cx + (pts[i].0 * sc as f32 / 10.0) as i32);
        let y0 = clampy(cy - (pts[i].1 * sc as f32 / 10.0) as i32);
        let x1 = clampx(cx + (pts[j].0 * sc as f32 / 10.0) as i32);
        let y1 = clampy(cy - (pts[j].1 * sc as f32 / 10.0) as i32);
        g.line(x0, y0, x1, y1, c, 210);
        g.line(x0, y0 + 1, x1, y1 + 1, c, 150);
        i += 1;
    }
}

// ---------- 对外分发 ----------
pub unsafe fn draw(st: &mut Suite, id: AppId, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    match id {
        AppId::Security => st.security(g, x, y, w, h),
        AppId::Calculator => st.calculator(g, x, y, w, h, id),
        AppId::Calendar => st.calendar(g, x, y, w, h),
        AppId::Clock => st.clock(g, x, y, w, h),
        AppId::Browser => st.browser(g, x, y, w, h),
        AppId::Notes => st.notes(g, x, y, w, h, id),
        AppId::Photos => st.photos(g, x, y, w, h, false),
        AppId::Videos => st.photos(g, x, y, w, h, true),
        AppId::Music => st.music(g, x, y, w, h),
        AppId::Maps => st.maps(g, x, y, w, h),
        AppId::Camera => st.camera(g, x, y, w, h),
        AppId::Contacts => st.contacts(g, x, y, w, h),
        AppId::Messages => st.messages(g, x, y, w, h),
        AppId::Mail => st.mail(g, x, y, w, h),
        _ => {}
    }
}

pub unsafe fn key(st: &mut Suite, id: AppId, scan: u16, ch: u16) -> bool {
    if id == AppId::Security {
        if ch == b'q' as u16 || ch == b'Q' as u16 { st.scan_start(false); return true; }
        if ch == b'f' as u16 || ch == b'F' as u16 { st.scan_start(true); return true; }
        if ch == b'r' as u16 || ch == b'R' as u16 { st.scan.lib = load_db(); st.msg("病毒库已重载", true); return true; }
    }
    match id {
        AppId::Calculator => st.calculator_key(ch, scan),
        AppId::Calendar => {
            if scan == 0x04 { st.month_off -= 1; true }
            else if scan == 0x03 { st.month_off += 1; true }
            else if ch == 13 { st.month_off = 0; true }
            else { false }
        }
        AppId::Clock => {
            if ch == 32 {
                if st.sw_run { st.sw_at = hw::uptime_secs().saturating_sub(st.sw_base); st.sw_run = false; }
                else { st.sw_base = hw::uptime_secs(); st.sw_run = true; }
                true
            } else if ch == b'r' as u16 || ch == b'R' as u16 { st.sw_run = false; st.sw_at = 0; true }
            else { false }
        }
        AppId::Browser => st.browser_key(ch, scan),
        AppId::Notes => st.notes_key(ch, scan),
        AppId::Music => {
            if ch == 13 { st.play_selected(); true }
            else if ch == 32 { crate::audio::stop(); st.msg("已停止", true); true }
            else { false }
        }
        AppId::Photos => st.photos_key(ch, scan, false),
        AppId::Videos => st.photos_key(ch, scan, true),
        AppId::Maps => st.maps_key(scan, ch),
        AppId::Camera => {
            if ch == 13 { st.shoot = true; true } else { false }
        }
        AppId::Contacts => {
            if ch == 13 { let p = "\\home\\user\\Contacts\\contacts.txt"; st.lines_save(p); true }
            else if ch == 8 || scan == 0x17 { if st.elen > 0 { st.elen -= 1; } true }
            else { let c = ch as u8; if c >= 0x20 && c < 0x7f && st.elen < 500 { st.edit[st.elen] = c; st.elen += 1; true } else { false } }
        }
        AppId::Messages => {
            if ch == 13 {
                let mut line = [0u8; 200];
                let mut l = 0;
                cat(&mut line, &mut l, "本地|");
                let mut i = 0;
                while i < st.elen && l < 180 { line[l] = st.edit[i]; l += 1; i += 1; }
                line[l] = b'\n';
                l += 1;
                if append_file(usr_vol(), "\\home\\user\\Messages\\threads.txt", &line[..l]) {
                    st.msg("已发送到本地记录", true);
                    st.elen = 0;
                } else { st.msg("保存失败", false); }
                true
            } else if ch == 8 || scan == 0x17 { if st.elen > 0 { st.elen -= 1; } true }
            else { let c = ch as u8; if c >= 0x20 && c < 0x7f && st.elen < 500 { st.edit[st.elen] = c; st.elen += 1; true } else { false } }
        }
        AppId::Mail => {
            if ch == 13 { let p = "\\home\\user\\Mail\\outbox.txt"; st.lines_save(p); true }
            else if ch == 8 || scan == 0x17 { if st.elen > 0 { st.elen -= 1; } true }
            else { let c = ch as u8; if c >= 0x20 && c < 0x7f && st.elen < 500 { st.edit[st.elen] = c; st.elen += 1; true } else { false } }
        }
        _ => false,
    }
}

pub unsafe fn click(st: &mut Suite, id: AppId, lx: i32, ly: i32, x: i32, y: i32, w: i32, h: i32) -> bool {
    match id {
        AppId::Security => st.security_click(lx, ly, w),
        AppId::Calculator => st.calculator_click(lx, ly, w),
        AppId::Calendar => {
            if ly > 42 && ly < 76 {
                if lx > 16 && lx < 52 { st.month_off -= 1; return true; }
                if lx > w - 50 && lx < w - 16 { st.month_off += 1; return true; }
            }
            false
        }
        AppId::Clock => {
            if ly > 178 && ly < 218 {
                if lx > w - 240 && lx < w - 144 {
                    if st.sw_run { st.sw_at = hw::uptime_secs().saturating_sub(st.sw_base); st.sw_run = false; }
                    else { st.sw_base = hw::uptime_secs(); st.sw_run = true; }
                    return true;
                }
                if lx > w - 134 && lx < w - 46 { st.sw_run = false; st.sw_at = 0; return true; }
            }
            false
        }
        AppId::Browser => {
            if ly > 42 && ly < 76 && lx > w - 100 && lx < w - 14 { st.editing = true; return true; }
            if st.editing && ly > 44 && ly < 80 && lx > w - 100 && lx < w - 14 {
                st.browser_key(13, 0);
                return true;
            }
            false
        }
        AppId::Notes => st.notes_click(lx, ly, x, y, w, h),
        AppId::Music => st.music_click(lx, ly, y, h),
        AppId::Camera => {
            if ly > h - 58 && ly < h - 16 && lx > w / 2 - 74 && lx < w / 2 + 74 {
                st.shoot = true;
                return true;
            }
            false
        }
        AppId::Contacts => st.lines_click(lx, ly, x, y, w, h, "\\home\\user\\Contacts\\contacts.txt"),
        AppId::Messages => st.lines_click(lx, ly, x, y, w, h, "\\home\\user\\Messages\\threads.txt"),
        AppId::Mail => st.lines_click(lx, ly, x, y, w, h, "\\home\\user\\Mail\\inbox.txt"),
        _ => false,
    }
}

unsafe fn seed_home() {
    ensure_dir(usr_vol(), "\\home\\user\\Notes");
    ensure_dir(usr_vol(), "\\home\\user\\Music");
    ensure_dir(usr_vol(), "\\home\\user\\Videos");
    ensure_dir(usr_vol(), "\\home\\user\\Contacts");
    ensure_dir(usr_vol(), "\\home\\user\\Messages");
    ensure_dir(usr_vol(), "\\home\\user\\Mail");
    ensure_dir(usr_vol(), "\\HYWOS\\SECURITY");
    let seeds = [
        ("\\home\\user\\Notes\\welcome.txt", "欢迎使用 HYWOS 备忘录\nCtrl+S 保存，Enter 换行。\n"),
        ("\\home\\user\\Contacts\\contacts.txt", "HYWOS 支持|10086|support@hywos.local\n"),
        ("\\home\\user\\Messages\\threads.txt", "系统|欢迎使用 HYWOS 1.1.0 信息|--:--\n"),
        ("\\home\\user\\Mail\\inbox.txt", "HYWOS 团队|欢迎使用 1.1.0|--:--\n"),
    ];
    let mut i = 0;
    while i < seeds.len() {
        if read_file(usr_vol(), seeds[i].0).is_none() {
            let _ = write_file(usr_vol(), seeds[i].0, seeds[i].1.as_bytes());
        }
        i += 1;
    }
}
