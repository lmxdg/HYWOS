use crate::efi::*;
use crate::fs::*;
use crate::gfx::*;
use crate::hw;
use crate::icon::{icon, AppId};

pub const HEADER_H: i32 = 34;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AppAction {
    None,
    Close,
    Screenshot,
    OpenUpdate,
    OpenApp(AppId),
}

pub struct Terminal {
    pub lines: [[u8; 92]; 120],
    pub lens: [u8; 120],
    pub n: usize,
    pub input: [u8; 76],
    pub ilen: usize,
    pub cwd: Str80,
    pub hist: usize,
}

impl Terminal {
    pub unsafe fn init(&mut self) {
        self.n = 0;
        self.ilen = 0;
        self.hist = 0;
        self.cwd = Str80::empty();
        self.cwd.push_str(HOME);
        self.print("HYWOS Shell 1.0.0 (x86_64 UEFI)");
        self.print("Type 'help' for a list of commands.");
        self.print("");
    }

    pub fn print(&mut self, s: &str) {
        if self.n >= 120 {
            let mut i = 1;
            while i < 120 {
                self.lines[i - 1] = self.lines[i];
                self.lens[i - 1] = self.lens[i];
                i += 1;
            }
            self.n = 119;
        }
        let line = &mut self.lines[self.n];
        let mut i = 0;
        for b in s.bytes() {
            if i < 90 { line[i] = b; i += 1; }
        }
        self.lens[self.n] = i as u8;
        self.n += 1;
    }

