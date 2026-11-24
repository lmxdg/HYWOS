use crate::efi::*;

#[repr(C)]
pub struct FileSystem {
    pub revision: u64,
    pub open_volume: usize,
}

#[repr(C)]
pub struct FileProtocol {
    pub revision: u64,
    pub open: usize,
    pub close: usize,
    pub delete: usize,
    pub read: usize,
    pub write: usize,
    pub get_position: usize,
    pub set_position: usize,
    pub get_info: usize,
    pub set_info: usize,
    pub flush: usize,
}

#[repr(C)]
pub struct EfiTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub pad1: u8,
    pub nanosecond: u32,
    pub timezone: i16,
    pub daylight: u8,
    pub pad2: u8,
}

#[repr(C)]
pub struct FileInfo {
    pub size: u64,
    pub file_size: u64,
    pub phys_size: u64,
    pub create: EfiTime,
    pub access: EfiTime,
    pub modify: EfiTime,
    pub attr: u64,
    pub name: [u16; 1],
}

#[repr(C)]
pub struct BlockIoMedia {
    pub media_id: u32,
    pub removable: u8,
    pub present: u8,
    pub logical_partition: u8,
    pub read_only: u8,
    pub write_caching: u8,
    pub pad1: [u8; 3],
    pub block_size: u32,
    pub io_align: u32,
    pub last_block: u64,
}

#[repr(C)]
pub struct BlockIo {
    pub revision: u64,
    pub media: *mut BlockIoMedia,
    pub reset: usize,
    pub read_blocks: usize,
    pub write_blocks: usize,
    pub flush: usize,
}

pub const MODE_READ: u64 = 1;
pub const MODE_WRITE: u64 = 2;
pub const MODE_CREATE: u64 = 0x8000_0000_0000_0000;
pub const ATTR_DIR: u64 = 0x10;
pub const FILEBUF_SIZE: usize = 65536;
pub const BMP_MAX: usize = 3 * 1024 * 1024;

static mut VOLS: [*mut FileProtocol; 8] = [core::ptr::null_mut(); 8];
static mut VOL_COUNT: usize = 0;
static mut SYS_VOL: usize = 0;
static mut USR_VOL: usize = 0;
static mut RDBUF: [u8; FILEBUF_SIZE] = [0; FILEBUF_SIZE];
static mut BMPBUF: [u8; BMP_MAX] = [0; BMP_MAX];
static mut SHOT_NUM: u32 = 0;
static mut BLK: *mut BlockIo = core::ptr::null_mut();

pub unsafe fn path_utf16(path: &str, buf: &mut [u16; 160]) -> usize {
    to_utf16(path, buf)
}

unsafe fn open_on(root: *mut FileProtocol, path: &str, mode: u64, attr: u64) -> *mut FileProtocol {
    if root.is_null() { return core::ptr::null_mut(); }
    let mut buf = [0u16; 160];
    path_utf16(path, &mut buf);
    let f: extern "efiapi" fn(*mut FileProtocol, *mut *mut FileProtocol, *const u16, u64, u64) -> Status =
        core::mem::transmute((*root).open);
    let mut out: *mut FileProtocol = core::ptr::null_mut();
    let st = f(root, &mut out, buf.as_ptr(), mode, attr);
    if st == EFI_SUCCESS { out } else { core::ptr::null_mut() }
}

unsafe fn close(f: *mut FileProtocol) {
    if f.is_null() { return; }
    let g: extern "efiapi" fn(*mut FileProtocol) -> Status = core::mem::transmute((*f).close);
    g(f);
}

pub unsafe fn exists_vol(vol: usize, path: &str) -> bool {
    if USE_RAM { return ram_exists(path); }
    let f = open_on(VOLS[vol], path, MODE_READ, 0);
    if f.is_null() {
        false
    } else {
        close(f);
        true
    }
}

pub unsafe fn vol_writable(vol: usize) -> bool {
    let ok = write_file(vol, "\\_hywos_probe.tmp", b"probe");
    if ok {
        let _ = delete_file(vol, "\\_hywos_probe.tmp");
    }
    ok
}

