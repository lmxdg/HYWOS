#![allow(dead_code)]

//! HYWOffice：打开并预览 txt / doc / docx / ppt / pptx。
//! docx/pptx 通过 ZIP + DEFLATE + XML 提取文本；doc/ppt 做可打印文本兼容提取。
//! 不保留原始排版、图片与表格结构，只做文本预览。

use crate::fs::{self, DirEntry};
use crate::gfx::{next_utf8, rgb, Gfx};

pub const MAX_FILES: usize = 16;
pub const TEXT_MAX: usize = 16384;
const INFLATE_MAX: usize = 512 * 1024;

static mut INFLATE_BUF: [u8; INFLATE_MAX] = [0; INFLATE_MAX];

pub const KIND_NONE: u8 = 0;
pub const KIND_TXT: u8 = 1;
pub const KIND_DOC: u8 = 2;
pub const KIND_DOCX: u8 = 3;
pub const KIND_PPT: u8 = 4;
pub const KIND_PPTX: u8 = 5;

pub struct OfficeState {
    pub name_buf: [[u8; 80]; MAX_FILES],
    pub name_len: [u8; MAX_FILES],
    pub count: usize,
    pub sel: usize,
    pub opened: bool,
    pub title: [u8; 96],
    pub title_len: usize,
    pub kind: u8,
    pub text: [u8; TEXT_MAX],
    pub text_len: usize,
    pub scroll: usize,
    pub msg: [u8; 96],
    pub msg_len: usize,
    pub msg_ok: bool,
    pub splash_frames: u8,
    pub editing: bool,
    pub path: [u8; 128],
    pub path_len: usize,
}

fn lower(c: u8) -> u8 {
    if c >= b'A' && c <= b'Z' { c + 32 } else { c }
}

fn ends_ci(s: &[u8], ext: &[u8]) -> bool {
    if s.len() < ext.len() { return false; }
    let off = s.len() - ext.len();
    let mut i = 0;
    while i < ext.len() {
        if lower(s[off + i]) != lower(ext[i]) { return false; }
        i += 1;
    }
    true
}

pub fn kind_of(name: &[u8]) -> u8 {
    if ends_ci(name, b".txt") { return KIND_TXT; }
    if ends_ci(name, b".docx") { return KIND_DOCX; }
    if ends_ci(name, b".pptx") { return KIND_PPTX; }
    if ends_ci(name, b".doc") { return KIND_DOC; }
    if ends_ci(name, b".ppt") { return KIND_PPT; }
    KIND_NONE
}

fn kind_name(k: u8) -> &'static str {
    match k {
        KIND_TXT => "纯文本",
        KIND_DOC => "Word 97-2003",
        KIND_DOCX => "Word 文档",
        KIND_PPT => "PowerPoint 97-2003",
        KIND_PPTX => "PowerPoint 演示文稿",
        _ => "未知",
    }
}

impl OfficeState {
    pub fn empty() -> OfficeState {
        unsafe { core::mem::zeroed() }
    }

    fn set_msg(&mut self, ok: bool, m: &str) {
        self.msg_len = 0;
        for c in m.bytes() {
            if self.msg_len < self.msg.len() {
                self.msg[self.msg_len] = c;
                self.msg_len += 1;
            }
        }
        self.msg_ok = ok;
    }

    fn set_title(&mut self, name: &[u8], kind: u8) {
        self.title_len = 0;
        for c in name {
            if self.title_len < 60 {
                self.title[self.title_len] = *c;
                self.title_len += 1;
            }
        }
        for c in b"  -  ".iter() {
            if self.title_len < 70 { self.title[self.title_len] = *c; self.title_len += 1; }
        }
        for c in kind_name(kind).bytes() {
            if self.title_len < self.title.len() { self.title[self.title_len] = c; self.title_len += 1; }
        }
    }