    pub fn prompt(&self) -> &'static str {
        "user@hywos:~$"
    }

    pub fn line_str(&self, i: usize) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.lines[i][..self.lens[i] as usize]) }
    }

    pub unsafe fn run(&mut self, cmd: &str) -> AppAction {
        let mut args: [&str; 4] = [""; 4];
        let mut na = 0;
        let mut start = 0usize;
        let b = cmd.as_bytes();
        let mut i = 0;
        while i <= b.len() {
            if i == b.len() || b[i] == b' ' {
                if i > start && na < 4 {
                    args[na] = core::str::from_utf8_unchecked(&b[start..i]);
                    na += 1;
                }
                start = i + 1;
            }
            i += 1;
        }
        if na == 0 { return AppAction::None; }
        let mut cwd_owned = Str80::empty();
        cwd_owned.push_str(self.cwd.as_str());
        let cwd = cwd_owned.as_str();
        match args[0] {
            "help" => {
                self.print("help      - show this help");
                self.print("ls [dir]  - list directory");
                self.print("cd <dir>  - change directory");
                self.print("pwd       - print working directory");
                self.print("cat <f>   - print file");
                self.print("echo <s>  - print text");
                self.print("mkdir <d> - create directory");
                self.print("rm <f>    - delete file");
                self.print("cp a b    - copy file");
                self.print("mv a b    - move file");
                self.print("df / mount- filesystem info");
                self.print("uname/date/ps/uptime/whoami");
                self.print("weather   - weather summary");
                self.print("screenshot- capture the screen");
                self.print("clear     - clear the terminal");
                self.print("exit      - close terminal window");
            }
            "clear" => { self.n = 0; }
            "pwd" => { let s = Str80::empty(); let _ = s; self.print(cwd); }
            "ls" => {
                let target = if na > 1 { path_join(cwd, args[1]) } else { let mut s = Str80::empty(); s.push_str(cwd); s };
                let mut ents: [DirEntry; 32] = core::mem::zeroed();
                let n = list_dir(usr_vol(), target.as_str(), &mut ents);
                if n == 0 { self.print("(empty or not found)"); }
                let mut k = 0;
                while k < n {
                    let e = &ents[k];
                    let mut buf = [0u8; 92];
                    let mut l = 0;
                    for c in e.name.b[..e.name.len].iter() {
                        if l < 88 { buf[l] = *c; l += 1; }
                    }
                    if e.is_dir && l < 90 { buf[l] = b'/'; l += 1; }
                    self.print(core::str::from_utf8_unchecked(&buf[..l]));
                    k += 1;
                }
            }
            "cd" => {
                let mut target = Str80::empty();
                if na < 2 || args[1] == "~" {
                    target.push_str(HOME);
                } else if args[1] == ".." {
                    let p = parent_of(cwd);
                    if p.len == 0 { target.push_str("\\"); } else { target.push_str(p.as_str()); }
                } else {
                    target = path_join(cwd, args[1]);
                }
                let mut ents: [DirEntry; 32] = core::mem::zeroed();
                let n = list_dir(usr_vol(), target.as_str(), &mut ents);
                if n == 0 && !exists_vol(usr_vol(), target.as_str()) {
                    self.print("cd: no such directory");
                } else {
                    self.cwd = target;
                }
            }
            "cat" => {
                if na < 2 { self.print("cat: missing file"); return AppAction::None; }
                let p = path_join(cwd, args[1]);
                match read_file(usr_vol(), p.as_str()) {
                    Some(n) => {
                        let buf = rdbuf();
                        let mut l = 0;
                        while l < n && l < 1200 {
                            let mut e = l;
                            while e < n && buf[e] != b'\n' && e - l < 88 { e += 1; }
                            self.print(core::str::from_utf8_unchecked(&buf[l..e]));
                            l = e + 1;
                        }
                    }
                    None => self.print("cat: cannot read file"),
                }
            }
            "echo" => {
                let mut buf = [0u8; 92];
                let mut l = 0;
                let mut k = 1;
                while k < na {
                    for c in args[k].bytes() { if l < 88 { buf[l] = c; l += 1; } }
                    if k + 1 < na && l < 88 { buf[l] = b' '; l += 1; }
                    k += 1;
                }
                self.print(core::str::from_utf8_unchecked(&buf[..l]));
            }
            "mkdir" => {
                if na < 2 { self.print("mkdir: missing name"); } else {
                    let p = path_join(cwd, args[1]);
                    ensure_dir(usr_vol(), p.as_str());
                    self.print("created directory");
                }
            }
            "rm" => {
                if na < 2 { self.print("rm: missing file"); } else {
                    let p = path_join(cwd, args[1]);
                    if delete_file(usr_vol(), p.as_str()) { self.print("removed"); } else { self.print("rm: failed"); }
                }
            }
            "cp" | "mv" => {
                if na < 3 { self.print("usage: cp <src> <dst>"); } else {
                    let a = path_join(cwd, args[1]);
                    let bb = path_join(cwd, args[2]);
                    match read_file(usr_vol(), a.as_str()) {
                        Some(n) => {
                            let buf = rdbuf();
                            let mut tmp = [0u8; 4096];
                            let l = if n < 4096 { n } else { 4096 };
                            let mut k = 0;
                            while k < l { tmp[k] = buf[k]; k += 1; }
                            if write_file(usr_vol(), bb.as_str(), &tmp[..l]) {
                                if args[0] == "mv" { delete_file(usr_vol(), a.as_str()); }
                                self.print("done");
                            } else { self.print("write failed"); }
                        }
                        None => self.print("cannot read source"),
                    }
                }
            }
            "uname" => self.print("HYWOS 1.0.0 x86_64 UEFI (Rust no_std kernel)"),
            "ps" => {
                self.print("  PID  CORE  STATE  NAME");
                self.print("    1     0  run    init");
                self.print("    2     1  run    compositor");
                self.print("    3     2  run    shell");
            }
            "free" => {
                self.print("Memory is managed by UEFI firmware pools.");
                self.print("Use the UEFI memory map for exact figures.");
            }
            "date" => {
                match hw::get_time() {
                    Some(t) => {
                        let mut b = [0u8; 32];
                        let mut l = 0;
                        l += hw::num2(&mut b[l..], t.year as u32, 4);
                        b[l] = b'-'; l += 1;
                        l += hw::num2(&mut b[l..], t.month as u32, 2);
                        b[l] = b'-'; l += 1;
                        l += hw::num2(&mut b[l..], t.day as u32, 2);
                        b[l] = b' '; l += 1;
                        l += hw::num2(&mut b[l..], t.hour as u32, 2);
                        b[l] = b':'; l += 1;
                        l += hw::num2(&mut b[l..], t.minute as u32, 2);
                        if l > 0 { self.print(core::str::from_utf8_unchecked(&b[..l])); }
                    }
                    None => self.print("RTC unavailable"),
                }
            }
            "df" => {
                let mut b = [0u8; 32];
                let mut l = 0;
                l += hw::num2(&mut b[l..], usr_vol() as u32, 1);
                self.print("volume  mounted  fs");
                self.print("HYWOS user volume is on a FAT filesystem.");
                let _ = b;
            }
            "mount" => {
                self.print("UEFI Simple File System volumes:");
                let mut k = 0;
                while k < vol_count() {
                    self.print("  volume mounted at SimpleFS#");
                    k += 1;
                }
            }
            "whoami" => self.print("user"),
            "uptime" => self.print("up 0 days (UEFI firmware uptime)"),
            "weather" => self.print("Weather app: 5-day offline forecast, C/F toggle."),
            "screenshot" => return AppAction::Screenshot,
            "appstore" => {
                self.print("appstore get <app> <key>");
                self.print("appstore set <app> <key> <value>");
                self.print("appstore del <app> <key>");
            }
            "exit" => return AppAction::Close,
            _ => {
                let mut b = [0u8; 92];
                let mut l = 0;
                for c in args[0].bytes() { if l < 70 { b[l] = c; l += 1; } }
                let tail = ": command not found";
                for c in tail.bytes() { if l < 90 { b[l] = c; l += 1; } }
                self.print(core::str::from_utf8_unchecked(&b[..l]));
            }
        }
        AppAction::None
    }

    pub unsafe fn key(&mut self, scan: u16, ch: u16) -> AppAction {
        if ch == 13 {
            let mut buf = [0u8; 76];
            let l = self.ilen;
            let mut i = 0;
            while i < l { buf[i] = self.input[i]; i += 1; }
            let cmd = core::str::from_utf8_unchecked(&buf[..l]);
            let mut p = [0u8; 92];
            let mut pl = 0;
            for c in self.prompt().bytes() { p[pl] = c; pl += 1; }
            p[pl] = b' '; pl += 1;
            for c in cmd.bytes() { if pl < 90 { p[pl] = c; pl += 1; } }
            let echo = core::str::from_utf8_unchecked(&p[..pl]);
            let mut e = [0u8; 92];
            let mut el = 0;
            for c in echo.bytes() { e[el] = c; el += 1; }
            self.print(core::str::from_utf8_unchecked(&e[..el]));
            let a = self.run(cmd);
            self.ilen = 0;
            return a;
        }
        if scan == 0x08 || ch == 8 {
            if self.ilen > 0 { self.ilen -= 1; }
            return AppAction::None;
        }
        if ch >= 32 && ch < 127 && self.ilen < 74 {
            self.input[self.ilen] = ch as u8;
            self.ilen += 1;
        }
        AppAction::None
    }
}

pub struct Settings {
    pub page: usize,
    pub msg: [u8; 72],
    pub msg_len: usize,
    pub msg_ok: bool,
    pub pending: [u8; 80],
    pub pending_len: usize,
}

pub struct Weather {
    pub city: usize,
    pub unit_f: bool,
    pub day: usize,
    pub frame: u32,
    pub loaded: bool,
}

