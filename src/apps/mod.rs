use crate::efi::*;
use crate::fs::*;
use crate::gfx::*;
use crate::hw;
use crate::icon::{icon, AppId};

pub mod suite;

pub const HEADER_H: i32 = 40;

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
        self.print("HYWOS 终端 1.1.1 (x86_64 UEFI)");
        self.print("输入 help 查看可用命令，支持中文显示。");
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
                self.print("help        显示本帮助");
                self.print("ls [目录]   列出目录内容");
                self.print("cd <目录>   切换目录");
                self.print("pwd         显示当前路径");
                self.print("cat <文件>  查看文件");
                self.print("echo <文本> 输出文本");
                self.print("mkdir <目录> 新建目录");
                self.print("rm <文件>   删除文件");
                self.print("cp a b      复制文件");
                self.print("mv a b      移动文件");
                self.print("df / mount  文件系统信息");
                self.print("uname date ps uptime whoami");
                self.print("weather     天气摘要");
                self.print("net         网卡驱动与链路状态");
                self.print("ping <IP>   ARP/ICMP 连通性测试");
                self.print("screenshot  截取屏幕");
                self.print("clear       清屏");
                self.print("exit        关闭终端窗口");
            }
            "clear" => { self.n = 0; }
            "pwd" => { let s = Str80::empty(); let _ = s; self.print(cwd); }
            "ls" => {
                let target = if na > 1 { path_join(cwd, args[1]) } else { let mut s = Str80::empty(); s.push_str(cwd); s };
                let mut ents: [DirEntry; 32] = core::mem::zeroed();
                let n = list_dir(usr_vol(), target.as_str(), &mut ents);
                if n == 0 { self.print("（空目录或不存在）"); }
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
                    self.print("cd：目录不存在");
                } else {
                    self.cwd = target;
                }
            }
            "cat" => {
                if na < 2 { self.print("cat：缺少文件名"); return AppAction::None; }
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
                if na < 2 { self.print("mkdir：缺少目录名"); } else {
                    let p = path_join(cwd, args[1]);
                    ensure_dir(usr_vol(), p.as_str());
                    self.print("已创建目录");
                }
            }
            "rm" => {
                if na < 2 { self.print("rm：缺少文件名"); } else {
                    let p = path_join(cwd, args[1]);
                    if delete_file(usr_vol(), p.as_str()) { self.print("已删除"); } else { self.print("rm：删除失败"); }
                }
            }
            "cp" | "mv" => {
                if na < 3 { self.print("用法：cp <源> <目标>"); } else {
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
                                self.print("完成");
                            } else { self.print("写入失败"); }
                        }
                        None => self.print("cannot read source"),
                    }
                }
            }
            "uname" => self.print("HYWOS 1.1.1 x86_64 UEFI (Rust no_std kernel)"),
            "net" => {
                let n = crate::net::info();
                self.print(if n.present { "自研网卡驱动" } else { "未绑定自研驱动" });
                if n.present {
                    self.print(n.name);
                    let mut b = [0u8; 32];
                    let l = crate::net::fmt_mac(&mut b);
                    let mut line = [0u8; 64];
                    let mut ll = 0;
                    for c in "MAC ".bytes() { line[ll] = c; ll += 1; }
                    let mut i = 0;
                    while i < l { line[ll] = b[i]; ll += 1; i += 1; }
                    for c in " 链路 ".bytes() { line[ll] = c; ll += 1; }
                    for c in (if n.link { "已连接" } else { "断开" }).bytes() { line[ll] = c; ll += 1; }
                    self.print(core::str::from_utf8_unchecked(&line[..ll]));
                    let mut b2 = [0u8; 48];
                    let l1 = crate::net::fmt_ip(&mut b2, n.ip);
                    let mut line2 = [0u8; 96];
                    let mut k = 0;
                    for c in "IP ".bytes() { line2[k] = c; k += 1; }
                    let mut i = 0;
                    while i < l1 { line2[k] = b2[i]; k += 1; i += 1; }
                    for c in "  网关 ".bytes() { line2[k] = c; k += 1; }
                    let mut b3 = [0u8; 24];
                    let l2 = crate::net::fmt_ip(&mut b3, n.gw);
                    i = 0;
                    while i < l2 { line2[k] = b3[i]; k += 1; i += 1; }
                    self.print(core::str::from_utf8_unchecked(&line2[..k]));
                    let mut line3 = [0u8; 64];
                    let mut k2 = 0;
                    for c in "收包 ".bytes() { line3[k2] = c; k2 += 1; }
                    k2 += crate::hw::num2(&mut line3[k2..], n.rx, 1);
                    for c in "  发包 ".bytes() { line3[k2] = c; k2 += 1; }
                    k2 += crate::hw::num2(&mut line3[k2..], n.tx, 1);
                    self.print(core::str::from_utf8_unchecked(&line3[..k2]));
                } else if crate::hw::net().present {
                    self.print("固件 SNP 可用，HTTP 由固件提供");
                }
            }
            "ping" => {
                if na < 2 {
                    self.print("用法: ping <IP>，例如 ping 10.0.2.2");
                } else if let Some(ip) = crate::net::parse_ip(args[1]) {
                    let ok = crate::net::ping(ip, 1500);
                    let mut b = [0u8; 64];
                    let mut l = 0;
                    for c in "PING ".bytes() { b[l] = c; l += 1; }
                    let mut i = 0;
                    while i < 4 {
                        l += crate::hw::num2(&mut b[l..], ip[i] as u32, 1);
                        if i < 3 { b[l] = b'.'; l += 1; }
                        i += 1;
                    }
                    for c in (if ok { ": 回复正常，链路可达" } else { ": 无回复（超时或未配置）" }).bytes() { b[l] = c; l += 1; }
                    self.print(core::str::from_utf8_unchecked(&b[..l]));
                } else {
                    self.print("IP 格式错误");
                }
            }
            "ps" => {
                self.print("  PID  CORE  STATE  NAME");
                self.print("    1     0  run    init");
                self.print("    2     1  run    compositor");
                self.print("    3     2  run    shell");
            }
            "free" => {
                self.print("内存由 UEFI 固件池管理。");
                self.print("精确数值请参阅 UEFI 内存映射。");
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
                self.print("卷      挂载    文件系统");
                self.print("用户卷为 FAT 文件系统。");
                let _ = b;
            }
            "mount" => {
                self.print("UEFI 简单文件系统卷：");
                let mut k = 0;
                while k < vol_count() {
                    self.print("  卷已挂载 #");
                    k += 1;
                }
            }
            "whoami" => self.print("user"),
            "uptime" => self.print("up 0 days (UEFI firmware uptime)"),
            "weather" => self.print("Weather app: 5-day offline forecast, C/F toggle."),
            "screenshot" => return AppAction::Screenshot,
            "appstore" => {
                self.print("appstore get <应用> <键>");
                self.print("appstore set <应用> <键> <值>");
                self.print("appstore del <应用> <键>");
            }
            "exit" => return AppAction::Close,
            _ => {
                let mut b = [0u8; 92];
                let mut l = 0;
                for c in args[0].bytes() { if l < 70 { b[l] = c; l += 1; } }
                let tail = "：命令未找到";
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
    pub update_state: u8,
    pub release: crate::updater::Release,
    pub update_msg: [u8; 96],
    pub update_msg_len: usize,
    pub update_ok: bool,
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

pub const CITIES: [&str; 5] = ["北京", "上海", "深圳", "成都", "哈尔滨"];
pub const DAY_NAMES: [&str; 5] = ["今天", "明天", "周三", "周四", "周五"];

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
    ("weather", "天气", AppId::Weather, "离线 5 日天气"),
    ("notes", "备忘录", AppId::Notes, "随手记录"),
    ("calculator", "计算器", AppId::Calculator, "日常计算"),
    ("photos", "照片", AppId::Photos, "浏览图片"),
    ("music", "音乐", AppId::Music, "本地音乐"),
    ("maps", "地图", AppId::Maps, "离线地图"),
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
    pub suite: suite::Suite,
}