pub unsafe fn fs_init() -> usize {
    bs_connect_all_storage();
    let mut count: usize = 0;
    let mut buf: *mut Handle = core::ptr::null_mut();
    let mut n: usize = 0;
    if !status_ok(bs_locate_handle_buffer(&GUID_SIMPLE_FS, &mut n, &mut buf)) {
        return 0;
    }
    let mut i = 0;
    while i < n && count < 8 {
        let mut fs: *mut FileSystem = core::ptr::null_mut();
        if status_ok(bs_handle_protocol(*buf.add(i), &GUID_SIMPLE_FS, &mut fs as *mut *mut FileSystem as *mut *mut core::ffi::c_void)) && !fs.is_null() {
            let f: extern "efiapi" fn(*mut FileSystem, *mut *mut FileProtocol) -> Status =
                core::mem::transmute((*fs).open_volume);
            let mut root: *mut FileProtocol = core::ptr::null_mut();
            if f(fs, &mut root) == EFI_SUCCESS && !root.is_null() {
                VOLS[count] = root;
                count += 1;
            }
        }
        i += 1;
    }
    VOL_COUNT = count;
    if count > 0 {
        let mut sys = 0;
        let mut k = 0;
        while k < count {
            if exists_vol(k, "\\EFI\\BOOT\\BOOTX64.EFI") || exists_vol(k, "\\EFI\\BOOT\\BOOTAA64.EFI") {
                sys = k;
                break;
            }
            k += 1;
        }
        SYS_VOL = sys;
        let mut any = false;
        let mut j = 0;
        while j < count {
            if vol_writable(j) { any = true; }
            j += 1;
        }
        let mut usr = sys;
        if !any {
            USE_RAM = true;
            serial_print("[fs] no writable volume, RAM fallback");
        } else {
            let mut k = 0;
            while k < count {
                if k != sys && !exists_vol(k, "\\EFI") && vol_writable(k) {
                    usr = k;
                    break;
                }
                k += 1;
            }
        }
        USR_VOL = usr;
        serial_print("[fs] sys index");
        serial_u32(sys as u32);
        serial_print("[fs] usr index");
        serial_u32(usr as u32);
        serial_print("[fs] total volumes");
        serial_u32(count as u32);
        let mut j2 = 0;
        while j2 < count {
            serial_print("[fs] vol writable");
            serial_u32(j2 as u32);
            serial_print(if vol_writable(j2) { "yes" } else { "no" });
            j2 += 1;
        }
    } else {
        USE_RAM = true;
        serial_print("[fs] no volumes, RAM fallback");
    }
    count
}


pub unsafe fn open_sys(path: &str) -> Option<*mut FileProtocol> {
    let f = open_on(VOLS[SYS_VOL], path, MODE_READ, 0);
    if f.is_null() { None } else { Some(f) }
}

pub unsafe fn close_f(f: *mut FileProtocol) {
    close(f);
}

pub unsafe fn file_size(f: *mut FileProtocol) -> Option<usize> {
    let gi: extern "efiapi" fn(*mut FileProtocol, *mut Guid, *mut usize, *mut u8) -> Status =
        core::mem::transmute((*f).get_info);
    let mut buf = [0u8; 512];
    let mut sz = 512usize;
    let mut g = GUID_FILE_INFO;
    if gi(f, &mut g, &mut sz, buf.as_mut_ptr()) != EFI_SUCCESS {
        return None;
    }
    let size = core::ptr::read_unaligned(buf.as_ptr().add(8) as *const u64);
    Some(size as usize)
}

pub unsafe fn read_at(f: *mut FileProtocol, off: u64, buf: &mut [u8]) -> Option<usize> {
    let sp: extern "efiapi" fn(*mut FileProtocol, u64) -> Status =
        core::mem::transmute((*f).set_position);
    if sp(f, off) != EFI_SUCCESS {
        return None;
    }
    let r: extern "efiapi" fn(*mut FileProtocol, *mut usize, *mut u8) -> Status =
        core::mem::transmute((*f).read);
    let mut sz = buf.len();
    if r(f, &mut sz, buf.as_mut_ptr()) == EFI_SUCCESS {
        Some(sz)
    } else {
        None
    }
}
pub unsafe fn is_install_media() -> bool {
    let mut k = 0;
    while k < VOL_COUNT {
        if exists_vol(k, "\\CDMARK.TXT") || exists_vol(k, "\\HYWOS\\CD") {
            return true;
        }
        k += 1;
    }
    let mut buf: *mut Handle = core::ptr::null_mut();
    let mut n: usize = 0;
    if status_ok(bs_locate_handle_buffer(&GUID_BLOCK_IO, &mut n, &mut buf)) && !buf.is_null() {
        let mut i = 0;
        while i < n {
            let mut b: *mut BlockIo = core::ptr::null_mut();
            if status_ok(bs_handle_protocol(
                *buf.add(i),
                &GUID_BLOCK_IO,
                &mut b as *mut *mut BlockIo as *mut *mut core::ffi::c_void,
            )) && !b.is_null()
            {
                let m = (*b).media;
                if !m.is_null() && (*m).present == 1 && (*m).block_size == 2048 {
                    return true;
                }
            }
            i += 1;
        }
    }
    false
}

pub unsafe fn sys_vol() -> usize { SYS_VOL }
pub unsafe fn usr_vol() -> usize { USR_VOL }
pub unsafe fn vol_count() -> usize { VOL_COUNT }

unsafe fn mkdirs(vol: usize, path: &str) {
    let bytes = path.as_bytes();
    let mut cur = [0u8; 160];
    let mut clen = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' {
            if clen > 0 {
                let name = core::str::from_utf8_unchecked(&cur[..clen]);
                let f = open_on(VOLS[vol], name, MODE_CREATE | MODE_WRITE, ATTR_DIR);
                if !f.is_null() { close(f); }
            }
        } else if clen < 158 {
            cur[clen] = c;
            clen += 1;
        }
        i += 1;
    }
    if clen > 0 {
        let name = core::str::from_utf8_unchecked(&cur[..clen]);
        let f = open_on(VOLS[vol], name, MODE_CREATE | MODE_WRITE, ATTR_DIR);
        if !f.is_null() { close(f); }
    }
}

pub unsafe fn ensure_dir(vol: usize, path: &str) {
    if USE_RAM { ram_mkdir(path); return; }
    mkdirs(vol, path);
}

