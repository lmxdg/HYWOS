#![allow(dead_code)]

use crate::fs::{self, Str80};
use crate::version::{RELEASES_API, UPDATE_DIR, VERSION_MAJOR, VERSION_MINOR, VERSION_PATCH};

const MAX_NET: usize = 262144;
const MAX_PKG: usize = 4 * 1024 * 1024;

static mut NETBUF: [u8; MAX_NET] = [0; MAX_NET];
static mut UPDBUF: [u8; MAX_PKG] = [0; MAX_PKG];

#[derive(Clone, Copy)]
pub struct Release {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub tag: [u8; 40],
    pub tag_len: usize,
    pub url: [u8; 260],
    pub url_len: usize,
}

impl Release {
    pub fn empty() -> Release {
        Release {
            major: 0,
            minor: 0,
            patch: 0,
            tag: [0; 40],
            tag_len: 0,
            url: [0; 260],
            url_len: 0,
        }
    }
    pub fn version_str(&self, buf: &mut [u8]) -> usize {
        let mut l = hw_num(buf, self.major);
        buf[l] = b'.'; l += 1;
        l += hw_num(&mut buf[l..], self.minor);
        buf[l] = b'.'; l += 1;
        l += hw_num(&mut buf[l..], self.patch);
        l
    }
    pub fn version_code(&self) -> u32 {
        self.major * 10000 + self.minor * 100 + self.patch
    }
}

