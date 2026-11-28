#![allow(dead_code)]

//! HYWOS AI 助手：OpenAI 兼容接口、对话、提示词/skills 注入与系统操作。

use crate::display;
use crate::fs;
use crate::gfx::{rgb, Gfx};
use crate::icon::AppId;
use super::AppAction;

const PATH: &str = "\\home\\user\\.apps\\ai.kv";

pub struct AiState {
    pub endpoint: [u8; 128],
    pub endpoint_len: usize,
    pub key: [u8; 192],
    pub key_len: usize,
    pub model: [u8; 48],
    pub model_len: usize,
    pub input: [u8; 256],
    pub input_len: usize,
    pub reply: [u8; 2048],
    pub reply_len: usize,
    pub status: [u8; 160],
    pub status_len: usize,
    pub status_ok: bool,
    pub config: bool,
    pub field: u8,
    pub busy: bool,
}

impl AiState {
    pub fn new() -> AiState {
        let mut s: AiState = unsafe { core::mem::zeroed() };
        s.set_endpoint(b"https://api.openai.com/v1/chat/completions");
        s.set_model(b"gpt-4o-mini");
        s
    }

    fn set_endpoint(&mut self, v: &[u8]) {
        self.endpoint_len = 0;
        for c in v {
            if self.endpoint_len < self.endpoint.len() { self.endpoint[self.endpoint_len] = *c; self.endpoint_len += 1; }
        }
    }
    fn set_model(&mut self, v: &[u8]) {
        self.model_len = 0;
        for c in v {
            if self.model_len < self.model.len() { self.model[self.model_len] = *c; self.model_len += 1; }
        }
    }

    fn set_status(&mut self, ok: bool, m: &str) {
        self.status_len = 0;
        for c in m.bytes() {
            if self.status_len < self.status.len() { self.status[self.status_len] = c; self.status_len += 1; }
        }
        self.status_ok = ok;
    }

    pub unsafe fn load(&mut self) {
        if let Some(n) = fs::read_file(fs::usr_vol(), PATH) {
            let b = &fs::rdbuf()[..n];
            self.endpoint_len = 0;
            self.key_len = 0;
            self.model_len = 0;
            read_field(b, b"endpoint", &mut self.endpoint, &mut self.endpoint_len);
            read_field(b, b"key", &mut self.key, &mut self.key_len);
            read_field(b, b"model", &mut self.model, &mut self.model_len);
        }
        if self.endpoint_len == 0 { self.set_endpoint(b"https://api.openai.com/v1/chat/completions"); }
        if self.model_len == 0 { self.set_model(b"gpt-4o-mini"); }
        if self.key_len == 0 {
            self.set_status(false, "未配置 API Key：按 C 配置 endpoint / key / model");
        } else {
            self.set_status(true, "已配置：Enter 发送，C 修改配置");
        }
    }

    unsafe fn save(&mut self) {
        let mut b = [0u8; 512];
        let mut l = 0usize;
        push_s(&mut b, &mut l, b"endpoint=");
        for i in 0..self.endpoint_len { push_b(&mut b, &mut l, self.endpoint[i]); }
        push_s(&mut b, &mut l, b"\nkey=");
        for i in 0..self.key_len { push_b(&mut b, &mut l, self.key[i]); }
        push_s(&mut b, &mut l, b"\nmodel=");
        for i in 0..self.model_len { push_b(&mut b, &mut l, self.model[i]); }
        push_b(&mut b, &mut l, b'\n');
        if fs::write_file(fs::usr_vol(), PATH, &b[..l]) {
            self.set_status(true, "配置已保存");
        } else {
            self.set_status(false, "配置保存失败");
        }
    }