pub unsafe fn init_state(st: &mut AppState, id: AppId) {
    st.tick = 0;
    st.suite.init(id);
    st.files_sel = 0;
    st.files_click = 0;
    st.files_cwd = Str80::empty();
    st.files_cwd.push_str(HOME);
    st.settings.page = 0;
    st.settings.msg_len = 0;
    st.settings.msg_ok = false;
    st.settings.pending_len = 0;
    st.settings.update_state = 0;
    st.settings.release = crate::updater::Release::empty();
    st.settings.update_msg_len = 0;
    st.settings.update_ok = false;
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
        AppId::Security => (720, 520),
        AppId::Calculator => (420, 460),
        AppId::Calendar => (620, 500),
        AppId::Clock => (720, 440),
        AppId::Browser => (720, 520),
        AppId::Notes => (760, 500),
        AppId::Photos => (760, 520),
        AppId::Videos => (760, 520),
        AppId::Music => (640, 440),
        AppId::Maps => (760, 520),
        AppId::Camera => (680, 500),
        AppId::Contacts => (680, 480),
        AppId::Messages => (680, 480),
        AppId::Mail => (680, 480),
        _ => (640, 420),
    }
}

fn shadow(g: &Gfx, x: i32, y: i32, w: i32, h: i32, r: i32) {
    let mut i = 9;
    while i > 0 {
        let a = (i * 4 + 4) as u8;
        g.round_rect_outline(x - i, y - i + 5, w + i * 2, h + i * 2, r + i, rgb(4, 6, 14), a);
        i -= 1;
    }
    g.round_rect(x - 3, y + h, w + 6, 8, 4, 0x000000, 30);
}

fn card(g: &Gfx, x: i32, y: i32, w: i32, h: i32, r: i32) {
    shadow(g, x, y, w, h, r);
    g.round_rect(x, y, w, h, r, rgb(246, 248, 253), 255);
    g.round_rect(x + 1, y + 1, w - 2, h - 2, r - 1, rgb(252, 253, 255), 255);
}

fn btn(g: &Gfx, x: i32, y: i32, w: i32, h: i32, label: &str, primary: bool) {
    if primary {
        g.round_rect(x, y, w, h, h / 2, rgb(60, 116, 240), 255);
        g.round_rect(x, y, w, h * 2 / 3, h / 2, rgb(116, 164, 255), 70);
        g.round_rect(x, y, w, h, h / 2, rgb(255, 255, 255), 0);
        g.text_center(x + w / 2, y + (h - 16) / 2, label, rgb(255, 255, 255), 1);
    } else {
        g.round_rect(x, y, w, h, h / 2, rgb(230, 234, 244), 255);
        g.text_center(x + w / 2, y + (h - 16) / 2, label, rgb(58, 64, 82), 1);
    }
}