pub unsafe fn read_file(vol: usize, path: &str) -> Option<usize> {
    if USE_RAM { return ram_read(path); }
    let f = open_on(VOLS[vol], path, MODE_READ, 0);
    if f.is_null() { return None; }
    let r: extern "efiapi" fn(*mut FileProtocol, *mut usize, *mut u8) -> Status =
        core::mem::transmute((*f).read);
    let buf = &mut *core::ptr::addr_of_mut!(RDBUF);
    let mut sz = buf.len();
    let st = r(f, &mut sz, buf.as_mut_ptr());
    close(f);
    if st == EFI_SUCCESS { Some(sz) } else { None }
}

pub unsafe fn rdbuf() -> &'static mut [u8] {
    &mut *core::ptr::addr_of_mut!(RDBUF)
}

pub unsafe fn write_file(vol: usize, path: &str, data: &[u8]) -> bool {
    if USE_RAM { return ram_write(path, data); }
    let parent = parent_of(path);
    if !parent.is_empty() { mkdirs(vol, parent.as_str()); }
    let f = open_on(VOLS[vol], path, MODE_WRITE | MODE_CREATE, 0);
    if f.is_null() { return false; }
    let w: extern "efiapi" fn(*mut FileProtocol, *mut usize, *mut u8) -> Status =
        core::mem::transmute((*f).write);
    let mut sz = data.len();
    let st = w(f, &mut sz, data.as_ptr() as *mut u8);
    let fl: extern "efiapi" fn(*mut FileProtocol) -> Status = core::mem::transmute((*f).flush);
    fl(f);
    close(f);
    st == EFI_SUCCESS
}

pub unsafe fn append_file(vol: usize, path: &str, data: &[u8]) -> bool {
    if USE_RAM {
        let mut tmp = [0u8; 8192];
        let mut l = 0usize;
        if let Some(n) = ram_read(path) {
            let rb = &*core::ptr::addr_of!(RDBUF);
            let c = if n < 8192 { n } else { 8192 };
            while l < c { tmp[l] = rb[l]; l += 1; }
        }
        let mut k = 0;
        while k < data.len() && l + k < 8192 { tmp[l + k] = data[k]; k += 1; }
        return ram_write(path, &tmp[..l + k]);
    }
    let mut existing_len = 0usize;
    if let Some(n) = read_file(vol, path) {
        existing_len = n;
    }
    let f = open_on(VOLS[vol], path, MODE_WRITE | MODE_CREATE, 0);
    if f.is_null() { return false; }
    let sp: extern "efiapi" fn(*mut FileProtocol, u64) -> Status = core::mem::transmute((*f).set_position);
    sp(f, existing_len as u64);
    let w: extern "efiapi" fn(*mut FileProtocol, *mut usize, *mut u8) -> Status =
        core::mem::transmute((*f).write);
    let mut sz = data.len();
    let st = w(f, &mut sz, data.as_ptr() as *mut u8);
    let fl: extern "efiapi" fn(*mut FileProtocol) -> Status = core::mem::transmute((*f).flush);
    fl(f);
    close(f);
    st == EFI_SUCCESS
}

pub unsafe fn delete_file(vol: usize, path: &str) -> bool {
    if USE_RAM { return ram_delete(path); }
    let f = open_on(VOLS[vol], path, MODE_WRITE, 0);
    if f.is_null() { return false; }
    let d: extern "efiapi" fn(*mut FileProtocol) -> Status = core::mem::transmute((*f).delete);
    let st = d(f);
    st == EFI_SUCCESS
}

pub struct Str80 {
    pub b: [u8; 80],
    pub len: usize,
}

impl Str80 {
    pub fn empty() -> Str80 { Str80 { b: [0; 80], len: 0 } }
    pub fn as_str(&self) -> &str {
        unsafe { core::str::from_utf8_unchecked(&self.b[..self.len]) }
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn push(&mut self, c: u8) {
        if self.len < 79 { self.b[self.len] = c; self.len += 1; }
    }
    pub fn push_str(&mut self, s: &str) {
        for c in s.bytes() { self.push(c); }
    }
}

pub struct DirEntry {
    pub name: Str80,
    pub is_dir: bool,
    pub size: u64,
}

pub unsafe fn list_dir(vol: usize, path: &str, out: &mut [DirEntry; 32]) -> usize {
    if USE_RAM { return ram_list(path, out); }
    let f = open_on(VOLS[vol], path, MODE_READ, 0);
    if f.is_null() { return 0; }
    let sp: extern "efiapi" fn(*mut FileProtocol, u64) -> Status = core::mem::transmute((*f).set_position);
    sp(f, 0);
    let r: extern "efiapi" fn(*mut FileProtocol, *mut usize, *mut u8) -> Status =
        core::mem::transmute((*f).read);
    let mut n = 0usize;
    let mut info = [0u8; 512];
    loop {
        let mut sz = info.len();
        let st = r(f, &mut sz, info.as_mut_ptr());
        if st != EFI_SUCCESS || sz < 82 { break; }
        let attr = core::ptr::read_unaligned(info.as_ptr().add(72) as *const u64);
        let name_ptr = info.as_ptr().add(80) as *const u16;
        let mut e = DirEntry { name: Str80::empty(), is_dir: (attr & 0x10) != 0, size: core::ptr::read_unaligned(info.as_ptr().add(8) as *const u64) };
        let mut i = 0;
        while i < 78 {
            let c = *name_ptr.add(i);
            if c == 0 { break; }
            e.name.push(if c < 128 { c as u8 } else { b'?' });
            i += 1;
        }
        if e.name.len > 0 && e.name.as_str() != "." && e.name.as_str() != ".." {
            out[n] = e;
            n += 1;
            if n >= 32 { break; }
        }
    }
    close(f);
    n
}

pub fn parent_of(path: &str) -> Str80 {
    let mut s = Str80::empty();
    let b = path.as_bytes();
    let mut last = 0;
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' { last = i; }
        i += 1;
    }
    i = 0;
    while i < last {
        s.push(b[i]);
        i += 1;
    }
    s
}