    pub unsafe fn key(&mut self, scan: u16, ch: u16) -> AppAction {
        if self.config {
            if scan == 0x0f {
                self.field = (self.field + 1) % 3;
                return AppAction::None;
            }
            if ch == 13 {
                self.config = false;
                self.save();
                return AppAction::None;
            }
            if scan == 0x17 {
                self.config = false;
                self.set_status(true, "已取消配置编辑");
                return AppAction::None;
            }
            if ch == 8 {
                match self.field {
                    1 => if self.key_len > 0 { self.key_len -= 1; },
                    2 => if self.model_len > 0 { self.model_len -= 1; },
                    _ => if self.endpoint_len > 0 { self.endpoint_len -= 1; },
                }
                return AppAction::None;
            }
            if ch >= 32 && ch < 127 {
                match self.field {
                    1 => if self.key_len < self.key.len() { self.key[self.key_len] = ch as u8; self.key_len += 1; },
                    2 => if self.model_len < self.model.len() { self.model[self.model_len] = ch as u8; self.model_len += 1; },
                    _ => if self.endpoint_len < self.endpoint.len() { self.endpoint[self.endpoint_len] = ch as u8; self.endpoint_len += 1; },
                }
            }
            return AppAction::None;
        }
        if ch == b'c' as u16 || ch == b'C' as u16 {
            self.config = true;
            self.field = 0;
            return AppAction::None;
        }
        if ch == 13 {
            return self.send();
        }
        if ch == 8 {
            if self.input_len > 0 { self.input_len -= 1; }
            return AppAction::None;
        }
        if scan == 0x17 {
            self.input_len = 0;
            self.reply_len = 0;
            return AppAction::None;
        }
        if ch >= 32 && ch < 127 && self.input_len < self.input.len() {
            self.input[self.input_len] = ch as u8;
            self.input_len += 1;
        }
        AppAction::None
    }

    unsafe fn send(&mut self) -> AppAction {
        if self.key_len == 0 {
            self.set_status(false, "请先按 C 配置 API Key");
            return AppAction::None;
        }
        if self.input_len == 0 {
            self.set_status(false, "请输入内容后按 Enter");
            return AppAction::None;
        }
        let mut body = [0u8; 4096];
        let mut bl = 0usize;
        push_s(&mut body, &mut bl, b"{\"model\":\"");
        for i in 0..self.model_len { push_b(&mut body, &mut bl, self.model[i]); }
        push_s(&mut body, &mut bl, b"\",\"messages\":[{\"role\":\"system\",\"content\":\"");
        let sys = "你是 HYWOS 1.2.4 的 AI 助手，开发方 MEMZ-LMXDG。系统为 C/C++ 混合内核 + Rust UI。可用动作：[[OPEN:terminal]] [[OPEN:settings]] [[OPEN:files]] [[OPEN:office]] [[BRIGHTNESS:0-100]] [[VOLUME:0-100]] [[WALLPAPER:0-4]] [[SCALE:100|125|150]] [[SHUTDOWN]] [[REBOOT]]。请用中文简洁回答，需要操作系统时在末尾单独输出动作标记。";
        json_escape(sys.as_bytes(), &mut body, &mut bl);
        push_s(&mut body, &mut bl, b"\"},{\"role\":\"user\",\"content\":\"");
        json_escape(&self.input[..self.input_len], &mut body, &mut bl);
        push_s(&mut body, &mut bl, b"\"}],\"temperature\":0.7}");
        self.set_status(true, "正在请求 OpenAI…");
        self.busy = true;
        let mut out = [0u8; 8192];
        let url = core::str::from_utf8_unchecked(&self.endpoint[..self.endpoint_len]);
        let res = crate::http::post(url, &self.key[..self.key_len], &body[..bl], &mut out);
        self.busy = false;
        match res {
            Ok(n) => {
                self.reply_len = extract_content(&out[..n], &mut self.reply);
                if self.reply_len == 0 {
                    self.set_status(false, "响应解析失败（检查 endpoint/model）");
                    return AppAction::None;
                }
                self.input_len = 0;
                self.set_status(true, "回复已生成");
                let mut rbuf = [0u8; 2048];
                let rn = self.reply_len;
                let mut k = 0usize;
                while k < rn { rbuf[k] = self.reply[k]; k += 1; }
                return self.actions(&rbuf[..rn]);
            }
            Err(_) => {
                self.set_status(false, "请求失败：检查网络与 endpoint/key");
                AppAction::None
            }
        }
    }

    unsafe fn actions(&mut self, reply: &[u8]) -> AppAction {
        let mut i = 0usize;
        while i + 1 < reply.len() {
            if reply[i] == b'[' && reply[i + 1] == b'[' {
                let s = i + 2;
                let mut j = s;
                while j + 1 < reply.len() && !(reply[j] == b']' && reply[j + 1] == b']') { j += 1; }
                if j + 1 < reply.len() {
                    let a = &reply[s..j];
                    if let Some(app) = open_app(a) {
                        self.set_status(true, "已执行系统操作");
                        return AppAction::OpenApp(app);
                    }
                    if let Some(v) = num_after(a, b"BRIGHTNESS:") {
                        display::set_brightness(v);
                        self.set_status(true, "已调整亮度");
                        return AppAction::None;
                    }
                    if let Some(v) = num_after(a, b"VOLUME:") {
                        display::set_volume(v);
                        self.set_status(true, "已调整音量");
                        return AppAction::None;
                    }
                    if let Some(v) = num_after(a, b"WALLPAPER:") {
                        self.set_status(true, "已切换壁纸");
                        return AppAction::SetWallpaper(v as u8);
                    }
                    if let Some(v) = num_after(a, b"SCALE:") {
                        self.set_status(true, "已切换缩放");
                        return AppAction::SetScale(v as u16);
                    }
                    if a == b"SHUTDOWN" {
                        crate::power::shutdown();
                    }
                    if a == b"REBOOT" {
                        crate::power::reboot();
                    }
                }
                i = j + 2;
            } else {
                i += 1;
            }
        }
        AppAction::None
    }
}