    pub unsafe fn scan(&mut self) {
        self.count = 0;
        self.sel = 0;
        self.opened = false;
        self.text_len = 0;
        self.scroll = 0;
        let mut ents: [DirEntry; 32] = core::mem::zeroed();
        let mut n = fs::list_dir(fs::usr_vol(), "\\home\\user\\Documents", &mut ents);
        if n == 0 {
            n = fs::list_dir(fs::usr_vol(), "\\home\\user", &mut ents);
        }
        let mut i = 0;
        while i < n && self.count < MAX_FILES {
            let nm = ents[i].name.as_str();
            if !ents[i].is_dir && kind_of(nm.as_bytes()) != KIND_NONE {
                let b = nm.as_bytes();
                let mut k = 0;
                while k < b.len() && k < 79 {
                    self.name_buf[self.count][k] = b[k];
                    k += 1;
                }
                self.name_len[self.count] = k as u8;
                self.count += 1;
            }
            i += 1;
        }
        if self.count == 0 {
            self.set_msg(false, "未找到文档：把 doc/docx/txt/ppt/pptx 放入 Documents");
        } else {
            self.set_msg(true, "选择左侧文件打开；支持 doc/docx/txt/ppt/pptx");
        }
    }

    pub unsafe fn open(&mut self, idx: usize) {
        if idx >= self.count {
            return;
        }
        let nl = self.name_len[idx] as usize;
        let mut path = [0u8; 128];
        let mut l = 0;
        for c in b"\\home\\user\\Documents\\" { path[l] = *c; l += 1; }
        let mut k = 0;
        while k < nl { path[l] = self.name_buf[idx][k]; l += 1; k += 1; }
        let mut n = fs::read_file(fs::usr_vol(), core::str::from_utf8_unchecked(&path[..l]));
        if n.is_none() {
            l = 0;
            for c in b"\\home\\user\\" { path[l] = *c; l += 1; }
            k = 0;
            while k < nl { path[l] = self.name_buf[idx][k]; l += 1; k += 1; }
            n = fs::read_file(fs::usr_vol(), core::str::from_utf8_unchecked(&path[..l]));
        }
        let sz = match n {
            Some(sz) => sz,
            None => {
                self.set_msg(false, "无法读取文件");
                return;
            }
        };
        let kind = kind_of(&self.name_buf[idx][..nl]);
        self.path_len = if l < 128 { l } else { 127 };
        let mut pi = 0;
        while pi < self.path_len { self.path[pi] = path[pi]; pi += 1; }
        self.kind = kind;
        self.text_len = 0;
        self.scroll = 0;
        let data = &fs::rdbuf()[..sz];
        match kind {
            KIND_TXT => copy_text(data, &mut self.text, &mut self.text_len),
            KIND_DOCX => {
                let out = &mut *core::ptr::addr_of_mut!(INFLATE_BUF);
                let ln = zip_find_and_unzip(data, b"word/document.xml", out);
                if ln > 0 {
                    xml_extract(&out[..ln], b"w:t", b"w:p", &mut self.text, &mut self.text_len);
                }
            }
            KIND_PPTX => {
                pptx_extract(data, &mut self.text, &mut self.text_len);
            }
            KIND_DOC | KIND_PPT => {
                heuristic_text(data, &mut self.text, &mut self.text_len);
            }
            _ => {}
        }
        if self.text_len == 0 {
            self.set_msg(false, "未能提取文本（可能为加密、空文档或复杂格式）");
            self.opened = false;
        } else {
            self.opened = true;
            self.splash_frames = 14;
            self.editing = false;
            let mut nm = [0u8; 80];
            let mut k = 0;
            while k < nl { nm[k] = self.name_buf[idx][k]; k += 1; }
            self.set_title(&nm[..nl], kind);
            self.set_msg(true, "只读文本预览；不保留原始排版与图片");
        }
    }

    pub unsafe fn key(&mut self, scan: u16, ch: u16) {
        if self.editing {
            if scan == 0x17 {
                self.editing = false;
                self.set_msg(true, "已退出编辑，文件未自动保存");
                return;
            }
            if ch == 19 {
                self.save();
                return;
            }
            if ch == 8 {
                if self.text_len > 0 { self.text_len -= 1; }
                return;
            }
            if ch == 13 {
                if self.text_len < self.text.len() { self.text[self.text_len] = b'\n'; self.text_len += 1; }
                return;
            }
            if ch >= 32 && ch < 127 && self.text_len < self.text.len() {
                self.text[self.text_len] = ch as u8;
                self.text_len += 1;
            }
            return;
        }
        if self.opened && self.kind == KIND_TXT && (ch == b'e' as u16 || ch == b'E' as u16) {
            self.editing = true;
            self.set_msg(true, "编辑中：Ctrl+S 保存，Esc 退出");
            return;
        }
        if (scan == 0x50 || scan == 0x02) && self.sel + 1 < self.count {
            self.sel += 1;
        } else if (scan == 0x48 || scan == 0x01) && self.sel > 0 {
            self.sel -= 1;
        } else if ch == 13 {
            self.open(self.sel);
        } else if scan == 0x49 || scan == 0x09 {
            if self.scroll > 0 { self.scroll -= 1; }
        } else if scan == 0x51 || scan == 0x0a {
            self.scroll += 1;
        }
    }