pub fn file_name(path: &str) -> Str80 {
    let mut s = Str80::empty();
    let b = path.as_bytes();
    let mut i = b.len();
    while i > 0 {
        if b[i - 1] == b'\\' {
            let mut k = i;
            while k < b.len() { s.push(b[k]); k += 1; }
            return s;
        }
        i -= 1;
    }
    s.push_str(path);
    s
}

pub fn path_join(dir: &str, name: &str) -> Str80 {
    let mut s = Str80::empty();
    s.push_str(dir);
    if s.len > 0 && s.b[s.len - 1] != b'\\' { s.push(b'\\'); }
    s.push_str(name);
    s
}

pub const HOME: &str = "\\home\\user";
pub const APPS_DIR: &str = "\\home\\user\\.apps";
pub const SHOT_DIR: &str = "\\home\\user\\Screenshots";
pub const PERM_FILE: &str = "\\home\\user\\.apps\\_permissions";

pub unsafe fn init_home() {
    let v = USR_VOL;
    ensure_dir(v, "\\home");
    ensure_dir(v, HOME);
    ensure_dir(v, APPS_DIR);
    ensure_dir(v, SHOT_DIR);
    ensure_dir(v, "\\home\\user\\Documents");
    ensure_dir(v, "\\home\\user\\Pictures");
    if read_file(v, PERM_FILE).is_none() {
        let d = b"# appid storage notify network\n";
        write_file(v, PERM_FILE, d);
    }
}

pub const PERM_STORAGE: u8 = 0;
pub const PERM_NOTIFY: u8 = 1;
pub const PERM_NETWORK: u8 = 2;

pub fn perm_index(which: u8) -> usize {
    which as usize
}

pub unsafe fn perm_get(appid: &str, which: u8) -> bool {
    let n = match read_file(USR_VOL, PERM_FILE) {
        Some(n) => n,
        None => return true,
    };
    let buf = &*core::ptr::addr_of!(RDBUF);
    let mut line_start = 0usize;
    let mut i = 0;
    while i < n {
        if buf[i] == b'\n' || i == n - 1 {
            let end = if buf[i] == b'\n' { i } else { i + 1 };
            if line_start < end && buf[line_start] != b'#' {
                let mut p = line_start;
                let mut id = [0u8; 32];
                let mut il = 0;
                while p < end && buf[p] != b' ' {
                    if il < 31 { id[il] = buf[p]; il += 1; }
                    p += 1;
                }
                if &id[..il] == appid.as_bytes() {
                    let mut flags = [1u8; 3];
                    let mut f = 0;
                    while p < end && f < 3 {
                        while p < end && buf[p] == b' ' { p += 1; }
                        if p < end {
                            flags[f] = if buf[p] == b'1' { 1 } else { 0 };
                            f += 1;
                            while p < end && buf[p] != b' ' { p += 1; }
                        }
                    }
                    return flags[which as usize] == 1;
                }
            }
            line_start = i + 1;
        }
        i += 1;
    }
    true
}