#[derive(Clone, Copy)]
pub struct Forecast {
    pub hi: i32,
    pub lo: i32,
    pub cond: u8,
}

pub const CITIES: [&str; 5] = ["Beijing", "Shanghai", "Shenzhen", "Chengdu", "Harbin"];
pub const DAY_NAMES: [&str; 5] = ["TODAY", "TOMORROW", "WED", "THU", "FRI"];

pub static FORECAST: [[Forecast; 5]; 5] = [
    [
        Forecast { hi: 24, lo: 14, cond: 0 },
        Forecast { hi: 26, lo: 15, cond: 0 },
        Forecast { hi: 22, lo: 13, cond: 1 },
        Forecast { hi: 19, lo: 11, cond: 2 },
        Forecast { hi: 21, lo: 12, cond: 1 },
    ],
    [
        Forecast { hi: 27, lo: 19, cond: 1 },
        Forecast { hi: 28, lo: 20, cond: 0 },
        Forecast { hi: 25, lo: 18, cond: 2 },
        Forecast { hi: 24, lo: 17, cond: 2 },
        Forecast { hi: 26, lo: 18, cond: 1 },
    ],
    [
        Forecast { hi: 31, lo: 24, cond: 0 },
        Forecast { hi: 32, lo: 25, cond: 0 },
        Forecast { hi: 30, lo: 24, cond: 1 },
        Forecast { hi: 29, lo: 23, cond: 2 },
        Forecast { hi: 30, lo: 24, cond: 1 },
    ],
    [
        Forecast { hi: 23, lo: 17, cond: 2 },
        Forecast { hi: 22, lo: 16, cond: 2 },
        Forecast { hi: 24, lo: 17, cond: 1 },
        Forecast { hi: 26, lo: 18, cond: 0 },
        Forecast { hi: 25, lo: 18, cond: 1 },
    ],
    [
        Forecast { hi: 12, lo: 2, cond: 1 },
        Forecast { hi: 10, lo: 0, cond: 2 },
        Forecast { hi: 14, lo: 3, cond: 0 },
        Forecast { hi: 15, lo: 4, cond: 0 },
        Forecast { hi: 11, lo: 1, cond: 1 },
    ],
];

pub struct Store {
    pub sel: usize,
    pub installed: [bool; 6],
    pub msg: [u8; 64],
    pub msg_len: usize,
    pub msg_ok: bool,
}

pub const STORE_APPS: [(&str, &str, AppId, &str); 6] = [
    ("weather", "Weather", AppId::Weather, "Offline 5-day forecast with C/F units"),
    ("notes", "Notes", AppId::Notes, "Quick notes stored in your home volume"),
    ("calculator", "Calculator", AppId::Calculator, "A crisp calculator for daily math"),
    ("photos", "Photos", AppId::Photos, "Browse your Pictures folder"),
    ("music", "Music", AppId::Music, "Local audio player shell"),
    ("maps", "Maps", AppId::Maps, "Offline map canvas"),
];

pub struct AppState {
    pub term: Terminal,
    pub settings: Settings,
    pub weather: Weather,
    pub store: Store,
    pub files_cwd: Str80,
    pub files_sel: usize,
    pub files_click: usize,
    pub tick: u32,
}

pub unsafe fn init_state(st: &mut AppState, id: AppId) {
    st.tick = 0;
    st.files_sel = 0;
    st.files_click = 0;
    st.files_cwd = Str80::empty();
    st.files_cwd.push_str(HOME);
    st.settings.page = 0;
    st.settings.msg_len = 0;
    st.settings.msg_ok = false;
    st.settings.pending_len = 0;
    st.weather.city = 0;
    st.weather.day = 0;
    st.weather.frame = 0;
    st.weather.loaded = false;
    st.weather.unit_f = false;
    let mut ub = [0u8; 8];
    if let Some(n) = appstore_get("weather", "unit", &mut ub) {
        if n > 0 && ub[0] == b'f' { st.weather.unit_f = true; }
    }
    st.weather.loaded = true;
    st.store.sel = 0;
    st.store.msg_len = 0;
    st.store.msg_ok = false;
    let mut i = 0;
    while i < 6 {
        let mut v = [0u8; 8];
        st.store.installed[i] = appstore_get(STORE_APPS[i].0, "installed", &mut v).is_some();
        i += 1;
    }
    if id == AppId::Terminal {
        st.term.init();
    }
}

pub fn app_size(id: AppId) -> (i32, i32) {
    match id {
        AppId::Terminal => (720, 460),
        AppId::Files => (700, 440),
        AppId::Settings => (760, 500),
        AppId::Weather => (520, 430),
        AppId::Store => (680, 470),
        AppId::About => (620, 420),
        _ => (640, 420),
    }
}

fn shadow(g: &Gfx, x: i32, y: i32, w: i32, h: i32, r: i32) {
    let mut i = 7;
    while i > 0 {
        let a = (i * 5) as u8;
        g.round_rect_outline(x - i, y - i + 3, w + i * 2, h + i * 2, r + i, 0x000000, a);
        i -= 1;
    }
    g.rect(x + 6, y + h, w - 12, 2, 0x000000, 40);
}

fn card(g: &Gfx, x: i32, y: i32, w: i32, h: i32, r: i32) {
    shadow(g, x, y, w, h, r);
    g.round_rect(x, y, w, h, r, rgb(250, 251, 254), 255);
    g.round_rect(x, y, w, 1, 0, rgb(255, 255, 255), 255);
}

fn btn(g: &Gfx, x: i32, y: i32, w: i32, h: i32, label: &str, primary: bool) {
    let c = if primary { rgb(46, 120, 240) } else { rgb(228, 232, 240) };
    let tc = if primary { rgb(255, 255, 255) } else { rgb(48, 54, 68) };
    g.round_rect(x, y, w, h, h / 2, c, 255);
    g.text_center(x + w / 2, y + (h - 16) / 2, label, tc, 1);
}