    unsafe fn save(&mut self) {
        if self.path_len == 0 {
            self.set_msg(false, "没有可保存的文件路径");
            return;
        }
        let p = core::str::from_utf8_unchecked(&self.path[..self.path_len]);
        if fs::write_file(fs::usr_vol(), p, &self.text[..self.text_len]) {
            self.set_msg(true, "已保存到原文件");
        } else {
            self.set_msg(false, "保存失败");
        }
    }
}

fn copy_text(data: &[u8], out: &mut [u8], len: &mut usize) {
    *len = 0;
    let mut i = 0;
    while i < data.len() && *len < out.len() {
        let c = data[i];
        if c != b'\r' {
            out[*len] = c;
            *len += 1;
        }
        i += 1;
    }
}

fn push(out: &mut [u8], len: &mut usize, b: u8) {
    if *len < out.len() {
        out[*len] = b;
        *len += 1;
    }
}

fn tag_at(xml: &[u8], pos: usize, tag: &[u8]) -> bool {
    if pos + tag.len() > xml.len() { return false; }
    let mut i = 0;
    while i < tag.len() {
        if xml[pos + i] != tag[i] { return false; }
        i += 1;
    }
    let after = pos + tag.len();
    if after < xml.len() {
        let c = xml[after];
        if c != b'>' && c != b'/' && c != b' ' && c != b'\t' && c != b'\n' && c != b'\r' {
            return false;
        }
    }
    true
}

fn xml_text(s: &[u8], out: &mut [u8], len: &mut usize) {
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'&' {
            if i + 5 <= s.len() && s[i + 1] == b'a' && s[i + 2] == b'm' && s[i + 3] == b'p' && s[i + 4] == b';' {
                push(out, len, b'&'); i += 5; continue;
            }
            if i + 4 <= s.len() && s[i + 1] == b'l' && s[i + 2] == b't' && s[i + 3] == b';' {
                push(out, len, b'<'); i += 4; continue;
            }
            if i + 4 <= s.len() && s[i + 1] == b'g' && s[i + 2] == b't' && s[i + 3] == b';' {
                push(out, len, b'>'); i += 4; continue;
            }
            if i + 6 <= s.len() && s[i + 1] == b'q' && s[i + 2] == b'u' && s[i + 3] == b'o' && s[i + 4] == b't' && s[i + 5] == b';' {
                push(out, len, b'"'); i += 6; continue;
            }
        }
        push(out, len, s[i]);
        i += 1;
    }
}

fn xml_extract(xml: &[u8], ttag: &[u8], ptag: &[u8], out: &mut [u8], len: &mut usize) {
    let mut i = 0;
    while i < xml.len() {
        if xml[i] != b'<' {
            i += 1;
            continue;
        }
        if i + 1 < xml.len() && xml[i + 1] == b'/' {
            if tag_at(xml, i + 2, ptag) {
                push(out, len, b'\n');
            }
        } else if tag_at(xml, i + 1, ttag) {
            let mut j = i + 1;
            while j < xml.len() && xml[j] != b'>' { j += 1; }
            if j < xml.len() { j += 1; }
            let start = j;
            while j < xml.len() && xml[j] != b'<' { j += 1; }
            xml_text(&xml[start..j], out, len);
            i = j;
            continue;
        }
        while i < xml.len() && xml[i] != b'>' { i += 1; }
        i += 1;
    }
}