pub unsafe fn perm_set(appid: &str, which: u8, on: bool) {
    let mut out = [0u8; 4096];
    let mut len = 0usize;
    let mut found = false;
    let mut old_flags = [true, true, true];
    old_flags[0] = perm_get(appid, 0);
    old_flags[1] = perm_get(appid, 1);
    old_flags[2] = perm_get(appid, 2);
    if let Some(n) = read_file(USR_VOL, PERM_FILE) {
        let buf = &*core::ptr::addr_of!(RDBUF);
        let mut line_start = 0usize;
        let mut i = 0;
        while i <= n {
            if i == n || buf[i] == b'\n' {
                let end = i;
                let mut is_app = false;
                if line_start < end && buf[line_start] != b'#' {
                    let mut p = line_start;
                    let mut id = [0u8; 32];
                    let mut il = 0;
                    while p < end && buf[p] != b' ' {
                        if il < 31 { id[il] = buf[p]; il += 1; }
                        p += 1;
                    }
                    if &id[..il] == appid.as_bytes() { is_app = true; }
                }
                if is_app && !found {
                    found = true;
                    let mut pid = Str80::empty();
                    pid.push_str(appid);
                    let mut flags = old_flags;
                    flags[which as usize] = on;
                    let mut line = Str80::empty();
                    line.push_str(pid.as_str());
                    let mut f = 0;
                    while f < 3 {
                        line.push(b' ');
                        line.push(if flags[f] { b'1' } else { b'0' });
                        f += 1;
                    }
                    for b in line.b[..line.len].iter() {
                        if len < 4095 { out[len] = *b; len += 1; }
                    }
                    if len < 4095 { out[len] = b'\n'; len += 1; }
                } else if line_start < end || is_app {
                    let mut p = line_start;
                    while p < end && len < 4095 { out[len] = buf[p]; len += 1; p += 1; }
                    if len < 4095 { out[len] = b'\n'; len += 1; }
                }
                line_start = i + 1;
                if i == n { break; }
            }
            i += 1;
        }
    }
    if !found {
        let mut line = Str80::empty();
        line.push_str(appid);
        let mut flags = [true, true, true];
        flags[which as usize] = on;
        let mut f = 0;
        while f < 3 {
            line.push(b' ');
            line.push(if flags[f] { b'1' } else { b'0' });
            f += 1;
        }
        for b in line.b[..line.len].iter() {
            if len < 4095 { out[len] = *b; len += 1; }
        }
        if len < 4095 { out[len] = b'\n'; len += 1; }
    }
    let mut hdr = Str80::empty();
    hdr.push_str("# appid storage notify network\n");
    let mut final_buf = [0u8; 4200];
    let mut fl = 0;
    for b in hdr.b[..hdr.len].iter() {
        final_buf[fl] = *b; fl += 1;
    }
    for b in out[..len].iter() {
        final_buf[fl] = *b; fl += 1;
    }
    write_file(USR_VOL, PERM_FILE, &final_buf[..fl]);
}

pub unsafe fn storage_allowed(appid: &str) -> bool {
    perm_get(appid, PERM_STORAGE)
}

pub fn kv_path(appid: &str) -> Str80 {
    let mut s = Str80::empty();
    s.push_str(APPS_DIR);
    s.push(b'\\');
    s.push_str(appid);
    s.push_str(".kv");
    s
}

pub unsafe fn appstore_set(appid: &str, key: &str, val: &str) -> bool {
    if !storage_allowed(appid) { return false; }
    if val.len() > 120 { return false; }
    let p = kv_path(appid);
    let mut out = [0u8; 8192];
    let mut len = 0usize;
    let mut replaced = false;
    let mut existing = 0usize;
    if let Some(n) = read_file(USR_VOL, p.as_str()) {
        existing = n;
        let buf = &*core::ptr::addr_of!(RDBUF);
        let mut i = 0;
        while i <= existing {
            if i == existing || buf[i] == b'\n' {
                let end = i;
                let mut eq = usize::MAX;
                let mut j = end;
                while j > 0 && buf[j - 1] != b'\n' { j -= 1; }
                let ls = j;
                let mut k = ls;
                while k < end {
                    if buf[k] == b'=' { eq = k; break; }
                    k += 1;
                }
                if eq != usize::MAX && eq - ls == key.len() && &buf[ls..eq] == key.as_bytes() {
                    replaced = true;
                    for c in key.bytes() { if len < 8191 { out[len] = c; len += 1; } }
                    if len < 8191 { out[len] = b'='; len += 1; }
                    for c in val.bytes() { if len < 8191 { out[len] = c; len += 1; } }
                    if len < 8191 { out[len] = b'\n'; len += 1; }
                } else if end > ls {
                    for c in buf[ls..end].iter() { if len < 8191 { out[len] = *c; len += 1; } }
                    if len < 8191 { out[len] = b'\n'; len += 1; }
                }
                if i == existing { break; }
            }
            i += 1;
        }
    }
    if !replaced {
        for c in key.bytes() { if len < 8191 { out[len] = c; len += 1; } }
        if len < 8191 { out[len] = b'='; len += 1; }
        for c in val.bytes() { if len < 8191 { out[len] = c; len += 1; } }
        if len < 8191 { out[len] = b'\n'; len += 1; }
    }
    write_file(USR_VOL, p.as_str(), &out[..len])
}

pub unsafe fn appstore_get(appid: &str, key: &str, out: &mut [u8]) -> Option<usize> {
    if !storage_allowed(appid) { return None; }
    let p = kv_path(appid);
    let n = read_file(USR_VOL, p.as_str())?;
    let buf = &*core::ptr::addr_of!(RDBUF);
    let mut ls = 0usize;
    let mut i = 0;
    while i <= n {
        if i == n || buf[i] == b'\n' {
            let end = i;
            let mut eq = usize::MAX;
            let mut k = ls;
            while k < end {
                if buf[k] == b'=' { eq = k; break; }
                k += 1;
            }
            if eq != usize::MAX && eq - ls == key.len() && &buf[ls..eq] == key.as_bytes() {
                let vlen = end - eq - 1;
                let l = if vlen < out.len() { vlen } else { out.len() };
                let mut j = 0;
                while j < l { out[j] = buf[eq + 1 + j]; j += 1; }
                return Some(l);
            }
            if i == n { break; }
            ls = i + 1;
        }
        i += 1;
    }
    None
}