fn toggle(g: &Gfx, x: i32, y: i32, on: bool) {
    let c = if on { rgb(48, 190, 110) } else { rgb(198, 204, 214) };
    g.round_rect(x, y, 40, 22, 11, c, 255);
    let knob = if on { x + 20 } else { x + 2 };
    g.circle(knob + 9, y + 11, 9, rgb(255, 255, 255), 255);
}

fn fmt_temp(v: i32, f: bool, buf: &mut [u8]) -> usize {
    let t = if f { v * 9 / 5 + 32 } else { v };
    let mut l = hw::num2(buf, t.unsigned_abs(), 1);
    if t < 0 { 
        let mut i = l;
        while i > 0 { buf[i] = buf[i - 1]; i -= 1; }
        buf[0] = b'-';
        l += 1;
    }
    l
}

fn sun_cloud(g: &Gfx, cx: i32, cy: i32, s: i32, frame: u32, cond: u8) {
    if cond == 2 {
        let drift = ((frame / 12) % 20) as i32 - 10;
        g.circle(cx - s / 3 + drift, cy, s / 3, rgb(212, 220, 232), 255);
        g.circle(cx + s / 4 + drift, cy - s / 6, s / 2, rgb(196, 206, 220), 255);
        g.round_rect(cx - s / 2 + drift, cy + s / 8, s, s / 3, s / 6, rgb(196, 206, 220), 255);
        let mut i = 0;
        while i < 3 {
            g.rect(cx - s / 3 + drift + i * s / 4, cy + s / 2, 3, s / 3, rgb(110, 160, 230), 220);
            i += 1;
        }
    } else {
        let bob = (sinf(frame as f32 * 0.06) * 4.0) as i32;
        g.circle(cx - s / 5, cy + bob, s / 2, rgb(255, 206, 84), 255);
        g.circle(cx - s / 5, cy + bob, s / 3, rgb(255, 232, 140), 255);
        if cond == 1 {
            let drift = ((frame / 10) % 26) as i32 - 13;
            g.circle(cx + s / 2 + drift, cy + s / 4, s / 3, rgb(230, 236, 246), 255);
            g.circle(cx + s / 5 + drift, cy + s / 5, s / 2, rgb(240, 244, 252), 255);
            g.round_rect(cx - s / 6 + drift, cy + s / 3, s, s / 3, s / 6, rgb(240, 244, 252), 255);
        }
    }
}

pub unsafe fn draw(st: &mut AppState, id: AppId, g: &Gfx, x: i32, y: i32, w: i32, h: i32, focus: bool) {
    card(g, x, y, w, h, 14);
    let title_c = if focus { rgb(236, 240, 248) } else { rgb(228, 231, 238) };
    g.round_rect(x, y, w, HEADER_H, 14, title_c, 255);
    g.rect(x, y + HEADER_H - 12, w, 12, title_c, 255);
    if focus {
        g.circle(x + 18, y + HEADER_H / 2, 6, rgb(255, 96, 86), 255);
        g.circle(x + 36, y + HEADER_H / 2, 6, rgb(250, 196, 84), 255);
        g.circle(x + 54, y + HEADER_H / 2, 6, rgb(82, 200, 112), 255);
    }
    g.text_center(x + w / 2, y + 9, id.name(), rgb(52, 58, 72), 1);
    st.tick += 1;
    match id {
        AppId::Terminal => draw_term(&mut st.term, g, x, y, w, h),
        AppId::Files => draw_files(st, g, x, y, w, h),
        AppId::Settings => draw_settings(&mut st.settings, g, x, y, w, h),
        AppId::Weather => draw_weather(&mut st.weather, g, x, y, w, h),
        AppId::Store => draw_store(&mut st.store, g, x, y, w, h),
        AppId::About => draw_about(g, x, y, w, h),
        _ => draw_generic(g, id, x, y, w, h),
    }
}

fn draw_term(t: &mut Terminal, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let ix = x + 12;
    let iy = y + HEADER_H + 8;
    let iw = w - 24;
    let ih = h - HEADER_H - 20;
    g.round_rect(ix, iy, iw, ih, 8, rgb(16, 20, 28), 255);
    let rows = (ih - 34) / 16;
    let start = if t.n as i32 > rows { t.n as i32 - rows } else { 0 };
    let mut i = start as usize;
    let mut row = 0;
    while i < t.n {
        let s = t.line_str(i);
        let col = if s.starts_with("user@hywos") { rgb(120, 220, 160) } else { rgb(200, 214, 236) };
        g.text(ix + 10, iy + 10 + row * 16, s, col, 1);
        row += 1;
        i += 1;
    }
    let py = iy + ih - 24;
    g.text(ix + 10, py, t.prompt(), rgb(120, 220, 160), 1);
    let px = ix + 10 + t.prompt().len() as i32 * 8 + 8;
    let input = unsafe { core::str::from_utf8_unchecked(&t.input[..t.ilen]) };
    g.text(px, py, input, rgb(240, 246, 255), 1);
    if (t.tick_blink() / 20) % 2 == 0 {
        g.rect(px + input.len() as i32 * 8, py, 7, 15, rgb(160, 210, 255), 200);
    }
}

impl Terminal {
    fn tick_blink(&self) -> u32 {
        unsafe { TICK }
    }
}

pub static mut TICK: u32 = 0;