fn heuristic_text(data: &[u8], out: &mut [u8], len: &mut usize) {
    *len = 0;
    // OLE 文档文本多为 UTF-16LE，先提取连续可打印序列。
    let mut i = 0;
    while i + 1 < data.len() && *len < out.len() {
        if data[i] >= 32 && data[i] < 127 && data[i + 1] == 0 {
            let s = i;
            while i + 1 < data.len() && data[i] >= 32 && data[i] < 127 && data[i + 1] == 0 {
                i += 2;
            }
            let n = (i - s) / 2;
            if n >= 3 {
                let mut k = s;
                while k < i {
                    push(out, len, data[k]);
                    k += 2;
                }
                push(out, len, b'\n');
            }
        } else {
            i += 1;
        }
    }
    if *len >= 64 {
        return;
    }
    // 回退：提取 ASCII 可打印串。
    i = 0;
    while i < data.len() && *len < out.len() {
        if data[i] >= 32 && data[i] < 127 {
            let s = i;
            while i < data.len() && data[i] >= 32 && data[i] < 127 { i += 1; }
            if i - s >= 4 {
                let mut k = s;
                while k < i {
                    push(out, len, data[k]);
                    k += 1;
                }
                push(out, len, b'\n');
            }
        } else {
            i += 1;
        }
    }
}

fn rd16(d: &[u8], o: usize) -> usize {
    d[o] as usize | ((d[o + 1] as usize) << 8)
}

fn rd32(d: &[u8], o: usize) -> usize {
    d[o] as usize | ((d[o + 1] as usize) << 8) | ((d[o + 2] as usize) << 16) | ((d[o + 3] as usize) << 24)
}

fn find_eocd(d: &[u8]) -> Option<usize> {
    if d.len() < 22 { return None; }
    let mut i = d.len() - 22;
    loop {
        if rd32(d, i) == 0x06054b50 {
            return Some(i);
        }
        if i == 0 { break; }
        i -= 1;
    }
    None
}

fn unzip_entry(data: &[u8], method: usize, csize: usize, loff: usize, out: &mut [u8]) -> usize {
    if loff + 30 > data.len() || rd32(data, loff) != 0x04034b50 {
        return 0;
    }
    let nl = rd16(data, loff + 26);
    let el = rd16(data, loff + 28);
    let doff = loff + 30 + nl + el;
    if doff + csize > data.len() {
        return 0;
    }
    let comp = &data[doff..doff + csize];
    match method {
        0 => {
            let n = if csize < out.len() { csize } else { out.len() };
            out[..n].copy_from_slice(&comp[..n]);
            n
        }
        8 => inflate(comp, out),
        _ => 0,
    }
}

unsafe fn zip_find_and_unzip(data: &[u8], target: &[u8], out: &mut [u8]) -> usize {
    let eocd = match find_eocd(data) {
        Some(x) => x,
        None => return 0,
    };
    let total = rd16(data, eocd + 10);
    let mut off = rd32(data, eocd + 16);
    let mut i = 0;
    while i < total {
        if off + 46 > data.len() || rd32(data, off) != 0x02014b50 { break; }
        let method = rd16(data, off + 10);
        let csize = rd32(data, off + 20);
        let nlen = rd16(data, off + 28);
        let elen = rd16(data, off + 30);
        let clen = rd16(data, off + 32);
        let loff = rd32(data, off + 42);
        if off + 46 + nlen <= data.len() {
            let name = &data[off + 46..off + 46 + nlen];
            if name == target {
                return unzip_entry(data, method, csize, loff, out);
            }
        }
        off += 46 + nlen + elen + clen;
        i += 1;
    }
    0
}

unsafe fn pptx_extract(data: &[u8], out: &mut [u8], len: &mut usize) {
    let eocd = match find_eocd(data) {
        Some(x) => x,
        None => return,
    };
    let total = rd16(data, eocd + 10);
    let mut off = rd32(data, eocd + 16);
    let mut i = 0;
    let mut slides = 0;
    while i < total && slides < 24 {
        if off + 46 > data.len() || rd32(data, off) != 0x02014b50 { break; }
        let method = rd16(data, off + 10);
        let csize = rd32(data, off + 20);
        let nlen = rd16(data, off + 28);
        let elen = rd16(data, off + 30);
        let clen = rd16(data, off + 32);
        let loff = rd32(data, off + 42);
        if off + 46 + nlen <= data.len() {
            let name = &data[off + 46..off + 46 + nlen];
            if name.len() > 18 && &name[..15] == b"ppt/slides/slid" && ends_ci(name, b".xml") {
                let buf = &mut *core::ptr::addr_of_mut!(INFLATE_BUF);
                let n = unzip_entry(data, method, csize, loff, buf);
                if n > 0 {
                    xml_extract(&buf[..n], b"a:t", b"a:p", out, len);
                    push(out, len, b'\n');
                    slides += 1;
                }
            }
        }
        off += 46 + nlen + elen + clen;
        i += 1;
    }
}