fn toggle(g: &Gfx, x: i32, y: i32, on: bool) {
    let c = if on { rgb(48, 196, 118) } else { rgb(204, 210, 222) };
    g.round_rect(x, y + 1, 42, 24, 12, rgb(0, 0, 0), 22);
    g.round_rect(x, y, 42, 24, 12, c, 255);
    let kx = if on { x + 20 } else { x + 2 };
    g.circle(kx + 10, y + 12, 10, rgb(255, 255, 255), 255);
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
    card(g, x, y, w, h, 18);
    let th = HEADER_H;
    g.round_rect(x, y, w, th, 18, rgb(238, 242, 251), 255);
    g.rect(x, y + th - 14, w, 14, rgb(238, 242, 251), 255);
    g.rect(x + 1, y + th - 2, w - 2, 1, rgb(210, 218, 232), 220);
    if focus {
        g.circle(x + 20, y + 20, 7, rgb(255, 95, 86), 255);
        g.circle(x + 42, y + 20, 7, rgb(255, 189, 46), 255);
        g.circle(x + 64, y + 20, 7, rgb(39, 201, 63), 255);
    } else {
        g.circle(x + 20, y + 20, 6, rgb(198, 204, 216), 255);
        g.circle(x + 42, y + 20, 6, rgb(206, 212, 224), 255);
        g.circle(x + 64, y + 20, 6, rgb(214, 220, 232), 255);
    }
    g.text_center(x + w / 2, y + 13, id.name(), rgb(44, 50, 66), 1);
    g.rect(x + w - 96, y + 8, 1, th - 16, rgb(210, 218, 232), 160);
    g.circle(x + w - 30, y + 20, 5, rgb(190, 198, 212), 200);
    st.tick += 1;
    match id {
        AppId::Terminal => draw_term(&mut st.term, g, x, y, w, h),
        AppId::Files => draw_files(st, g, x, y, w, h),
        AppId::Settings => draw_settings(&mut st.settings, g, x, y, w, h),
        AppId::Weather => draw_weather(&mut st.weather, g, x, y, w, h),
        AppId::Store => draw_store(&mut st.store, g, x, y, w, h),
        AppId::About => draw_about(g, x, y, w, h),
        AppId::Security | AppId::Calculator | AppId::Calendar | AppId::Clock | AppId::Browser
        | AppId::Notes | AppId::Photos | AppId::Videos | AppId::Music | AppId::Maps
        | AppId::Camera | AppId::Contacts | AppId::Messages | AppId::Mail => {
            suite::draw(&mut st.suite, id, g, x, y, w, h)
        }
        _ => draw_generic(g, id, x, y, w, h),
    }
}