fn draw_files(st: &mut AppState, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let top = y + HEADER_H + 8;
    let side = 168;
    g.round_rect(x + 10, top, side, h - HEADER_H - 24, 8, rgb(238, 241, 247), 255);
    let places = ["Home", "Documents", "Pictures", "Screenshots", "App Data"];
    let mut i = 0;
    while i < 5 {
        let py = top + 14 + i as i32 * 30;
        if i == st.files_sel.min(4) {
            g.round_rect(x + 18, py - 6, side - 16, 26, 6, rgb(214, 226, 248), 255);
        }
        g.text(x + 28, py, places[i], rgb(56, 64, 84), 1);
        i += 1;
    }
    let lx = x + side + 24;
    let lw = w - side - 40;
    let mut ents: [DirEntry; 32] = unsafe { core::mem::zeroed() };
    let n = unsafe { list_dir(usr_vol(), st.files_cwd.as_str(), &mut ents) };
    g.text(lx, top + 2, st.files_cwd.as_str(), rgb(120, 128, 146), 1);
    let mut k = 0;
    while k < n && k < 13 {
        let ey = top + 26 + k as i32 * 26;
        if k == st.files_click {
            g.round_rect(lx - 6, ey - 5, lw, 24, 5, rgb(222, 232, 250), 255);
        }
        let name = ents[k].name.as_str();
        if ents[k].is_dir {
            g.round_rect(lx, ey, 14, 11, 3, rgb(250, 196, 84), 255);
            g.text(lx + 22, ey, name, rgb(48, 56, 76), 1);
        } else {
            g.round_rect(lx, ey, 11, 14, 2, rgb(150, 170, 200), 255);
            let col = if name.ends_with(".updpkg") { rgb(196, 72, 66) } else { rgb(72, 82, 102) };
            g.text(lx + 22, ey, name, col, 1);
        }
        k += 1;
    }
    if n == 0 {
        g.text(lx, top + 26, "(empty)", rgb(150, 158, 176), 1);
    }
    g.text(lx, y + h - 24, "Double-click a .updpkg to open Software Update", rgb(140, 148, 168), 1);
}

fn draw_settings(s: &mut Settings, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let top = y + HEADER_H + 8;
    let side = 190;
    g.round_rect(x + 10, top, side, h - HEADER_H - 24, 8, rgb(238, 241, 247), 255);
    let pages = ["Display", "Permissions", "Software Update", "About"];
    let mut i = 0;
    while i < 4 {
        let py = top + 12 + i as i32 * 34;
        if s.page == i {
            g.round_rect(x + 18, py - 7, side - 16, 30, 7, rgb(214, 226, 248), 255);
            g.text(x + 30, py, pages[i], rgb(36, 92, 200), 1);
        } else {
            g.text(x + 30, py, pages[i], rgb(62, 70, 90), 1);
        }
        i += 1;
    }
    let cx = x + side + 26;
    let cy = top + 6;
    let cw = w - side - 40;
    match s.page {
        0 => {
            g.text(cx, cy, "Display", rgb(40, 46, 60), 2);
            g.text(cx, cy + 42, "Resolution", rgb(130, 138, 156), 1);
            let mut b = [0u8; 24];
            let mut l = hw::num2(&mut b, crate::gfx::Gfx::screen_w() as u32, 1);
            b[l] = b'x'; l += 1;
            l += hw::num2(&mut b[l..], crate::gfx::Gfx::screen_h() as u32, 1);
            g.text(cx, cy + 62, unsafe { core::str::from_utf8_unchecked(&b[..l]) }, rgb(50, 56, 72), 1);
            g.text(cx, cy + 92, "Compositor", rgb(130, 138, 156), 1);
            g.text(cx, cy + 112, "Software 32bpp, 4x supersampled icons", rgb(50, 56, 72), 1);
            btn(g, cx, cy + 150, 190, 34, "Take screenshot", true);
            g.text(cx, cy + 200, "F12 or Print Screen also captures.", rgb(140, 148, 168), 1);
            if s.msg_len > 0 {
                let m = unsafe { core::str::from_utf8_unchecked(&s.msg[..s.msg_len]) };
                g.text(cx, cy + 222, m, if s.msg_ok { rgb(40, 160, 90) } else { rgb(190, 70, 66) }, 1);
            }
        }
        1 => {
            g.text(cx, cy, "Permissions", rgb(40, 46, 60), 2);
            g.text(cx, cy + 34, "Per-app access control, stored in .apps/_permissions", rgb(130, 138, 156), 1);
            let apps = ["terminal", "files", "settings", "weather", "store", "about"];
            g.text(cx, cy + 66, "APP", rgb(150, 158, 176), 1);
            g.text(cx + 150, cy + 66, "STORAGE", rgb(150, 158, 176), 1);
            g.text(cx + 250, cy + 66, "NOTIFY", rgb(150, 158, 176), 1);
            g.text(cx + 340, cy + 66, "NETWORK", rgb(150, 158, 176), 1);
            let mut i = 0;
            while i < 6 {
                let ry = cy + 88 + i as i32 * 34;
                g.text(cx, ry, apps[i], rgb(50, 56, 72), 1);
                let mut k = 0;
                while k < 3 {
                    let on = unsafe { perm_get(apps[i], k as u8) };
                    toggle(g, cx + 150 + k as i32 * 100, ry - 3, on);
                    k += 1;
                }
                i += 1;
            }
            g.text(cx, cy + 330, "Storage off makes appstore_set/get/del return denied.", rgb(150, 158, 176), 1);
        }
        2 => {
            g.text(cx, cy, "Software Update", rgb(40, 46, 60), 2);
            g.text(cx, cy + 36, "Package format: CONLYUPD, CRC32 protected", rgb(130, 138, 156), 1);
            g.text(cx, cy + 62, "Target: LBA 64 kernel region only", rgb(130, 138, 156), 1);
            let p = if s.pending_len > 0 {
                unsafe { core::str::from_utf8_unchecked(&s.pending[..s.pending_len]) }
            } else {
                "(no package selected)"
            };
            g.text(cx, cy + 92, p, rgb(50, 56, 72), 1);
            btn(g, cx, cy + 126, 170, 34, "Apply update", true);
            btn(g, cx + 190, cy + 126, 150, 34, "Check device", false);
            let mut dev = [0u8; 40];
            let mut dl = 0;
            match unsafe { block_info() } {
                Some((blocks, bs)) => {
                    for c in b"Kernel device: ".iter() { dev[dl] = *c; dl += 1; }
                    dl += hw::num2(&mut dev[dl..], blocks as u32, 1);
                    for c in b" blocks x ".iter() { dev[dl] = *c; dl += 1; }
                    dl += hw::num2(&mut dev[dl..], bs, 1);
                }
                None => {
                    for c in b"Kernel device: none".iter() { dev[dl] = *c; dl += 1; }
                }
            }
            g.text(cx, cy + 190, unsafe { core::str::from_utf8_unchecked(&dev[..dl]) }, rgb(120, 128, 148), 1);
            if s.msg_len > 0 {
                let m = unsafe { core::str::from_utf8_unchecked(&s.msg[..s.msg_len]) };
                g.text(cx, cy + 220, m, if s.msg_ok { rgb(40, 160, 90) } else { rgb(190, 70, 66) }, 1);
            }
            g.text(cx, cy + 262, "User files, apps and settings are never touched.", rgb(150, 158, 176), 1);
        }
        _ => draw_about(g, x + side + 16, top, w - side - 26, h - HEADER_H - 24),
    }
}