pub unsafe fn appstore_del(appid: &str, key: &str) -> bool {
    if !storage_allowed(appid) { return false; }
    let p = kv_path(appid);
    let mut out = [0u8; 8192];
    let mut len = 0usize;
    let mut found = false;
    if let Some(n) = read_file(USR_VOL, p.as_str()) {
        let buf = &*core::ptr::addr_of!(RDBUF);
        let mut ls = 0usize;
        let mut i = 0;
        while i <= n {
            if i == n || buf[i] == b'\n' {
                let end = i;
                let mut eq = usize::MAX;
                let mut k = ls;
                while k < end {
                    if buf[k] == b'=' { eq = k; break; }
                    k += 1;
                }
                if eq != usize::MAX && eq - ls == key.len() && &buf[ls..eq] == key.as_bytes() {
                    found = true;
                } else if end > ls {
                    for c in buf[ls..end].iter() { if len < 8191 { out[len] = *c; len += 1; } }
                    if len < 8191 { out[len] = b'\n'; len += 1; }
                }
                if i == n { break; }
                ls = i + 1;
            }
            i += 1;
        }
    }
    if !found { return false; }
    write_file(USR_VOL, p.as_str(), &out[..len])
}

pub unsafe fn screenshot(pixels: &[u32], w: usize, h: usize) -> Option<u32> {
    if w * h > 1024 * 768 { return None; }
    let row_bytes = w * 3;
    let pad = (4 - row_bytes % 4) % 4;
    let stride = row_bytes + pad;
    let img_size = stride * h;
    if img_size + 54 > BMP_MAX { return None; }
    let bmp = &mut *core::ptr::addr_of_mut!(BMPBUF);
    bmp[..2].copy_from_slice(b"BM");
    let total = (54 + img_size) as u32;
    bmp[2] = total as u8; bmp[3] = (total >> 8) as u8; bmp[4] = (total >> 16) as u8; bmp[5] = (total >> 24) as u8;
    bmp[10] = 54;
    let dib = 40u32;
    bmp[14] = dib as u8;
    bmp[18] = w as u8; bmp[19] = (w >> 8) as u8; bmp[20] = (w >> 16) as u8; bmp[21] = (w >> 24) as u8;
    bmp[22] = h as u8; bmp[23] = (h >> 8) as u8; bmp[24] = (h >> 16) as u8; bmp[25] = (h >> 24) as u8;
    bmp[26] = 1;
    bmp[28] = 24;
    bmp[34] = img_size as u8; bmp[35] = (img_size >> 8) as u8; bmp[36] = (img_size >> 16) as u8; bmp[37] = (img_size >> 24) as u8;
    bmp[38] = 0x13; bmp[39] = 0x0b;
    bmp[42] = 0x13; bmp[43] = 0x0b;
    let mut y = 0;
    while y < h {
        let src_row = h - 1 - y;
        let dst = 54 + y * stride;
        let mut x = 0;
        while x < w {
            let c = pixels[src_row * w + x];
            let o = dst + x * 3;
            bmp[o] = (c & 0xff) as u8;
            bmp[o + 1] = ((c >> 8) & 0xff) as u8;
            bmp[o + 2] = ((c >> 16) & 0xff) as u8;
            x += 1;
        }
        y += 1;
    }
    mkdirs(USR_VOL, SHOT_DIR);
    let mut n = SHOT_NUM + 1;
    let mut path = Str80::empty();
    loop {
        path = Str80::empty();
        path.push_str(SHOT_DIR);
        path.push(b'\\');
        path.push_str("shot-");
        let mut digits = [0u8; 4];
        let mut v = n;
        let mut d = 3i32;
        while d >= 0 {
            digits[d as usize] = b'0' + (v % 10) as u8;
            v /= 10;
            d -= 1;
        }
        for c in digits.iter() { path.push(*c); }
        path.push_str(".bmp");
        if !exists_vol(USR_VOL, path.as_str()) { break; }
        n += 1;
        if n > 9999 { return None; }
    }
    if write_file(USR_VOL, path.as_str(), &bmp[..54 + img_size]) {
        SHOT_NUM = n;
        Some(n)
    } else {
        None
    }
}

pub unsafe fn find_block_device() -> *mut BlockIo {
    if !BLK.is_null() { return BLK; }
    let mut buf: *mut Handle = core::ptr::null_mut();
    let mut n: usize = 0;
    if !status_ok(bs_locate_handle_buffer(&GUID_BLOCK_IO, &mut n, &mut buf)) {
        return core::ptr::null_mut();
    }
    let mut i = 0;
    while i < n {
        let mut bio: *mut BlockIo = core::ptr::null_mut();
        if status_ok(bs_handle_protocol(*buf.add(i), &GUID_BLOCK_IO, &mut bio as *mut *mut BlockIo as *mut *mut core::ffi::c_void)) && !bio.is_null() {
            let m = (*bio).media;
            if !m.is_null() && (*m).present == 1 && (*m).read_only == 0 && (*m).block_size == 512 && (*m).last_block > 256 {
                BLK = bio;
                return bio;
            }
        }
        i += 1;
    }
    core::ptr::null_mut()
}

pub unsafe fn block_info() -> Option<(u64, u32)> {
    let b = find_block_device();
    if b.is_null() { return None; }
    let m = (*b).media;
    Some(((*m).last_block, (*m).block_size))
}