fn draw_term(t: &mut Terminal, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let ix = x + 12;
    let iy = y + HEADER_H + 8;
    let iw = w - 24;
    let ih = h - HEADER_H - 20;
    g.round_rect(ix, iy, iw, ih, 12, rgb(11, 15, 22), 255);
    g.round_rect_outline(ix, iy, iw, ih, 12, rgb(255, 255, 255), 18);
    g.text(ix + 14, iy + 8, "HYWOS 终端 · UTF-8 中文", rgb(94, 180, 218), 1);
    g.rect(ix + 14, iy + 28, iw - 28, 1, rgb(255, 255, 255), 16);
    let top = iy + 40;
    let rows = (ih - 76) / 16;
    let start = if t.n as i32 > rows { t.n as i32 - rows } else { 0 };
    let mut i = start as usize;
    let mut row = 0;
    while i < t.n {
        let s = t.line_str(i);
        let col = if s.starts_with("user@hywos") { rgb(104, 216, 160) } else { rgb(204, 218, 240) };
        g.text(ix + 14, top + row * 16, s, col, 1);
        row += 1;
        i += 1;
    }
    let py = iy + ih - 26;
    g.text(ix + 14, py, t.prompt(), rgb(104, 216, 160), 1);
    let px = ix + 14 + text_width(t.prompt(), 1) + 8;
    let input = unsafe { core::str::from_utf8_unchecked(&t.input[..t.ilen]) };
    g.text(px, py, input, rgb(240, 246, 255), 1);
    if (t.tick_blink() / 20) % 2 == 0 {
        g.rect(px + text_width(input, 1), py, 7, 15, rgb(150, 210, 255), 200);
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
    let side = 176;
    g.round_rect(x + 10, top, side, h - HEADER_H - 24, 12, rgb(235, 240, 249), 255);
    let places = ["主目录", "文档", "图片", "截图", "应用数据"];
    let mut i = 0;
    while i < 5 {
        let py = top + 14 + i as i32 * 32;
        if i == st.files_sel.min(4) {
            g.round_rect(x + 18, py - 7, side - 16, 28, 8, rgb(206, 222, 250), 255);
            g.rect(x + 18, py - 5, 3, 24, rgb(60, 116, 240), 255);
        }
        g.text(x + 30, py, places[i], rgb(54, 62, 82), 1);
        i += 1;
    }
    let lx = x + side + 26;
    let lw = w - side - 44;
    let mut ents: [DirEntry; 32] = unsafe { core::mem::zeroed() };
    let n = unsafe { list_dir(usr_vol(), st.files_cwd.as_str(), &mut ents) };
    g.round_rect(lx - 8, top - 2, lw + 12, 26, 8, rgb(232, 238, 248), 200);
    g.text(lx, top + 4, st.files_cwd.as_str(), rgb(96, 106, 128), 1);
    let mut k = 0;
    while k < n && k < 12 {
        let ey = top + 34 + k as i32 * 28;
        if k == st.files_click {
            g.round_rect(lx - 8, ey - 6, lw + 12, 26, 8, rgb(214, 228, 252), 255);
        }
        let name = ents[k].name.as_str();
        if ents[k].is_dir {
            g.round_rect(lx, ey, 16, 12, 3, rgb(248, 194, 82), 255);
            g.rect(lx, ey + 2, 7, 3, rgb(248, 194, 82), 255);
            g.text(lx + 24, ey - 2, name, rgb(46, 54, 74), 1);
        } else {
            g.round_rect(lx, ey - 2, 12, 15, 3, rgb(150, 170, 202), 255);
            let col = if name.ends_with(".updpkg") { rgb(200, 76, 70) } else { rgb(70, 80, 100) };
            g.text(lx + 24, ey - 2, name, col, 1);
        }
        k += 1;
    }
    if n == 0 {
        g.text(lx, top + 40, "（空目录）", rgb(150, 158, 176), 1);
    }
    g.text(lx, y + h - 26, "双击 .updpkg 可打开软件更新", rgb(136, 146, 168), 1);
}

unsafe fn draw_settings(s: &mut Settings, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let top = y + HEADER_H + 8;
    let side = 190;
    g.round_rect(x + 10, top, side, h - HEADER_H - 24, 12, rgb(235, 240, 249), 255);
    let pages = ["显示", "权限", "软件更新", "关于本机"];
    let mut i = 0;
    while i < 4 {
        let py = top + 12 + i as i32 * 36;
        if s.page == i {
            g.round_rect(x + 18, py - 8, side - 16, 32, 9, rgb(206, 222, 250), 255);
            g.rect(x + 18, py - 6, 3, 28, rgb(60, 116, 240), 255);
            g.text(x + 32, py, pages[i], rgb(34, 88, 196), 1);
        } else {
            g.text(x + 32, py, pages[i], rgb(62, 70, 90), 1);
        }
        i += 1;
    }
    let cx = x + side + 26;
    let cy = top + 6;
    match s.page {
        0 => {
            g.text(cx, cy, "显示设置", rgb(36, 42, 58), 2);
            g.text(cx, cy + 46, "分辨率", rgb(132, 140, 158), 1);
            let mut b = [0u8; 24];
            let mut l = hw::num2(&mut b, crate::gfx::Gfx::screen_w() as u32, 1);
            b[l] = b'x'; l += 1;
            l += hw::num2(&mut b[l..], crate::gfx::Gfx::screen_h() as u32, 1);
            g.text(cx, cy + 66, core::str::from_utf8_unchecked(&b[..l]), rgb(50, 56, 72), 1);
            g.text(cx, cy + 98, "合成器", rgb(132, 140, 158), 1);
            g.text(cx, cy + 118, "软件 32bpp · 图标 4 倍超采样", rgb(50, 56, 72), 1);
            btn(g, cx, cy + 152, 180, 36, "立即截屏", true);
            g.text(cx, cy + 202, "也可按 F12 或 Print Screen 截屏", rgb(140, 148, 168), 1);
            if s.msg_len > 0 {
                let m = core::str::from_utf8_unchecked(&s.msg[..s.msg_len]);
                g.text(cx, cy + 226, m, if s.msg_ok { rgb(36, 160, 88) } else { rgb(192, 72, 68) }, 1);
            }
        }
        1 => {
            g.text(cx, cy, "应用权限", rgb(36, 42, 58), 2);
            g.text(cx, cy + 36, "按应用控制存储、通知与网络", rgb(132, 140, 158), 1);
            let apps = ["终端", "文件", "设置", "天气", "商店", "关于本机"];
            g.text(cx, cy + 68, "应用", rgb(150, 158, 176), 1);
            g.text(cx + 150, cy + 68, "存储", rgb(150, 158, 176), 1);
            g.text(cx + 250, cy + 68, "通知", rgb(150, 158, 176), 1);
            g.text(cx + 340, cy + 68, "网络", rgb(150, 158, 176), 1);
            let mut i = 0;
            while i < 6 {
                let ry = cy + 92 + i as i32 * 34;
                g.text(cx, ry, apps[i], rgb(50, 56, 72), 1);
                let mut k = 0;
                while k < 3 {
                    let on = perm_get(["terminal", "files", "settings", "weather", "store", "about"][i], k as u8);
                    toggle(g, cx + 150 + k as i32 * 100, ry - 3, on);
                    k += 1;
                }
                i += 1;
            }
            g.text(cx, cy + 330, "关闭存储后，应用读写接口会真实拒绝", rgb(150, 158, 176), 1);
        }
        2 => {
            g.text(cx, cy, "软件更新", rgb(36, 42, 58), 2);
            g.text(cx, cy + 34, "当前版本 1.1.1 · GitHub Releases 在线检测", rgb(132, 140, 158), 1);
            let by = cy + 58;
            btn(g, cx, by, 140, 34, "检查更新 (C)", true);
            let can_dl = s.update_state == 2;
            btn(g, cx + 152, by, 160, 34, if can_dl { "下载并安装 (D)" } else { "等待检查" }, can_dl);
            let status = match s.update_state {
                1 => "已是最新版本",
                2 => "发现新版本，可下载并安装",
                3 => "检查失败：无网络或固件未提供 HTTP/TLS",
                4 => "更新已写入，重启后生效",
                _ => "尚未检查更新",
            };
            g.text(cx, cy + 104, status, if s.update_state == 3 { rgb(192, 72, 68) } else { rgb(50, 56, 72) }, 1);
            if s.update_state == 2 {
                let mut vb = [0u8; 24];
                let vl = s.release.version_str(&mut vb);
                g.text(cx, cy + 124, core::str::from_utf8_unchecked(&vb[..vl]), rgb(36, 160, 88), 2);
                let mut u = [0u8; 80];
                let mut ul = 0;
                if s.release.url_len > 0 {
                    let prefix = if s.release.url[0] == b'\\' { "本地包：" } else { "下载源：" };
                    for c in prefix.bytes() { if ul < 40 { u[ul] = c; ul += 1; } }
                    let mut k = 0;
                    while k < s.release.url_len && ul < 58 {
                        u[ul] = s.release.url[k];
                        ul += 1;
                        k += 1;
                    }
                }
                if ul > 0 {
                    g.text(cx, cy + 152, core::str::from_utf8_unchecked(&u[..ul]), rgb(118, 128, 148), 1);
                }
            }
            if s.update_msg_len > 0 {
                let m = core::str::from_utf8_unchecked(&s.update_msg[..s.update_msg_len]);
                g.text(cx, cy + 174, m, if s.update_ok { rgb(36, 160, 88) } else { rgb(192, 72, 68) }, 1);
            }
            g.rect(cx, cy + 198, w - side - 52, 1, rgb(214, 222, 236), 200);
            g.text(cx, cy + 208, "本地更新包（.updpkg）", rgb(36, 42, 58), 1);
            let p = if s.pending_len > 0 {
                core::str::from_utf8_unchecked(&s.pending[..s.pending_len])
            } else {
                "（尚未选择本地更新包）"
            };
            g.round_rect(cx - 8, cy + 226, w - side - 52, 26, 7, rgb(232, 238, 248), 200);
            g.text(cx, cy + 232, p, rgb(50, 56, 72), 1);
            btn(g, cx, cy + 262, 140, 34, "应用本地包", true);
            btn(g, cx + 152, cy + 262, 160, 34, "检查设备", false);
            let mut dev = [0u8; 56];
            let mut dl = 0;
            match block_info() {
                Some((blocks, bs)) => {
                    for c in "内核设备：".as_bytes().iter() { dev[dl] = *c; dl += 1; }
                    dl += hw::num2(&mut dev[dl..], blocks as u32, 1);
                    for c in " 块 × ".as_bytes().iter() { dev[dl] = *c; dl += 1; }
                    dl += hw::num2(&mut dev[dl..], bs, 1);
                }
                None => {
                    for c in "内核设备：未发现可写块设备".as_bytes().iter() { dev[dl] = *c; dl += 1; }
                }
            }
            g.text(cx, cy + 308, core::str::from_utf8_unchecked(&dev[..dl]), rgb(118, 128, 148), 1);
            g.text(cx, cy + 334, "在线检测 GitHub Releases", rgb(150, 158, 176), 1);
            g.text(cx, cy + 356, "离线回退 \\updates\\latest.json", rgb(150, 158, 176), 1);
            g.text(cx, cy + 378, "更新只替换内核文件，用户数据保留", rgb(150, 158, 176), 1);
            if s.msg_len > 0 {
                let m = core::str::from_utf8_unchecked(&s.msg[..s.msg_len]);
                g.text(cx, cy + 404, m, if s.msg_ok { rgb(36, 160, 88) } else { rgb(192, 72, 68) }, 1);
            }
        }
        _ => draw_about(g, x + side + 16, top, w - side - 26, h - HEADER_H - 24),
    }
}

unsafe fn draw_weather(wx: &mut Weather, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    wx.frame += 1;
    let ix = x + 14;
    let iy = y + HEADER_H + 6;
    let iw = w - 28;
    let ih = h - HEADER_H - 18;
    g.round_rect(ix, iy, iw, ih, 14, rgb(52, 118, 214), 255);
    g.vgrad(ix + 1, iy + 1, iw - 2, ih / 2, rgb(94, 158, 240), rgb(58, 118, 210));
    g.vgrad(ix + 1, iy + ih / 2, iw - 2, ih / 2 - 1, rgb(58, 118, 210), rgb(34, 82, 170));
    g.round_rect(ix, iy, iw, 1, 0, rgb(255, 255, 255), 70);
    let f = wx.frame;
    sun_cloud(g, ix + iw - 92, iy + 74, 44, f, 1);
    let city = CITIES[wx.city];
    g.text(ix + 20, iy + 14, city, rgb(240, 248, 255), 2);
    let cur = FORECAST[wx.city][wx.day];
    let mut tb = [0u8; 12];
    let tl = fmt_temp(cur.hi, wx.unit_f, &mut tb);
    g.text(ix + 20, iy + 44, core::str::from_utf8_unchecked(&tb[..tl]), rgb(255, 255, 255), 3);
    let unit = if wx.unit_f { "F" } else { "C" };
    g.text(ix + 20 + 3 * 24 + 6, iy + 62, unit, rgb(214, 232, 255), 2);
    g.round_rect(ix + iw - 84, iy + 14, 70, 30, 15, rgb(255, 255, 255), 44);
    g.text_center(ix + iw - 49, iy + 21, "C / F", rgb(255, 255, 255), 1);
    let conf = match cur.cond { 0 => "晴", 1 => "多云", _ => "小雨" };
    g.text(ix + 20, iy + 100, conf, rgb(214, 232, 255), 1);
    let rows_y = iy + 126;
    let mut i = 0;
    while i < 5 {
        let ry = rows_y + i as i32 * 34;
        let fc = FORECAST[wx.city][i];
        if i == wx.day {
            g.round_rect(ix + 10, ry - 4, iw - 20, 30, 9, rgb(255, 255, 255), 46);
            g.rect(ix + 10, ry - 4, 3, 30, rgb(255, 232, 120), 255);
        }
        g.text(ix + 24, ry + 2, DAY_NAMES[i], rgb(236, 245, 255), 1);
        let mut hb = [0u8; 12];
        let hl = fmt_temp(fc.hi, wx.unit_f, &mut hb);
        let mut lb = [0u8; 12];
        let ll = fmt_temp(fc.lo, wx.unit_f, &mut lb);
        let hu = if wx.unit_f { "F" } else { "C" };
        g.text(ix + iw - 158, ry + 2, core::str::from_utf8_unchecked(&hb[..hl]), rgb(255, 255, 255), 1);
        g.text(ix + iw - 158 + 24, ry + 2, hu, rgb(214, 232, 255), 1);
        g.text(ix + iw - 96, ry + 2, core::str::from_utf8_unchecked(&lb[..ll]), rgb(190, 214, 246), 1);
        g.text(ix + iw - 96 + 24, ry + 2, hu, rgb(190, 214, 246), 1);
        sun_cloud(g, ix + iw - 210, ry + 10, 22, f + i as u32 * 7, fc.cond);
        i += 1;
    }
    g.text(ix + 20, iy + ih - 24, "离线 5 日天气 · 单位保存在应用存储", rgb(204, 226, 252), 1);
}

unsafe fn draw_store(s: &mut Store, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
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
        g.round_rect(cx, cy, cw, ch, 12, if selected { rgb(219, 232, 253) } else { rgb(241, 244, 250) }, 255);
        if selected {
            g.round_rect_outline(cx, cy, cw, ch, 12, rgb(74, 140, 244), 220);
        }
        let ic = icon(STORE_APPS[i].2);
        g.blit_rgba(cx + 12, cy + 14, 42, 42, ic, 64, 64);
        g.text(cx + 64, cy + 14, STORE_APPS[i].1, rgb(40, 48, 64), 1);
        let desc = STORE_APPS[i].3;
        g.text(cx + 64, cy + 34, desc, rgb(128, 136, 156), 1);
        let label = if s.installed[i] { "已安装" } else { "安装" };
        btn(g, cx + 64, cy + 56, 82, 26, label, !s.installed[i]);
        i += 1;
    }
    if s.msg_len > 0 {
        let m = core::str::from_utf8_unchecked(&s.msg[..s.msg_len]);
        g.text(x + 24, y + h - 28, m, if s.msg_ok { rgb(36, 160, 88) } else { rgb(192, 72, 68) }, 1);
    }
}