fn draw_weather(wx: &mut Weather, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    wx.frame += 1;
    let ix = x + 14;
    let iy = y + HEADER_H + 6;
    let iw = w - 28;
    let ih = h - HEADER_H - 20;
    g.round_rect(ix, iy, iw, ih, 10, rgb(48, 112, 210), 255);
    g.vgrad(ix + 1, iy + 1, iw - 2, ih / 2, rgb(86, 150, 236), rgb(56, 116, 206));
    g.vgrad(ix + 1, iy + ih / 2, iw - 2, ih / 2 - 1, rgb(56, 116, 206), rgb(38, 86, 172));
    let f = wx.frame;
    sun_cloud(g, ix + iw - 92, iy + 74, 44, f, 1);
    let city = CITIES[wx.city];
    g.text(ix + 18, iy + 14, city, rgb(240, 248, 255), 2);
    let cur = FORECAST[wx.city][wx.day];
    let mut tb = [0u8; 12];
    let tl = fmt_temp(cur.hi, wx.unit_f, &mut tb);
    g.text(ix + 18, iy + 44, unsafe { core::str::from_utf8_unchecked(&tb[..tl]) }, rgb(255, 255, 255), 3);
    let unit = if wx.unit_f { "F" } else { "C" };
    g.text(ix + 108, iy + 56, unit, rgb(210, 228, 255), 2);
    g.round_rect(ix + iw - 88, iy + 14, 74, 30, 15, rgb(255, 255, 255), 40);
    g.text_center(ix + iw - 51, iy + 21, if wx.unit_f { "F / C" } else { "C / F" }, rgb(255, 255, 255), 1);
    let conf = match cur.cond {
        0 => "Clear sky",
        1 => "Partly cloudy",
        _ => "Light rain",
    };
    g.text(ix + 18, iy + 96, conf, rgb(214, 232, 255), 1);
    let rows_y = iy + 120;
    let mut i = 0;
    while i < 5 {
        let ry = rows_y + i as i32 * 34;
        let fc = FORECAST[wx.city][i];
        if i == wx.day {
            g.round_rect(ix + 10, ry - 4, iw - 20, 30, 8, rgb(255, 255, 255), 42);
            g.rect(ix + 10, ry - 4, 3, 30, rgb(255, 232, 120), 255);
        }
        g.text(ix + 22, ry + 2, DAY_NAMES[i], rgb(236, 245, 255), 1);
        let mut hb = [0u8; 12];
        let hl = fmt_temp(fc.hi, wx.unit_f, &mut hb);
        let mut lb = [0u8; 12];
        let ll = fmt_temp(fc.lo, wx.unit_f, &mut lb);
        g.text(ix + iw - 150, ry + 2, unsafe { core::str::from_utf8_unchecked(&hb[..hl]) }, rgb(255, 255, 255), 1);
        g.text(ix + iw - 96, ry + 2, unsafe { core::str::from_utf8_unchecked(&lb[..ll]) }, rgb(184, 210, 246), 1);
        sun_cloud(g, ix + iw - 210, ry + 10, 22, f + i as u32 * 7, fc.cond);
        i += 1;
    }
    g.text(ix + 18, iy + ih - 24, "Offline 5-day forecast. Units persist via app store KV.", rgb(202, 224, 252), 1);
}

fn draw_store(s: &mut Store, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let top = y + HEADER_H + 10;
    let cw = (w - 60) / 2;
    let ch = 96;
    let mut i = 0;
    while i < 6 {
        let col = (i % 2) as i32;
        let row = (i / 2) as i32;
        let cx = x + 20 + col * (cw + 16);
        let cy = top + row * (ch + 8);
        let selected = s.sel == i;
        g.round_rect(cx, cy, cw, ch, 10, if selected { rgb(226, 236, 252) } else { rgb(242, 245, 250) }, 255);
        if selected { g.round_rect_outline(cx, cy, cw, ch, 10, rgb(90, 150, 240), 200); }
        let ic = unsafe { icon(STORE_APPS[i].2) };
        g.blit_rgba(cx + 12, cy + 16, 44, 44, ic, 64, 64);
        g.text(cx + 66, cy + 14, STORE_APPS[i].1, rgb(44, 52, 68), 1);
        let desc = STORE_APPS[i].3;
        let mut dl = desc.len() as i32;
        if dl > 30 { dl = 30; }
        g.text(cx + 66, cy + 34, &desc[..dl as usize], rgb(130, 138, 156), 1);
        let label = if s.installed[i] { "Installed" } else { "Install" };
        btn(g, cx + 66, cy + 54, 86, 26, label, !s.installed[i]);
        i += 1;
    }
    if s.msg_len > 0 {
        let m = unsafe { core::str::from_utf8_unchecked(&s.msg[..s.msg_len]) };
        g.text(x + 24, y + h - 28, m, if s.msg_ok { rgb(40, 160, 90) } else { rgb(190, 70, 66) }, 1);
    }
}