/* ---------------- DEFLATE / inflate ---------------- */

struct BitR<'a> {
    d: &'a [u8],
    pos: usize,
    buf: u32,
    n: u32,
}

impl<'a> BitR<'a> {
    fn new(d: &'a [u8]) -> BitR<'a> {
        BitR { d, pos: 0, buf: 0, n: 0 }
    }
    fn bit(&mut self) -> u32 {
        if self.n == 0 {
            self.buf = if self.pos < self.d.len() { self.d[self.pos] as u32 } else { 0 };
            self.pos += 1;
            self.n = 8;
        }
        let b = self.buf & 1;
        self.buf >>= 1;
        self.n -= 1;
        b
    }
    fn bits(&mut self, n: u32) -> u32 {
        let mut v = 0;
        let mut i = 0;
        while i < n {
            v |= self.bit() << i;
            i += 1;
        }
        v
    }
    fn align(&mut self) {
        self.n = 0;
    }
    fn byte(&mut self) -> u8 {
        self.align();
        let b = if self.pos < self.d.len() { self.d[self.pos] } else { 0 };
        self.pos += 1;
        b
    }
}

struct Huff {
    counts: [u16; 16],
    symbols: [u16; 288],
}

impl Huff {
    fn new() -> Huff {
        Huff { counts: [0; 16], symbols: [0; 288] }
    }
}

fn build_huff(lengths: &[u8], h: &mut Huff) {
    h.counts = [0; 16];
    let mut i = 0;
    while i < lengths.len() {
        h.counts[lengths[i] as usize] += 1;
        i += 1;
    }
    h.counts[0] = 0;
    let mut offs = [0u16; 16];
    let mut k = 1;
    while k < 16 {
        offs[k] = offs[k - 1] + h.counts[k - 1];
        k += 1;
    }
    i = 0;
    while i < lengths.len() {
        let l = lengths[i] as usize;
        if l != 0 {
            h.symbols[offs[l] as usize] = i as u16;
            offs[l] += 1;
        }
        i += 1;
    }
}

fn decode(br: &mut BitR, h: &Huff) -> u32 {
    let mut code = 0u32;
    let mut first = 0u32;
    let mut index = 0usize;
    let mut len = 1;
    while len < 16 {
        code |= br.bit();
        let count = h.counts[len] as u32;
        if code >= first && code - first < count {
            return h.symbols[index + (code - first) as usize] as u32;
        }
        index += count as usize;
        first = (first + count) << 1;
        code <<= 1;
        len += 1;
    }
    0
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
    131, 163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
    13, 13,
];

fn fixed_tables(lit: &mut Huff, dist: &mut Huff) {
    let mut ll = [0u8; 288];
    let mut i = 0;
    while i < 288 {
        ll[i] = if i < 144 { 8 } else if i < 256 { 9 } else if i < 280 { 7 } else { 8 };
        i += 1;
    }
    build_huff(&ll, lit);
    let mut dl = [0u8; 32];
    i = 0;
    while i < 30 {
        dl[i] = 5;
        i += 1;
    }
    build_huff(&dl, dist);
}

fn inflate_block(br: &mut BitR, lit: &Huff, dist: &Huff, out: &mut [u8], pos: &mut usize) -> bool {
    loop {
        let sym = decode(br, lit);
        if sym < 256 {
            if *pos >= out.len() { return false; }
            out[*pos] = sym as u8;
            *pos += 1;
        } else if sym == 256 {
            return true;
        } else {
            let li = (sym - 257) as usize;
            if li >= 29 { return false; }
            let len = LEN_BASE[li] as usize + br.bits(LEN_EXTRA[li] as u32) as usize;
            let ds = decode(br, dist) as usize;
            if ds >= 30 { return false; }
            let distance = DIST_BASE[ds] as usize + br.bits(DIST_EXTRA[ds] as u32) as usize;
            if distance > *pos { return false; }
            let mut k = 0;
            while k < len {
                if *pos >= out.len() { return false; }
                out[*pos] = out[*pos - distance];
                *pos += 1;
                k += 1;
            }
        }
    }
}

pub fn inflate(data: &[u8], out: &mut [u8]) -> usize {
    let mut br = BitR::new(data);
    let mut pos = 0usize;
    let mut lit = Huff::new();
    let mut dist = Huff::new();
    loop {
        let last = br.bit();
        let typ = br.bits(2);
        if typ == 0 {
            br.align();
            let len = br.bits(16) as usize;
            let _nlen = br.bits(16);
            let mut k = 0;
            while k < len {
                if pos >= out.len() { return 0; }
                out[pos] = br.byte();
                pos += 1;
                k += 1;
            }
        } else if typ == 1 {
            fixed_tables(&mut lit, &mut dist);
            if !inflate_block(&mut br, &lit, &dist, out, &mut pos) { return 0; }
        } else if typ == 2 {
            let hlit = br.bits(5) as usize + 257;
            let hdist = br.bits(5) as usize + 1;
            let hclen = br.bits(4) as usize + 4;
            let order = [16usize, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
            let mut cl = [0u8; 19];
            let mut i = 0;
            while i < hclen {
                cl[order[i]] = br.bits(3) as u8;
                i += 1;
            }
            let mut clh = Huff::new();
            build_huff(&cl, &mut clh);
            let mut lens = [0u8; 320];
            i = 0;
            while i < hlit + hdist {
                let sym = decode(&mut br, &clh);
                if sym < 16 {
                    if i < 320 { lens[i] = sym as u8; }
                    i += 1;
                } else if sym == 16 {
                    let prev = if i > 0 { lens[i - 1] } else { 0 };
                    let rep = 3 + br.bits(2) as usize;
                    let mut k = 0;
                    while k < rep && i < 320 { lens[i] = prev; i += 1; k += 1; }
                } else if sym == 17 {
                    let rep = 3 + br.bits(3) as usize;
                    let mut k = 0;
                    while k < rep && i < 320 { lens[i] = 0; i += 1; k += 1; }
                } else if sym == 18 {
                    let rep = 11 + br.bits(7) as usize;
                    let mut k = 0;
                    while k < rep && i < 320 { lens[i] = 0; i += 1; k += 1; }
                } else {
                    return 0;
                }
            }
            if hlit > 288 || hdist > 32 { return 0; }
            build_huff(&lens[..hlit], &mut lit);
            build_huff(&lens[hlit..hlit + hdist], &mut dist);
            if !inflate_block(&mut br, &lit, &dist, out, &mut pos) { return 0; }
        } else {
            return 0;
        }
        if last == 1 { break; }
    }
    pos
}

/* ---------------- 绘制与交互 ---------------- */

fn char_w(cp: u32) -> i32 {
    if cp < 128 { 8 } else { 16 }
}

unsafe fn draw_splash(g: &Gfx, x: i32, y: i32, w: i32, h: i32, fr: u8) {
    g.round_rect(x, y, w, h, 18, rgb(14, 20, 34), 255);
    let cx = x + w / 2;
    let cy = y + h / 2;
    g.blit_rgba(cx - 48, cy - 96, 96, 96, &crate::logo::LOGO_RGBA, crate::logo::LOGO_SIZE as i32, crate::logo::LOGO_SIZE as i32);
    g.text_center(cx, cy + 16, "HYWOffice", rgb(226, 240, 255), 2);
    g.text_center(cx, cy + 48, "正在打开文档…", rgb(150, 168, 196), 1);
    let t = (14 - fr) as f32 * 0.5;
    let mut i = 0;
    while i < 8 {
        let a = t + i as f32 * 0.35;
        let a2 = a + 0.2;
        let x0 = cx + (crate::gfx::cosf(a) * 14.0) as i32;
        let y0 = cy + 80 + (crate::gfx::sinf(a) * 14.0) as i32;
        let x1 = cx + (crate::gfx::cosf(a2) * 14.0) as i32;
        let y1 = cy + 80 + (crate::gfx::sinf(a2) * 14.0) as i32;
        g.line(x0, y0, x1, y1, rgb(120, 200, 255), (40 + i * 20) as u8);
        i += 1;
    }
}

pub unsafe fn draw(s: &mut OfficeState, g: &Gfx, x: i32, y: i32, w: i32, h: i32) {
    if s.splash_frames > 0 {
        s.splash_frames -= 1;
        draw_splash(g, x, y, w, h, s.splash_frames);
        return;
    }
    let top = y + 40 + 8;
    let list_w = 220i32;
    g.round_rect(x + 10, top, list_w, h - 40 - 24, 10, rgb(238, 242, 250), 255);
    g.text(x + 22, top + 10, "文档", rgb(40, 48, 64), 1);
    if s.count == 0 {
        g.text(x + 22, top + 36, "（空）", rgb(150, 158, 176), 1);
    } else {
        let mut i = 0;
        while i < s.count {
            let ry = top + 34 + i as i32 * 28;
            if ry > top + h - 70 { break; }
            if i == s.sel {
                g.round_rect(x + 16, ry - 4, list_w - 12, 24, 6, rgb(206, 222, 250), 255);
            }
            let nm = core::str::from_utf8_unchecked(&s.name_buf[i][..s.name_len[i] as usize]);
            g.text(x + 24, ry, nm, if i == s.sel { rgb(34, 88, 196) } else { rgb(50, 56, 72) }, 1);
            i += 1;
        }
    }
    g.text(x + 22, top + h - 40, "单击列表打开 · ↑↓ 选择", rgb(140, 148, 168), 1);

    let cx = x + list_w + 26;
    let cy = top + 6;
    let cw = w - list_w - 40;
    if s.opened {
        let t = core::str::from_utf8_unchecked(&s.title[..s.title_len]);
        g.text(cx, cy, t, rgb(36, 42, 58), 1);
        g.text(cx, cy + 20, core::str::from_utf8_unchecked(&s.msg[..s.msg_len]), rgb(140, 148, 168), 1);
        let py = cy + 44;
        let ph = h - 40 - 24 - 44;
        g.round_rect(cx - 6, py - 6, cw + 12, ph + 12, 10, rgb(250, 252, 255), 255);
        g.round_rect_outline(cx - 6, py - 6, cw + 12, ph + 12, 10, rgb(214, 222, 236), 200);
        let maxw = cw - 16;
        let maxlines = ph / 18;
        let mut i = 0usize;
        let mut line = 0usize;
        let mut px = 0i32;
        let mut yy = py;
        while i < s.text_len {
            let cp = next_utf8(&s.text[..s.text_len], &mut i);
            if cp == 10 {
                line += 1;
                px = 0;
                continue;
            }
            let cwid = char_w(cp);
            if px + cwid > maxw {
                line += 1;
                px = 0;
            }
            if line >= s.scroll && line < s.scroll + maxlines as usize {
                yy = py + (line - s.scroll) as i32 * 18;
                if cp < 128 {
                    g.glyph(cx + px, yy, cp as u8, rgb(36, 42, 58), 1, 240);
                } else {
                    g.cjk_glyph(cx + px, yy, cp, rgb(36, 42, 58), 1, 240);
                }
            }
            px += cwid;
            if line >= s.scroll + maxlines as usize + 1 { break; }
        }
        let _ = yy;
    } else {
        g.text(cx, cy, "HYWOffice", rgb(36, 42, 58), 2);
        g.text(cx, cy + 40, "支持 txt / doc / docx / ppt / pptx", rgb(50, 56, 72), 1);
        g.text(cx, cy + 64, "选择左侧文件后显示文本预览", rgb(140, 148, 168), 1);
        g.text(cx, cy + 88, "docx/pptx 通过 ZIP 解压与 XML 提取文本", rgb(140, 148, 168), 1);
        g.text(cx, cy + 112, "doc/ppt 为二进制兼容提取，不保留排版", rgb(140, 148, 168), 1);
        if s.msg_len > 0 {
            let m = core::str::from_utf8_unchecked(&s.msg[..s.msg_len]);
            g.text(cx, cy + 150, m, if s.msg_ok { rgb(36, 160, 88) } else { rgb(192, 72, 68) }, 1);
        }
    }
}

pub unsafe fn click(s: &mut OfficeState, mx: i32, my: i32, x: i32, y: i32) {
    let top = y + 40 + 8;
    let list_w = 220i32;
    if mx < x + 10 || mx > x + 10 + list_w { return; }
    let ry = my - (top + 34);
    if ry < 0 { return; }
    let idx = (ry / 28) as usize;
    if idx < s.count {
        s.sel = idx;
        s.open(idx);
    }
}

pub unsafe fn key(s: &mut OfficeState, scan: u16, ch: u16) {
    s.key(scan, ch);
}