fn hw_num(buf: &mut [u8], mut v: u32) -> usize {
    if v == 0 {
        buf[0] = b'0';
        return 1;
    }
    let mut tmp = [0u8; 10];
    let mut n = 0;
    while v > 0 {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    let mut i = 0;
    while i < n {
        buf[i] = tmp[n - 1 - i];
        i += 1;
    }
    n
}

pub fn local_code() -> u32 {
    VERSION_MAJOR * 10000 + VERSION_MINOR * 100 + VERSION_PATCH
}

fn parse_version(s: &[u8]) -> (u32, u32, u32) {
    let mut v = [0u32; 3];
    let mut part = 0usize;
    let mut any = false;
    for c in s.iter() {
        if *c >= b'0' && *c <= b'9' {
            if part < 3 {
                v[part] = v[part] * 10 + (*c - b'0') as u32;
            }
            any = true;
        } else if *c == b'.' {
            if part < 2 { part += 1; }
        } else if any {
            break;
        }
    }
    (v[0], v[1], v[2])
}

fn find_from(data: &[u8], key: &[u8], from: usize) -> Option<usize> {
    let mut i = from;
    while i + key.len() <= data.len() {
        let mut k = 0;
        while k < key.len() && data[i + k] == key[k] { k += 1; }
        if k == key.len() { return Some(i); }
        i += 1;
    }
    None
}

fn read_string_at(data: &[u8], at: usize, key: &[u8], out: &mut [u8]) -> Option<usize> {
    let mut p = at + key.len();
    while p < data.len() && (data[p] == b' ' || data[p] == b'\t' || data[p] == b':') { p += 1; }
    if p >= data.len() || data[p] != b'"' { return None; }
    p += 1;
    let mut n = 0usize;
    while p < data.len() && data[p] != b'"' && n + 1 < out.len() {
        if data[p] == b'\\' && p + 1 < data.len() { p += 1; }
        out[n] = data[p];
        n += 1;
        p += 1;
    }
    Some(n)
}

fn find_asset(data: &[u8], start: usize, end: usize) -> Option<(usize, usize)> {
    let key = b"\"browser_download_url\"";
    let mut i = start;
    while i + key.len() <= end {
        let mut k = 0;
        while k < key.len() && data[i + k] == key[k] { k += 1; }
        if k == key.len() {
            let mut p = i + key.len();
            while p < end && (data[p] == b' ' || data[p] == b'\t' || data[p] == b':') { p += 1; }
            if p < end && data[p] == b'"' {
                p += 1;
                let s0 = p;
                while p < end && data[p] != b'"' { p += 1; }
                if p > s0 {
                    let slice = &data[s0..p];
                    let mut has = false;
                    let mut q = 0;
                    while q + 4 <= slice.len() {
                        let tail = &slice[q..];
                        if tail.starts_with(b".updpkg") || tail.starts_with(b".upd") || tail.starts_with(b".UPD") {
                            has = true;
                            break;
                        }
                        q += 1;
                    }
                    if has { return Some((s0, p - s0)); }
                }
            }
        }
        i += 1;
    }
    None
}

fn evaluate(data: &[u8], out: &mut Release) -> Option<u32> {
    let key = b"\"tag_name\"";
    let mut search = 0usize;
    let mut best_code = 0u32;
    let mut have = false;
    loop {
        let at = match find_from(data, key, search) { Some(x) => x, None => break };
        let mut tag = [0u8; 40];
        let tl = match read_string_at(data, at, key, &mut tag) { Some(x) => x, None => { search = at + 1; continue; } };
        let next = find_from(data, key, at + 1).unwrap_or(data.len());
        let (maj, min, pat) = parse_version(&tag[..tl]);
        let code = maj * 10000 + min * 100 + pat;
        if !have || code > best_code {
            have = true;
            best_code = code;
            out.major = maj;
            out.minor = min;
            out.patch = pat;
            out.tag_len = tl;
            let mut i = 0;
            while i < tl { out.tag[i] = tag[i]; i += 1; }
            out.url_len = 0;
            if let Some((s0, len)) = find_asset(data, at, next) {
                let l = if len < out.url.len() - 1 { len } else { out.url.len() - 1 };
                let mut k = 0;
                while k < l { out.url[k] = data[s0 + k]; k += 1; }
                out.url_len = l;
            }
        }
        search = at + 1;
    }
    if have { Some(best_code) } else { None }
}

fn scan_local_packages(out: &mut Release) -> Option<u32> {
    let mut path = Str80::empty();
    path.push_str(UPDATE_DIR);
    let mut ents: [fs::DirEntry; 32] = unsafe { core::mem::zeroed() };
    let n = unsafe { fs::list_dir(fs::usr_vol(), path.as_str(), &mut ents) };
    let mut best = 0u32;
    let mut found = false;
    let mut i = 0;
    while i < n {
        let name = ents[i].name.as_str();
        if name.ends_with(".updpkg") {
            let full = fs::path_join(path.as_str(), name);
            if let Some(len) = unsafe { fs::read_file(fs::usr_vol(), full.as_str()) } {
                let buf = unsafe { fs::rdbuf() };
                let info = unsafe { fs::parse_updpkg(&buf[..len]) };
                if info.valid {
                    let code = info.version;
                    if code > best {
                        best = code;
                        out.major = code / 10000;
                        out.minor = (code / 100) % 100;
                        out.patch = code % 100;
                        out.url_len = 0;
                        for c in full.as_str().bytes() {
                            if out.url_len < out.url.len() - 1 {
                                out.url[out.url_len] = c;
                                out.url_len += 1;
                            }
                        }
                        found = true;
                    }
                }
            }
        }
        i += 1;
    }
    if found { Some(best) } else { None }
}

pub enum Check {
    Latest(u32, u32, u32),
    Available(Release),
    Error(u32),
}

pub unsafe fn check_update() -> Check {
    let local = local_code();
    let mut seen_code = 0u32;
    let mut seen = Release::empty();
    let mut any = false;
    let mut rel = Release::empty();
    let buf = &mut *core::ptr::addr_of_mut!(NETBUF);
    if let Ok(n) = crate::http::get(RELEASES_API, buf) {
        if let Some(code) = evaluate(&buf[..n], &mut rel) {
            if code > local {
                return Check::Available(rel);
            }
            if !any || code > seen_code {
                any = true;
                seen_code = code;
                seen = rel;
            }
        }
    }
    let mut cache = Str80::empty();
    cache.push_str(UPDATE_DIR);
    cache.push(b'\\');
    cache.push_str("latest.json");
    if let Some(n) = fs::read_file(fs::usr_vol(), cache.as_str()) {
        let buf = fs::rdbuf();
        let mut rel = Release::empty();
        if let Some(code) = evaluate(&buf[..n], &mut rel) {
            if code > local {
                return Check::Available(rel);
            }
            if !any || code > seen_code {
                any = true;
                seen_code = code;
                seen = rel;
            }
        }
    }
    let mut local_pkg = Release::empty();
    if let Some(code) = scan_local_packages(&mut local_pkg) {
        if code > local {
            return Check::Available(local_pkg);
        }
        if !any || code > seen_code {
            any = true;
            seen_code = code;
            seen = local_pkg;
        }
    }
    if any {
        return Check::Latest(seen.major, seen.minor, seen.patch);
    }
    Check::Error(1)
}

unsafe fn local_pkg_for(version: u32) -> Option<Str80> {
    let mut dir = Str80::empty();
    dir.push_str(UPDATE_DIR);
    let mut ents: [fs::DirEntry; 32] = core::mem::zeroed();
    let n = fs::list_dir(fs::usr_vol(), dir.as_str(), &mut ents);
    let mut i = 0;
    while i < n {
        let name = ents[i].name.as_str();
        let is_pkg = name.ends_with(".updpkg") || name.ends_with(".upd") || name.ends_with(".UPD");
        if is_pkg {
            let full = fs::path_join(dir.as_str(), name);
            if let Some(len) = fs::read_file(fs::usr_vol(), full.as_str()) {
                let buf = fs::rdbuf();
                let info = fs::parse_updpkg(&buf[..len]);
                if info.valid && info.version == version {
                    return Some(full);
                }
            }
        }
        i += 1;
    }
    None
}

pub unsafe fn download_and_apply(rel: &Release) -> u8 {
    if rel.url_len == 0 {
        return 10;
    }
    let mut dir = Str80::empty();
    dir.push_str(UPDATE_DIR);
    fs::ensure_dir(fs::usr_vol(), dir.as_str());
    let url = core::str::from_utf8_unchecked(&rel.url[..rel.url_len]);
    if url.starts_with('\\') {
        let code = fs::apply_update_file(fs::usr_vol(), url);
        if code != 4 && code != 1 {
            return code;
        }
        if let Some(p) = local_pkg_for(rel.version_code()) {
            return fs::apply_update_file(fs::usr_vol(), p.as_str());
        }
        return 11;
    }
    let mut name = Str80::empty();
    name.push_str(dir.as_str());
    name.push(b'\\');
    name.push_str("hywos-");
    let mut vb = [0u8; 24];
    let vl = rel.version_str(&mut vb);
    let mut i = 0;
    while i < vl { name.push(vb[i]); i += 1; }
    name.push_str(".updpkg");
    let buf = &mut *core::ptr::addr_of_mut!(UPDBUF);
    match crate::http::get(url, buf) {
        Ok(n) => {
            if !fs::write_file(fs::usr_vol(), name.as_str(), &buf[..n]) {
                return 12;
            }
            fs::apply_update(&buf[..n])
        }
        Err(e) => (20u32 + e) as u8,
    }
}