fn draw_about(g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let cx = x + w / 2;
    let ic = unsafe { icon(AppId::About) };
    let s = if w < 400 { 76 } else { 96 };
    g.blit_rgba(cx - s / 2, y + 18, s, s, ic, 64, 64);
    g.text_center(cx, y + 18 + s + 8, "HYWOS", rgb(30, 38, 56), 3);
    g.text_center(cx, y + 18 + s + 62, "HYWOS 1.0.0 \"Harmony of Stars\"", rgb(110, 120, 140), 1);
    let lines = [
        "Kernel      HYWOS 1.0.0 (Rust, no_std UEFI)",
        "Architecture x86_64 / ACPI SMP topology",
        "Graphics    GOP software compositor",
        "License     MIT",
        "Source      github.com/lmxdg/HYWOS",
    ];
    let mut i = 0;
    while i < 5 {
        g.text(x + 24, y + 18 + s + 74 + i as i32 * 20, lines[i], rgb(72, 82, 100), 1);
        i += 1;
    }
    let c = hw::cpu();
    let mut b = [0u8; 40];
    let mut l = 0;
    for ch in b"Cores       ".iter() { b[l] = *ch; l += 1; }
    l += hw::num2(&mut b[l..], c.enabled as u32, 1);
    for ch in b" / ".iter() { b[l] = *ch; l += 1; }
    l += hw::num2(&mut b[l..], c.total as u32, 1);
    let _ = h;
    g.text(x + 24, y + 18 + s + 74 + 5 * 20, unsafe { core::str::from_utf8_unchecked(&b[..l]) }, rgb(72, 82, 100), 1);
}

fn draw_generic(g: &Gfx, id: AppId, x: i32, y: i32, w: i32, h: i32) {
    let ic = unsafe { icon(id) };
    let s = 88;
    g.blit_rgba(x + w / 2 - s / 2, y + HEADER_H + 30, s, s, ic, 64, 64);
    g.text_center(x + w / 2, y + HEADER_H + 30 + s + 16, id.name(), rgb(40, 48, 64), 2);
    g.text_center(x + w / 2, y + HEADER_H + 30 + s + 50, "Part of the HYWOS application suite", rgb(130, 138, 156), 1);
    g.text_center(x + w / 2, y + HEADER_H + 30 + s + 70, "Runtime available in a future update", rgb(160, 168, 184), 1);
}