fn read_field(b: &[u8], key: &[u8], out: &mut [u8], len: &mut usize) {
    let mut i = 0usize;
    while i + key.len() < b.len() {
        let mut ok = true;
        let mut k = 0;
        while k < key.len() {
            if b[i + k] != key[k] { ok = false; break; }
            k += 1;
        }
        if ok && b[i + key.len()] == b'=' {
            *len = 0;
            let mut j = i + key.len() + 1;
            while j < b.len() && b[j] != b'\n' && b[j] != b'\r' && *len < out.len() {
                out[*len] = b[j];
                *len += 1;
                j += 1;
            }
            return;
        }
        i += 1;
    }
}

fn push_b(out: &mut [u8], l: &mut usize, c: u8) {
    if *l < out.len() { out[*l] = c; *l += 1; }
}
fn push_s(out: &mut [u8], l: &mut usize, s: &[u8]) {
    for c in s { push_b(out, l, *c); }
}

fn json_escape(s: &[u8], out: &mut [u8], l: &mut usize) {
    for c in s {
        match *c {
            b'"' => { push_b(out, l, b'\\'); push_b(out, l, b'"'); }
            b'\\' => { push_b(out, l, b'\\'); push_b(out, l, b'\\'); }
            b'\n' => { push_b(out, l, b'\\'); push_b(out, l, b'n'); }
            b'\r' => {}
            _ => push_b(out, l, *c),
        }
    }
}

fn extract_content(json: &[u8], out: &mut [u8]) -> usize {
    let pat = b"\"content\":\"";
    let mut i = 0usize;
    while i + pat.len() < json.len() {
        let mut ok = true;
        let mut k = 0;
        while k < pat.len() {
            if json[i + k] != pat[k] { ok = false; break; }
            k += 1;
        }
        if ok {
            let mut j = i + pat.len();
            let mut l = 0usize;
            while j < json.len() && l < out.len() {
                let c = json[j];
                if c == b'"' { return l; }
                if c == b'\\' && j + 1 < json.len() {
                    let e = json[j + 1];
                    match e {
                        b'n' => { out[l] = b'\n'; l += 1; j += 2; continue; }
                        b't' => { out[l] = b'\t'; l += 1; j += 2; continue; }
                        b'"' => { out[l] = b'"'; l += 1; j += 2; continue; }
                        b'\\' => { out[l] = b'\\'; l += 1; j += 2; continue; }
                        b'u' => { j += 6; continue; }
                        _ => { j += 2; continue; }
                    }
                }
                out[l] = c;
                l += 1;
                j += 1;
            }
            return l;
        }
        i += 1;
    }
    0
}

fn num_after(a: &[u8], prefix: &[u8]) -> Option<u32> {
    if a.len() <= prefix.len() { return None; }
    let mut i = 0;
    while i < prefix.len() {
        if a[i] != prefix[i] { return None; }
        i += 1;
    }
    let mut v = 0u32;
    let mut any = false;
    while i < a.len() && a[i] >= b'0' && a[i] <= b'9' {
        v = v * 10 + (a[i] - b'0') as u32;
        i += 1;
        any = true;
    }
    if any { Some(v) } else { None }
}

fn open_app(a: &[u8]) -> Option<AppId> {
    if a == b"OPEN:terminal" { return Some(AppId::Terminal); }
    if a == b"OPEN:settings" { return Some(AppId::Settings); }
    if a == b"OPEN:files" { return Some(AppId::Files); }
    if a == b"OPEN:office" { return Some(AppId::Office); }
    if a == b"OPEN:browser" { return Some(AppId::Browser); }
    None
}

pub unsafe fn key(s: &mut AiState, scan: u16, ch: u16) -> AppAction {
    s.key(scan, ch)
}