unsafe fn draw_about(g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let cx = x + w / 2;
    let lx = cx - 48;
    let ly = y + 44;
    g.round_rect(lx - 6, ly - 6, 108, 108, 24, rgb(16, 26, 58), 40);
    g.blit_rgba(lx, ly, 96, 96, &crate::logo::LOGO_RGBA, crate::logo::LOGO_SIZE as i32, crate::logo::LOGO_SIZE as i32);
    g.big_text(cx - 40, y + 138, "HYWOS", rgb(28, 36, 56), 255);
    g.text_center(cx, y + 190, "星河版 · 版本 1.1.1", rgb(108, 118, 140), 1);
    let vx = x + 22 + 40;
    let mut ry = y + 212;
    g.text(x + 22, ry, "内核", rgb(68, 78, 98), 1);
    g.text(vx, ry, "HYWOS Kernel 1.1.1（Rust no_std UEFI）", rgb(68, 78, 98), 1);
    ry += 19;
    g.text(x + 22, ry, "架构", rgb(68, 78, 98), 1);
    g.text(vx, ry, "x86_64 · ACPI / CPUID 拓扑", rgb(68, 78, 98), 1);
    ry += 19;
    g.text(x + 22, ry, "显卡", rgb(68, 78, 98), 1);
    g.text(vx, ry, crate::video::name(), rgb(68, 78, 98), 1);
    ry += 19;
    g.text(x + 22, ry, "输入", rgb(68, 78, 98), 1);
    g.text(vx, ry, if crate::input::modern_keyboard() || crate::input::absolute_pointer() { crate::input::describe() } else if crate::ps2::mouse_ok() { "PS/2 键盘与鼠标（8042）" } else { "UEFI 固件输入" }, rgb(68, 78, 98), 1);
    ry += 19;
    g.text(x + 22, ry, "音频", rgb(68, 78, 98), 1);
    g.text(vx, ry, crate::audio::name(), rgb(68, 78, 98), 1);
    ry += 19;
    g.text(x + 22, ry, "网卡", rgb(68, 78, 98), 1);
    let nic = crate::net::info();
    let mut nb = [0u8; 64];
    let mut nl = 0usize;
    if nic.present {
        for c in nic.name.bytes() { if nl < 20 { nb[nl] = c; nl += 1; } }
        for c in " · MAC ".as_bytes() { nb[nl] = *c; nl += 1; }
        let mut k = 0;
        while k < 6 {
            let hi = nic.mac[k] >> 4;
            let lo = nic.mac[k] & 0xf;
            nb[nl] = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 }; nl += 1;
            nb[nl] = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 }; nl += 1;
            if k < 5 { nb[nl] = b':'; nl += 1; }
            k += 1;
        }
    } else {
        let txt = if crate::hw::net().present { "UEFI SNP（固件网络栈）" } else { "未发现网卡" };
        for c in txt.bytes() { nb[nl] = c; nl += 1; }
    }
    g.text(vx, ry, core::str::from_utf8_unchecked(&nb[..nl]), rgb(68, 78, 98), 1);
    ry += 19;
    g.text(x + 22, ry, "授权", rgb(68, 78, 98), 1);
    g.text(vx, ry, "MIT License", rgb(68, 78, 98), 1);
    ry += 19;
    g.text(x + 22, ry, "源码", rgb(68, 78, 98), 1);
    g.text(vx, ry, "github.com/lmxdg/HYWOS", rgb(68, 78, 98), 1);
    ry += 19;
    let c = hw::cpu();
    let mut b = [0u8; 40];
    let mut l = 0;
    g.text(x + 22, ry, "核心", rgb(68, 78, 98), 1);
    l += hw::num2(&mut b[l..], c.enabled as u32, 1);
    for ch in " / ".as_bytes().iter() { b[l] = *ch; l += 1; }
    l += hw::num2(&mut b[l..], c.total as u32, 1);
    for ch in " 个逻辑处理器".as_bytes().iter() { b[l] = *ch; l += 1; }
    let _ = h;
    g.text(vx, ry, core::str::from_utf8_unchecked(&b[..l]), rgb(68, 78, 98), 1);
}