pub unsafe fn click(st: &mut AppState, id: AppId, mx: i32, my: i32, x: i32, y: i32, w: i32, h: i32) -> AppAction {
    let lx = mx - x;
    let ly = my - y;
    if ly < HEADER_H && lx > 10 && lx < 66 {
        return AppAction::Close;
    }
    match id {
        AppId::Settings => {
            let top = HEADER_H + 8;
            let side = 190;
            if lx > 10 && lx < 10 + side && ly > top {
                let idx = ((ly - top - 12 + 7) / 34) as usize;
                if idx < 4 { st.settings.page = idx; }
                return AppAction::None;
            }
            if st.settings.page == 0 {
                let bx = w - side - 40 - 40;
                if ly > top + 140 && ly < top + 190 && lx > side + 26 && lx < side + 26 + 190 {
                    return AppAction::Screenshot;
                }
            } else if st.settings.page == 1 {
                let cx = side + 26;
                if lx > cx + 140 && lx <= cx + 420 {
                    let k = ((lx - cx - 140) / 100).min(2) as u8;
                    let row = ((ly - (top + 88) + 6) / 34).max(0) as usize;
                    let apps = ["terminal", "files", "settings", "weather", "store", "about"];
                    if row < 6 {
                        let cur = perm_get(apps[row], k);
                        perm_set(apps[row], k, !cur);
                    }
                    return AppAction::None;
                }
            } else if st.settings.page == 2 {
                let cx = side + 26;
                if ly > top + 120 && ly < top + 160 && lx > cx && lx < cx + 170 {
                    if st.settings.pending_len > 0 {
                        let mut pathbuf = [0u8; 84];
                        let pl = st.settings.pending_len;
                        let mut i = 0;
                        while i < pl { pathbuf[i] = st.settings.pending[i]; i += 1; }
                        let p = core::str::from_utf8_unchecked(&pathbuf[..pl]);
                        let res = match read_file(usr_vol(), p) {
                            Some(n) => {
                                let buf = rdbuf();
                                let mut tmp = [0u8; 33000];
                                let l = if n < 33000 { n } else { 33000 };
                                let mut i = 0;
                                while i < l { tmp[i] = buf[i]; i += 1; }
                                apply_update(&tmp[..l])
                            }
                            None => 4,
                        };
                        let msg: &str = match res {
                            0 => "Update applied to LBA 64. Reboot to use it.",
                            1 => "Invalid CONLYUPD header.",
                            2 => "CRC32 mismatch. Update rejected.",
                            3 => "No writable kernel block device.",
                            _ => "Package not found on the user volume.",
                        };
                        st.settings.msg_len = msg.len().min(70);
                        let mut i = 0;
                        for c in msg.bytes() { if i < 70 { st.settings.msg[i] = c; i += 1; } }
                        st.settings.msg_ok = res == 0;
                    }
                    return AppAction::None;
                }
                if ly > top + 120 && ly < top + 160 && lx > cx + 180 && lx < cx + 340 {
                    let b = find_block_device();
                    let msg = if b.is_null() { "No writable block device found." } else { "Writable kernel device detected." };
                    st.settings.msg_len = msg.len();
                    let mut i = 0;
                    for c in msg.bytes() { st.settings.msg[i] = c; i += 1; }
                    st.settings.msg_ok = !b.is_null();
                }
            }
        }
        AppId::Weather => {
            let ix = x + 14;
            let iy = y + HEADER_H + 6;
            let iw = w - 28;
            if mx > ix + iw - 88 && mx < ix + iw - 14 && my > iy + 14 && my < iy + 44 {
                st.weather.unit_f = !st.weather.unit_f;
                let u: &[u8] = if st.weather.unit_f { b"f" } else { b"c" };
                let _ = appstore_set("weather", "unit", core::str::from_utf8_unchecked(u));
                return AppAction::None;
            }
            let rows_y = iy + 120;
            let mut i = 0;
            while i < 5 {
                let ry = rows_y + i as i32 * 34;
                if my > ry - 6 && my < ry + 28 && mx > ix + 8 && mx < ix + iw - 8 {
                    st.weather.day = i;
                }
                i += 1;
            }
        }
        AppId::Store => {
            let top = HEADER_H + 10;
            let cw = (w - 60) / 2;
            let ch = 108;
            let mut i = 0;
            while i < 6 {
                let col = (i % 2) as i32;
                let row = (i / 2) as i32;
                let cx = 20 + col * (cw + 16);
                let cy = top + row * (ch + 8);
                if lx > cx && lx < cx + cw && ly > cy && ly < cy + ch {
                    st.store.sel = i;
                    if lx > cx + 60 && ly > cy + 50 && ly < cy + 84 {
                        if st.store.installed[i] {
                            st.store.msgl("Already installed: " , STORE_APPS[i].1);
                        } else if appstore_set(STORE_APPS[i].0, "installed", "1") {
                            st.store.installed[i] = true;
                            st.store.msgl("Installed ", STORE_APPS[i].1);
                            st.store.msg_ok = true;
                        } else {
                            st.store.msgl("Denied: storage permission off for ", STORE_APPS[i].0);
                            st.store.msg_ok = false;
                        }
                    }
                }
                i += 1;
            }
            return AppAction::None;
        }
        AppId::Files => {
            let top = HEADER_H + 8;
            let side = 168;
            if lx < 10 + side && ly > top {
                let idx = ((ly - top - 14 + 6) / 30) as usize;
                let dirs = ["\\home\\user", "\\home\\user\\Documents", "\\home\\user\\Pictures", "\\home\\user\\Screenshots", "\\home\\user\\.apps"];
                if idx < 5 {
                    st.files_sel = idx;
                    st.files_cwd = Str80::empty();
                    st.files_cwd.push_str(dirs[idx]);
                    st.files_click = 0;
                }
                return AppAction::None;
            }
            let mut ents: [DirEntry; 32] = core::mem::zeroed();
            let n = list_dir(usr_vol(), st.files_cwd.as_str(), &mut ents);
            let lx2 = side + 24;
            let idx = ((ly - top - 26 + 5) / 26).max(0) as usize;
            if ly > top + 20 && idx < n {
                let same = st.files_click == idx;
                st.files_click = idx;
                if same {
                    if ents[idx].is_dir {
                        let joined = path_join(st.files_cwd.as_str(), ents[idx].name.as_str());
                        st.files_cwd = joined;
                        st.files_click = 0;
                    } else {
                        let name = ents[idx].name.as_str();
                        if name.ends_with(".updpkg") {
                            let full = path_join(st.files_cwd.as_str(), name);
                            select_update(st, full.as_str());
                            return AppAction::OpenUpdate;
                        }
                    }
                }
                let _ = lx2;
            }
            return AppAction::None;
        }
        _ => {}
    }
    AppAction::None
}

impl Store {
    fn msgl(&mut self, a: &str, b: &str) {
        self.msg_len = 0;
        self.msg_ok = true;
        for c in a.bytes() { if self.msg_len < 60 { self.msg[self.msg_len] = c; self.msg_len += 1; } }
        for c in b.bytes() { if self.msg_len < 60 { self.msg[self.msg_len] = c; self.msg_len += 1; } }
    }
}

pub unsafe fn key(st: &mut AppState, id: AppId, scan: u16, ch: u16) -> AppAction {
    if id == AppId::Terminal {
        return st.term.key(scan, ch);
    }
    if id == AppId::Settings && st.settings.page == 2 && scan == 0x16 {
        return AppAction::OpenUpdate;
    }
    if id == AppId::Weather && ch == b'u' as u16 {
        st.weather.unit_f = !st.weather.unit_f;
    }
    AppAction::None
}

pub unsafe fn select_update(st: &mut AppState, name: &str) {
    st.settings.page = 2;
    st.settings.pending_len = 0;
    for c in name.bytes() {
        if st.settings.pending_len < 78 {
            st.settings.pending[st.settings.pending_len] = c;
            st.settings.pending_len += 1;
        }
    }
}

pub unsafe fn set_message(st: &mut AppState, ok: bool, msg: &str) {
    st.settings.msg_len = 0;
    st.settings.msg_ok = ok;
    for c in msg.bytes() {
        if st.settings.msg_len < 70 {
            st.settings.msg[st.settings.msg_len] = c;
            st.settings.msg_len += 1;
        }
    }
}