pub unsafe fn draw(s: &mut AiState, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    let cx = x + 26;
    let cy = y + 54;
    g.text(cx, cy - 16, "AI 助手 · MEMZ-LMXDG", rgb(36, 42, 58), 2);
    if s.config {
        g.text(cx, cy + 24, "配置（Tab 切换字段，Enter 保存，Esc 取消）", rgb(90, 100, 120), 1);
        let names = ["Endpoint", "API Key", "Model"];
        let mut i = 0usize;
        while i < 3 {
            let yy = cy + 60 + i as i32 * 44;
            if i as u8 == s.field {
                g.round_rect(cx - 6, yy - 6, w - 60, 36, 8, rgb(214, 228, 250), 255);
            }
            g.text(cx, yy, names[i], rgb(50, 56, 72), 1);
            let val: &[u8] = if i == 0 {
                &s.endpoint[..s.endpoint_len]
            } else if i == 1 {
                &s.key[..s.key_len]
            } else {
                &s.model[..s.model_len]
            };
            let show = if i == 1 { b"********".as_slice() } else { val };
            g.text(cx + 100, yy, core::str::from_utf8_unchecked(show), rgb(40, 60, 96), 1);
            if i as u8 == s.field && (super::TICK / 16) % 2 == 0 {
                g.rect(cx + 100 + show.len() as i32 * 8, yy - 1, 2, 16, rgb(60, 116, 240), 220);
            }
            i += 1;
        }
        g.text(cx, cy + 210, if s.status_ok { core::str::from_utf8_unchecked(&s.status[..s.status_len]) } else { core::str::from_utf8_unchecked(&s.status[..s.status_len]) }, if s.status_ok { rgb(36, 160, 88) } else { rgb(192, 72, 68) }, 1);
        return;
    }
    let py = cy + 18;
    let ph = h - 140;
    g.round_rect(cx - 8, py - 8, w - 52, ph, 10, rgb(248, 250, 255), 255);
    g.round_rect_outline(cx - 8, py - 8, w - 52, ph, 10, rgb(214, 222, 236), 200);
    // 回复换行绘制
    let maxw = w - 84;
    let mut i = 0usize;
    let mut px = 0i32;
    let mut line = 0i32;
    while i < s.reply_len && line < (ph - 20) / 18 {
        let c = s.reply[i];
        if c == b'\n' {
            line += 1;
            px = 0;
            i += 1;
            continue;
        }
        let cw = if c < 128 { 8 } else { 16 };
        if px + cw > maxw {
            line += 1;
            px = 0;
        }
        if i < s.reply_len && line < (ph - 20) / 18 {
            if c < 128 {
                g.glyph(cx + px, py + line * 18, c, rgb(36, 42, 58), 1, 240);
            } else if i + 1 < s.reply_len {
                // UTF-8 多字节简单解码
                let mut cp = c as u32 & 0x1f;
                let extra = if c >= 0xf0 { 3 } else if c >= 0xe0 { 2 } else { 1 };
                let mut k = 1;
                while k <= extra && i + k < s.reply_len {
                    cp = (cp << 6) | (s.reply[i + k] as u32 & 0x3f);
                    k += 1;
                }
                g.cjk_glyph(cx + px, py + line * 18, cp, rgb(36, 42, 58), 1, 240);
                i += extra;
            }
        }
        px += cw;
        i += 1;
    }
    if s.reply_len == 0 {
        g.text(cx, py + 10, "输入问题后按 Enter，AI 可执行系统操作。", rgb(140, 148, 168), 1);
        g.text(cx, py + 34, "示例：把亮度调到 50，或打开终端。", rgb(140, 148, 168), 1);
    }
    let iy = py + ph + 10;
    g.round_rect(cx - 8, iy, w - 52, 36, 8, rgb(255, 255, 255), 255);
    g.round_rect_outline(cx - 8, iy, w - 52, 36, 8, rgb(180, 200, 230), 220);
    if s.input_len == 0 {
        g.text(cx + 4, iy + 10, "输入消息…（C 配置 API）", rgb(150, 160, 180), 1);
    } else {
        g.text(cx + 4, iy + 10, core::str::from_utf8_unchecked(&s.input[..s.input_len]), rgb(36, 42, 58), 1);
        if (super::TICK / 16) % 2 == 0 {
            g.rect(cx + 4 + s.input_len as i32 * 8, iy + 8, 2, 18, rgb(60, 116, 240), 220);
        }
    }
    let st = core::str::from_utf8_unchecked(&s.status[..s.status_len]);
    g.text(cx, iy + 44, st, if s.status_ok { rgb(36, 160, 88) } else { rgb(192, 72, 68) }, 1);
    g.text(cx, iy + 64, "Enter 发送 · Esc 清空 · C 配置", rgb(140, 148, 168), 1);
}