fn draw_generic(g: &Gfx, id: AppId, x: i32, y: i32, w: i32, h: i32) {
    let ic = unsafe { icon(id) };
    let s = 88;
    g.blit_rgba(x + w / 2 - s / 2, y + HEADER_H + 30, s, s, ic, 64, 64);
    g.text_center(x + w / 2, y + HEADER_H + 30 + s + 16, id.name(), rgb(40, 48, 64), 2);
    g.text_center(x + w / 2, y + HEADER_H + 30 + s + 56, "HYWOS 应用套件成员", rgb(130, 138, 156), 1);
    g.text_center(x + w / 2, y + HEADER_H + 30 + s + 80, "功能将在后续版本提供", rgb(160, 168, 184), 1);
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
                let by = top + 64;
                if ly > by && ly < by + 38 && lx > cx && lx < cx + 140 {
                    do_check_update(&mut st.settings);
                    return AppAction::None;
                }
                if ly > by && ly < by + 38 && lx > cx + 152 && lx < cx + 312 {
                    do_download_update(&mut st.settings);
                    return AppAction::None;
                }
                let ay = top + 268;
                if ly > ay && ly < ay + 38 && lx > cx && lx < cx + 140 {
                    apply_pending(&mut st.settings);
                    return AppAction::None;
                }
                if ly > ay && ly < ay + 38 && lx > cx + 152 && lx < cx + 312 {
                    let b = find_block_device();
                    let msg = if b.is_null() { "未发现可写块设备。" } else { "已检测到可写内核设备。" };
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
            if mx > ix + iw - 84 && mx < ix + iw - 14 && my > iy + 12 && my < iy + 46 {
                st.weather.unit_f = !st.weather.unit_f;
                let u: &[u8] = if st.weather.unit_f { b"f" } else { b"c" };
                let _ = appstore_set("weather", "unit", core::str::from_utf8_unchecked(u));
                return AppAction::None;
            }
            let rows_y = iy + 126;
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
            let ch = 96;
            let mut i = 0;
            while i < 6 {
                let col = (i % 2) as i32;
                let row = (i / 2) as i32;
                let cx = 20 + col * (cw + 16);
                let cy = top + row * (ch + 8);
                if lx > cx && lx < cx + cw && ly > cy && ly < cy + ch {
                    st.store.sel = i;
                    if lx > cx + 60 && ly > cy + 50 && ly < cy + 88 {
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
            let side = 176;
            if lx < 10 + side && ly > top {
                let idx = ((ly - top - 14 + 8) / 32) as usize;
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
            let idx = ((ly - top - 34 + 7) / 28).max(0) as usize;
            if ly > top + 28 && idx < n {
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
        AppId::Security | AppId::Calculator | AppId::Calendar | AppId::Clock | AppId::Browser
        | AppId::Notes | AppId::Photos | AppId::Videos | AppId::Music | AppId::Maps
        | AppId::Camera | AppId::Contacts | AppId::Messages | AppId::Mail => {
            if suite::click(&mut st.suite, id, lx, ly, x, y, w, h) {
                return AppAction::None;
            }
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
    if suite::key(&mut st.suite, id, scan, ch) {
        return AppAction::None;
    }
    if id == AppId::Settings {
        if scan == 0x04 && st.settings.page > 0 {
            st.settings.page -= 1;
            return AppAction::None;
        }
        if scan == 0x03 && st.settings.page < 3 {
            st.settings.page += 1;
            return AppAction::None;
        }
        if st.settings.page == 2 {
            if ch == b'c' as u16 || ch == b'C' as u16 {
                do_check_update(&mut st.settings);
                return AppAction::None;
            }
            if ch == b'd' as u16 || ch == b'D' as u16 {
                do_download_update(&mut st.settings);
                return AppAction::None;
            }
            if scan == 0x16 {
                return AppAction::OpenUpdate;
            }
        }
    }
    if id == AppId::Weather && ch == b'u' as u16 {
        st.weather.unit_f = !st.weather.unit_f;
    }
    AppAction::None
}

unsafe fn set_update_msg(s: &mut Settings, ok: bool, msg: &str) {
    s.update_msg_len = 0;
    s.update_ok = ok;
    for c in msg.bytes() {
        if s.update_msg_len < s.update_msg.len() - 1 {
            s.update_msg[s.update_msg_len] = c;
            s.update_msg_len += 1;
        }
    }
}

unsafe fn do_check_update(s: &mut Settings) {
    match crate::updater::check_update() {
        crate::updater::Check::Latest(_a, _b, _c) => {
            s.update_state = 1;
            set_update_msg(s, true, "已是最新版本 1.1.1");
        }
        crate::updater::Check::Available(r) => {
            s.release = r;
            s.update_state = 2;
            let mut vb = [0u8; 24];
            let vl = r.version_str(&mut vb);
            let ver = core::str::from_utf8_unchecked(&vb[..vl]);
            let mut buf = [0u8; 64];
            let mut l = 0;
            for c in "发现新版本 ".bytes() { buf[l] = c; l += 1; }
            for c in ver.bytes() { if l < 60 { buf[l] = c; l += 1; } }
            set_update_msg(s, true, core::str::from_utf8_unchecked(&buf[..l]));
        }
        crate::updater::Check::Error(_e) => {
            s.update_state = 3;
            set_update_msg(s, false, "检查失败：无网络或固件未提供 HTTP/TLS");
        }
    }
}

unsafe fn do_download_update(s: &mut Settings) {
    if s.update_state != 2 {
        return;
    }
    let r = s.release;
    if r.url_len == 0 {
        set_update_msg(s, false, "该版本没有 .updpkg 更新包");
        return;
    }
    s.pending_len = 0;
    if r.url[0] != b'\\' {
        for c in "\\updates\\hywos-".bytes() {
            if s.pending_len < 76 { s.pending[s.pending_len] = c; s.pending_len += 1; }
        }
        let mut vb = [0u8; 24];
        let vl = r.version_str(&mut vb);
        let mut i = 0;
        while i < vl && s.pending_len < 78 {
            s.pending[s.pending_len] = vb[i];
            s.pending_len += 1;
            i += 1;
        }
        for c in ".updpkg".bytes() {
            if s.pending_len < 79 { s.pending[s.pending_len] = c; s.pending_len += 1; }
        }
    } else {
        let mut i = 0;
        while i < r.url_len && s.pending_len < 79 {
            s.pending[s.pending_len] = r.url[i];
            s.pending_len += 1;
            i += 1;
        }
    }
    let code = crate::updater::download_and_apply(&r);
    let msg: &str = match code {
        0 => {
            s.update_state = 4;
            "更新成功：已写入内核区，重启后生效"
        }
        1 => "更新包头无效",
        2 => "CRC32 校验失败，已拒绝",
        3 => "未找到可写内核块设备",
        10 => "该版本没有 .updpkg 更新包",
        11 => "本地更新包读取失败",
        12 => "更新包保存到本地失败",
        _ => "网络下载失败，请检查连接",
    };
    set_update_msg(s, code == 0, msg);
}

unsafe fn apply_pending(s: &mut Settings) {
    if s.pending_len == 0 {
        s.msg_len = 0;
        return;
    }
    let mut pathbuf = [0u8; 84];
    let pl = s.pending_len;
    let mut i = 0;
    while i < pl { pathbuf[i] = s.pending[i]; i += 1; }
    let p = core::str::from_utf8_unchecked(&pathbuf[..pl]);
    let res = apply_update_file(usr_vol(), p);
    let msg: &str = match res {
        0 => "更新已写入内核区，请重启",
        1 => "更新包头无效",
        2 => "CRC32 校验失败，已拒绝",
        3 => "未找到可写内核块设备",
        _ => "用户卷中未找到更新包",
    };
    s.msg_len = msg.len().min(70);
    let mut i = 0;
    for c in msg.bytes() { if i < 70 { s.msg[i] = c; i += 1; } }
    s.msg_ok = res == 0;
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