pub unsafe fn write_lba(lba: u64, data: &[u8]) -> bool {
    let b = find_block_device();
    if b.is_null() { return false; }
    let m = (*b).media;
    let bs = (*m).block_size as usize;
    let blocks = (data.len() + bs - 1) / bs;
    if lba + blocks as u64 > (*m).last_block { return false; }
    let mut buf = [0u8; 4096];
    if data.len() > buf.len() { return false; }
    let mut i = 0;
    while i < data.len() { buf[i] = data[i]; i += 1; }
    let w: extern "efiapi" fn(*mut BlockIo, u32, u64, usize, *mut u8) -> Status =
        core::mem::transmute((*b).write_blocks);
    let st = w(b, (*m).media_id, lba, blocks * bs, buf.as_mut_ptr());
    let fl: extern "efiapi" fn(*mut BlockIo) -> Status = core::mem::transmute((*b).flush);
    fl(b);
    st == EFI_SUCCESS
}

pub struct UpdateInfo {
    pub valid: bool,
    pub version: u32,
    pub payload_len: usize,
    pub lba: u32,
    pub crc_ok: bool,
    pub payload_off: usize,
}

pub unsafe fn parse_updpkg(data: &[u8]) -> UpdateInfo {
    let mut info = UpdateInfo { valid: false, version: 0, payload_len: 0, lba: 64, crc_ok: false, payload_off: 32 };
    if data.len() < 32 { return info; }
    let magic = b"CONLYUPD";
    let mut i = 0;
    while i < 8 {
        if data[i] != magic[i] { return info; }
        i += 1;
    }
    let ver = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
    let plen = u32::from_le_bytes([data[12], data[13], data[14], data[15]]) as usize;
    let lba = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);
    let crc = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
    if 32 + plen > data.len() || plen == 0 || plen > 32 * 1024 { return info; }
    let mut payload = [0u8; 32 * 1024];
    let mut k = 0;
    while k < plen { payload[k] = data[32 + k]; k += 1; }
    let got = bs_crc32(payload.as_mut_ptr(), plen);
    info.valid = true;
    info.version = ver;
    info.payload_len = plen;
    info.lba = lba;
    info.crc_ok = got == crc;
    info
}

pub unsafe fn apply_update(data: &[u8]) -> u8 {
    let info = parse_updpkg(data);
    if !info.valid { return 1; }
    if !info.crc_ok { return 2; }
    let mut payload = [0u8; 32 * 1024];
    let mut k = 0;
    while k < info.payload_len { payload[k] = data[32 + k]; k += 1; }
    if write_lba(info.lba as u64, &payload[..info.payload_len]) { 0 } else { 3 }
}

pub static mut USE_RAM: bool = false;
const RAM_MAX_FILES: usize = 24;
const RAM_MAX_BYTES: usize = 6 * 1024 * 1024;
const RAM_MAX_DIRS: usize = 48;

static mut RAM_BUF: [u8; RAM_MAX_BYTES] = [0; RAM_MAX_BYTES];
static mut RAM_OFF: [usize; RAM_MAX_FILES] = [0; RAM_MAX_FILES];
static mut RAM_LEN: [usize; RAM_MAX_FILES] = [0; RAM_MAX_FILES];
static mut RAM_PATH: [[u8; 80]; RAM_MAX_FILES] = [[0; 80]; RAM_MAX_FILES];
static mut RAM_PLEN: [u8; RAM_MAX_FILES] = [0; RAM_MAX_FILES];
static mut RAM_N: usize = 0;
static mut RAM_USED: usize = 0;
static mut RAM_DIRS: [[u8; 80]; RAM_MAX_DIRS] = [[0; 80]; RAM_MAX_DIRS];
static mut RAM_DLEN: [u8; RAM_MAX_DIRS] = [0; RAM_MAX_DIRS];
static mut RAM_DN: usize = 0;

fn ram_norm(path: &str) -> ([u8; 80], usize) {
    let mut b = [0u8; 80];
    let mut n = 0usize;
    for c in path.bytes() {
        if n < 79 {
            b[n] = if c == b'/' { b'\\' } else { c };
            n += 1;
        }
    }
    (b, n)
}

unsafe fn ram_find(b: &[u8], n: usize) -> Option<usize> {
    let paths = &*core::ptr::addr_of!(RAM_PATH);
    let lens = &*core::ptr::addr_of!(RAM_PLEN);
    let sizes = &*core::ptr::addr_of!(RAM_LEN);
    let mut i = 0;
    while i < RAM_N {
        if lens[i] as usize == n && sizes[i] > 0 && &paths[i][..n] == b {
            return Some(i);
        }
        i += 1;
    }
    None
}

unsafe fn ram_is_dir(b: &[u8], n: usize) -> bool {
    let dirs = &*core::ptr::addr_of!(RAM_DIRS);
    let dl = &*core::ptr::addr_of!(RAM_DLEN);
    let mut i = 0;
    while i < RAM_DN {
        if dl[i] as usize == n && &dirs[i][..n] == b {
            return true;
        }
        i += 1;
    }
    false
}

pub unsafe fn ram_mkdir(path: &str) {
    let (b, n) = ram_norm(path);
    if ram_is_dir(&b, n) { return; }
    if RAM_DN >= RAM_MAX_DIRS { return; }
    let i = RAM_DN;
    let mut k = 0;
    while k < n { RAM_DIRS[i][k] = b[k]; k += 1; }
    RAM_DLEN[i] = n as u8;
    RAM_DN += 1;
}

pub unsafe fn ram_write(path: &str, data: &[u8]) -> bool {
    let (b, n) = ram_norm(path);
    if let Some(i) = ram_find(&b, n) {
        if RAM_LEN[i] >= data.len() {
            let off = RAM_OFF[i];
            let buf = &mut *core::ptr::addr_of_mut!(RAM_BUF);
            let mut k = 0;
            while k < data.len() { buf[off + k] = data[k]; k += 1; }
            RAM_LEN[i] = data.len();
            return true;
        }
    }
    if RAM_N >= RAM_MAX_FILES || RAM_USED + data.len() > RAM_MAX_BYTES {
        return false;
    }
    let off = RAM_USED;
    let buf = &mut *core::ptr::addr_of_mut!(RAM_BUF);
    let mut k = 0;
    while k < data.len() { buf[off + k] = data[k]; k += 1; }
    RAM_USED += data.len();
    let mut idx = usize::MAX;
    if let Some(i) = ram_find(&b, n) { idx = i; }
    if idx == usize::MAX {
        idx = RAM_N;
        RAM_N += 1;
    }
    let mut k = 0;
    while k < n { RAM_PATH[idx][k] = b[k]; k += 1; }
    RAM_PLEN[idx] = n as u8;
    RAM_OFF[idx] = off;
    RAM_LEN[idx] = data.len();
    true
}

pub unsafe fn ram_read(path: &str) -> Option<usize> {
    let (b, n) = ram_norm(path);
    let i = ram_find(&b, n)?;
    let len = RAM_LEN[i];
    let off = RAM_OFF[i];
    let src = &*core::ptr::addr_of!(RAM_BUF);
    let dst = &mut *core::ptr::addr_of_mut!(RDBUF);
    let l = if len < dst.len() { len } else { dst.len() };
    let mut k = 0;
    while k < l { dst[k] = src[off + k]; k += 1; }
    Some(len)
}

pub unsafe fn ram_delete(path: &str) -> bool {
    let (b, n) = ram_norm(path);
    if let Some(i) = ram_find(&b, n) {
        RAM_LEN[i] = 0;
        true
    } else {
        false
    }
}

pub unsafe fn ram_exists(path: &str) -> bool {
    let (b, n) = ram_norm(path);
    ram_find(&b, n).is_some() || ram_is_dir(&b, n)
}

unsafe fn ram_add_entry(out: &mut [DirEntry; 32], count: &mut usize, name: &[u8], is_dir: bool, size: u64) {
    if *count >= 32 { return; }
    let mut j = 0;
    while j < *count {
        let e = &out[j];
        if e.name.len == name.len() && &e.name.b[..name.len()] == name {
            return;
        }
        j += 1;
    }
    let mut e = DirEntry { name: Str80::empty(), is_dir, size };
    let mut k = 0;
    while k < name.len() { e.name.push(name[k]); k += 1; }
    out[*count] = e;
    *count += 1;
}

pub unsafe fn ram_list(path: &str, out: &mut [DirEntry; 32]) -> usize {
    let (b, n) = ram_norm(path);
    let mut count = 0usize;
    let paths = &*core::ptr::addr_of!(RAM_PATH);
    let lens = &*core::ptr::addr_of!(RAM_PLEN);
    let sizes = &*core::ptr::addr_of!(RAM_LEN);
    let mut i = 0;
    while i < RAM_N {
        let plen = lens[i] as usize;
        if sizes[i] > 0 && plen > n && plen <= 80 {
            let p = &paths[i][..plen];
            let mut prefix_ok = true;
            let mut k = 0;
            while k < n {
                if p[k] != b[k] { prefix_ok = false; break; }
                k += 1;
            }
            if prefix_ok && (n == 0 || p[n] == b'\\') {
                let start = if n == 0 { 0 } else { n + 1 };
                let mut end = start;
                while end < plen && p[end] != b'\\' { end += 1; }
                let is_dir = end < plen;
                ram_add_entry(out, &mut count, &p[start..end], is_dir, if is_dir { 0 } else { sizes[i] as u64 });
            }
        }
        i += 1;
    }
    let dirs = &*core::ptr::addr_of!(RAM_DIRS);
    let dl = &*core::ptr::addr_of!(RAM_DLEN);
    let mut j = 0;
    while j < RAM_DN {
        let dlen = dl[j] as usize;
        if dlen > n && dlen <= 80 {
            let p = &dirs[j][..dlen];
            let mut prefix_ok = true;
            let mut k = 0;
            while k < n {
                if p[k] != b[k] { prefix_ok = false; break; }
                k += 1;
            }
            if prefix_ok && (n == 0 || p[n] == b'\\') {
                let start = if n == 0 { 0 } else { n + 1 };
                let mut end = start;
                while end < dlen && p[end] != b'\\' { end += 1; }
                ram_add_entry(out, &mut count, &p[start..end], true, 0);
            }
        }
        j += 1;
    }
    count
}

pub unsafe fn ram_usage() -> (usize, usize) {
    (RAM_N, RAM_USED)
}