use crate::efi::*;

#[derive(Clone, Copy)]
pub struct CpuInfo {
    pub total: usize,
    pub enabled: usize,
    pub bsp_apic: u8,
    pub ids: [u8; 64],
    pub acpi_ok: bool,
}

static mut CPU: CpuInfo = CpuInfo {
    total: 1,
    enabled: 1,
    bsp_apic: 0,
    ids: [0; 64],
    acpi_ok: false,
};

pub static mut NET: NetInfo = NetInfo { present: false, mac: [0; 6], link: false };
pub static mut FIRMWARE: [u8; 64] = [0; 64];
pub static mut FIRMWARE_LEN: usize = 0;

#[derive(Clone, Copy)]
pub struct NetInfo {
    pub present: bool,
    pub mac: [u8; 6],
    pub link: bool,
}

#[repr(C)]
pub struct MacAddr {
    pub addr: [u8; 32],
}

#[repr(C)]
pub struct SnpMode {
    pub state: u32,
    pub hw_address_size: u32,
    pub media_header_size: u32,
    pub max_packet_size: u32,
    pub nvram_size: u32,
    pub nvram_access_size: u32,
    pub receive_filter_mask: u32,
    pub receive_filter_setting: u32,
    pub max_mcast_filter_count: u32,
    pub mcast_filter_count: u32,
    pub mcast_filter: [MacAddr; 16],
    pub current_address: MacAddr,
    pub broadcast_address: MacAddr,
    pub permanent_address: MacAddr,
    pub if_type: u8,
}

#[repr(C)]
pub struct Snp {
    pub revision: u64,
    pub start: usize,
    pub stop: usize,
    pub initialize: usize,
    pub reset: usize,
    pub shutdown: usize,
    pub receive_filters: usize,
    pub set_receive_filter: usize,
    pub statistics: usize,
    pub mcast_ip_to_mac: usize,
    pub nvdata: usize,
    pub get_status: usize,
    pub transmit: usize,
    pub receive: usize,
    pub wait_for_packet: usize,
    pub mode: *mut SnpMode,
}

#[repr(C)]
#[derive(Clone, Copy)]
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
pub struct RuntimeServices {
    pub hdr: TableHeader,
    pub get_time: usize,
    pub set_time: usize,
    pub get_wakeup_time: usize,
    pub set_wakeup_time: usize,
    pub set_virtual_address_map: usize,
    pub convert_pointer: usize,
    pub get_variable: usize,
    pub get_next_variable_name: usize,
    pub set_variable: usize,
    pub get_next_high_monotonic_count: usize,
    pub reset_system: usize,
}

#[repr(C)]
pub struct PointerState {
    pub relative_x: i32,
    pub relative_y: i32,
    pub relative_z: i32,
    pub left: u8,
    pub right: u8,
}

static mut POINTER: *mut core::ffi::c_void = core::ptr::null_mut();
static mut PT_INITED: bool = false;

pub unsafe fn pointer_init() -> bool {
    if PT_INITED { return !POINTER.is_null(); }
    PT_INITED = true;
    POINTER = bs_locate_protocol(&GUID_SIMPLE_POINTER);
    !POINTER.is_null()
}

pub unsafe fn pointer_present() -> bool {
    !POINTER.is_null()
}

const POINTER_GAIN_NUM: i32 = 12;
const POINTER_GAIN_DEN: i32 = 5;

/// 相对指针增益：小幅移动保留精细控制，常规移动约 2.4 倍，修复指针移速过慢。
fn pointer_gain(v: i32) -> i32 {
    let a = if v < 0 { -v } else { v };
    let num = if a <= 3 { POINTER_GAIN_NUM - 2 } else { POINTER_GAIN_NUM };
    v * num / POINTER_GAIN_DEN
}

pub struct InputState {
    pub x: i32,
    pub y: i32,
    pub left: bool,
    pub right: bool,
    pub prev_left: bool,
    pub prev_right: bool,
    pub clicked: bool,
    pub right_clicked: bool,
    pub down: bool,
    pub moved: bool,
    pub scroll: i32,
}

impl InputState {
    pub fn new(cx: i32, cy: i32) -> InputState {
        InputState {
            x: cx,
            y: cy,
            left: false,
            right: false,
            prev_left: false,
            prev_right: false,
            clicked: false,
            right_clicked: false,
            down: false,
            moved: false,
            scroll: 0,
        }
    }

    pub unsafe fn poll(&mut self, w: i32, h: i32) {
        self.clicked = false;
        self.right_clicked = false;
        self.moved = false;
        self.scroll = 0;
        crate::usbhid::poll(w, h);
        crate::ps2::poll();
        if let Some((ax, ay, btn, moved)) = crate::input::abs_mouse(w, h) {
            if moved {
                self.x = ax;
                self.y = ay;
                self.moved = true;
            }
            self.left = btn & 1 != 0;
            self.right = btn & 2 != 0;
            self.finish_buttons();
            return;
        }
        if let Some((ax, ay, btn, moved)) = crate::usbhid::abs_state() {
            if moved {
                self.x = ax;
                self.y = ay;
                self.moved = true;
            }
            self.left = btn & 1 != 0;
            self.right = btn & 2 != 0;
            self.finish_buttons();
            return;
        }
        if !POINTER.is_null() {
            let fp = POINTER as *mut *mut core::ffi::c_void;
            let f2 = *fp.add(1);
            let call: extern "efiapi" fn(*mut core::ffi::c_void, *mut *mut PointerState) -> Status =
                core::mem::transmute(f2);
            let mut s: *mut PointerState = core::ptr::null_mut();
            if call(POINTER, &mut s) == EFI_SUCCESS && !s.is_null() {
                let dz = (*s).relative_z;
                let dx = (*s).relative_x;
                let dy = (*s).relative_y;
                if dx != 0 || dy != 0 {
                    self.x += pointer_gain(dx);
                    self.y += pointer_gain(dy);
                    if self.x < 0 { self.x = 0; }
                    if self.y < 0 { self.y = 0; }
                    if self.x >= w - 1 { self.x = w - 1; }
                    if self.y >= h - 1 { self.y = h - 1; }
                    self.moved = true;
                }
                if dz != 0 { self.scroll += dz; }
                self.left = (*s).left != 0;
                self.right = (*s).right != 0;
                if dx != 0 || dy != 0 || self.left {
                    serial_print("[ptr] dx/dy/btn");
                    serial_u32(dx as u32);
                    serial_u32(dy as u32);
                    serial_u32(self.left as u32);
                }
            }
            self.finish_buttons();
            return;
        }
        if let Some((dx, dy, btn, moved)) = crate::usbhid::rel_state() {
            if moved {
                self.x += pointer_gain(dx);
                self.y += pointer_gain(dy);
                if self.x < 0 { self.x = 0; }
                if self.y < 0 { self.y = 0; }
                if self.x >= w - 1 { self.x = w - 1; }
                if self.y >= h - 1 { self.y = h - 1; }
                self.moved = true;
            }
            self.left = btn & 1 != 0;
            self.right = btn & 2 != 0;
            self.finish_buttons();
            return;
        }
        if crate::ps2::mouse_ok() {
            let (dx, dy, btn, moved) = crate::ps2::take_mouse();
            let mut ax = pointer_gain(dx);
            let mut ay = pointer_gain(dy);
            if ax > 160 { ax = 160; }
            if ax < -160 { ax = -160; }
            if ay > 160 { ay = 160; }
            if ay < -160 { ay = -160; }
            if moved {
                self.x += ax;
                self.y += ay;
                if self.x < 0 { self.x = 0; }
                if self.y < 0 { self.y = 0; }
                if self.x >= w - 1 { self.x = w - 1; }
                if self.y >= h - 1 { self.y = h - 1; }
                self.moved = true;
            }
            self.left = btn & 1 != 0;
            self.right = btn & 2 != 0;
            self.finish_buttons();
        }
    }

    fn finish_buttons(&mut self) {
        if self.left && !self.prev_left { self.clicked = true; }
        if self.right && !self.prev_right { self.right_clicked = true; }
        self.prev_left = self.left;
        self.prev_right = self.right;
        self.down = self.left;
    }
    pub unsafe fn warp(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }
}

unsafe fn read_u32(p: *const u8) -> u32 {
    core::ptr::read_unaligned(p as *const u32)
}

unsafe fn read_u64(p: *const u8) -> u64 {
    core::ptr::read_unaligned(p as *const u64)
}

unsafe fn sig_eq(p: *const u8, s: &[u8; 4]) -> bool {
    *p == s[0] && *p.add(1) == s[1] && *p.add(2) == s[2] && *p.add(3) == s[3]
}

pub unsafe fn print_sig(p: *const u8) {
    let mut b = [b'.'; 4];
    let mut i = 0;
    while i < 4 {
        let c = *p.add(i);
        if c >= 32 && c < 127 { b[i] = c; }
        i += 1;
    }
    serial_print(core::str::from_utf8_unchecked(&b));
}

unsafe fn parse_root(root: *const u8, is_x: bool) -> bool {
    let len = read_u32(root.add(4)) as usize;
    if len < 36 { return false; }
    let esz = if is_x { 8 } else { 4 };
    let n = (len - 36) / esz;
    let mut k = 0;
    while k < n {
        let entry = root.add(36 + k * esz);
        let tbl = if is_x { read_u64(entry) as *const u8 } else { read_u32(entry) as *const u8 };
        if !tbl.is_null() && sig_eq(tbl, b"APIC") {
            CPU.acpi_ok = true;
            parse_madt(tbl);
            return true;
        }
        k += 1;
    }
    false
}

#[cfg(target_arch = "x86_64")]
pub unsafe fn cpu_topology_cpuid() -> usize {
    let max = core::arch::x86_64::__cpuid(0).eax;
    if max >= 1 {
        let r = core::arch::x86_64::__cpuid(1);
        let n = ((r.ebx >> 16) & 0xff) as usize;
        if n > 0 { return n; }
    }
    1
}

#[cfg(not(target_arch = "x86_64"))]
pub unsafe fn cpu_topology_cpuid() -> usize { 1 }

unsafe fn cpuid_fallback() {
    let n = cpu_topology_cpuid();
    if n > 1 {
        CPU.total = n;
        CPU.enabled = n;
        serial_print("[acpi] cpuid fallback cores");
        serial_u32(n as u32);
    }
}

unsafe fn find_rsdp() -> *const u8 {
    let mut a = 0xE0000usize;
    while a < 0x100000 {
        let p = a as *const u8;
        if *p == b'R' && *p.add(1) == b'S' && *p.add(2) == b'D' && *p.add(3) == b' '
            && *p.add(4) == b'P' && *p.add(5) == b'T' && *p.add(6) == b'R' && *p.add(7) == b' '
        {
            return p;
        }
        a += 16;
    }
    core::ptr::null()
}

pub unsafe fn acpi_probe() {
    let st = &*ST;
    serial_print("[acpi] probe");
    let mut p20: *const u8 = core::ptr::null();
    let mut p10: *const u8 = core::ptr::null();
    if !st.config_table.is_null() {
        let mut i = 0;
        while i < st.num_table_entries {
            let e = st.config_table.add(i);
            let g = (*e).guid;
            if g.a == GUID_ACPI20_TABLE.a && g.b == GUID_ACPI20_TABLE.b && g.c == GUID_ACPI20_TABLE.c {
                p20 = (*e).table as *const u8;
            }
            if g.a == GUID_ACPI10_TABLE.a && g.b == GUID_ACPI10_TABLE.b && g.c == GUID_ACPI10_TABLE.c {
                p10 = (*e).table as *const u8;
            }
            i += 1;
        }
    }
    if !p20.is_null() && sig_eq(p20, b"XSDT") {
        serial_print("[acpi] XSDT via config table");
        if parse_root(p20, true) { return; }
    }
    if !p20.is_null() && sig_eq(p20, b"RSDT") {
        serial_print("[acpi] RSDT via config table");
        if parse_root(p20, false) { return; }
    }
    if !p10.is_null() && sig_eq(p10, b"RSDT") {
        serial_print("[acpi] RSDT 1.0 via config table");
        if parse_root(p10, false) { return; }
    }
    let rsdp = find_rsdp();
    if rsdp.is_null() {
        serial_print("[acpi] RSDP not found");
        cpuid_fallback();
        return;
    }
    serial_print("[acpi] RSDP found");
    let rev = *rsdp.add(15);
    if rev >= 2 {
        let x = read_u64(rsdp.add(24)) as *const u8;
        if !x.is_null() && sig_eq(x, b"XSDT") {
            if parse_root(x, true) { return; }
        }
    }
    let r = read_u32(rsdp.add(16)) as *const u8;
    if !r.is_null() && sig_eq(r, b"RSDT") {
        if parse_root(r, false) { return; }
    }
    serial_print("[acpi] no root via RSDP");
    cpuid_fallback();
}

unsafe fn parse_madt(p: *const u8) {
    let len = read_u32(p.add(4)) as usize;
    if len < 44 { return; }
    serial_print("[acpi] MADT found");
    let mut off = 44usize;
    let mut count = 0usize;
    let mut enabled = 0usize;
    while off + 2 <= len {
        let typ = *p.add(off);
        let elen = *p.add(off + 1) as usize;
        if elen < 2 { break; }
        if typ == 0 && elen >= 8 {
            let apic = *p.add(off + 3);
            let flags = read_u32(p.add(off + 4));
            if count < 64 { CPU.ids[count] = apic; }
            count += 1;
            if flags & 1 != 0 { enabled += 1; }
        } else if typ == 9 && elen >= 16 {
            let apic = read_u32(p.add(off + 4));
            let flags = read_u32(p.add(off + 8));
            if count < 64 { CPU.ids[count] = apic as u8; }
            count += 1;
            if flags & 1 != 0 { enabled += 1; }
        }
        off += elen;
    }
    if count > 0 {
        CPU.total = count;
        CPU.enabled = if enabled > 0 { enabled } else { count };
        CPU.bsp_apic = CPU.ids[0];
        serial_print("[acpi] cpus detected");
    }
}

pub fn cpu() -> &'static CpuInfo {
    unsafe { &*core::ptr::addr_of!(CPU) }
}

pub fn net() -> &'static NetInfo {
    unsafe { &*core::ptr::addr_of!(NET) }
}

pub unsafe fn net_probe() {
    let snp = bs_locate_protocol(&GUID_SIMPLE_NETWORK) as *mut Snp;
    if snp.is_null() {
        NET.present = false;
        return;
    }
    NET.present = true;
    let m = (*snp).mode;
    if m.is_null() { return; }
    let n = if (*m).hw_address_size > 6 { 6 } else { (*m).hw_address_size as usize };
    let mut i = 0;
    while i < n {
        NET.mac[i] = (*m).current_address.addr[i];
        i += 1;
    }
    NET.link = (*m).state != 0;
}

pub unsafe fn firmware_vendor() {
    let st = &*ST;
    if st.firmware_vendor.is_null() { return; }
    let mut i = 0;
    while i < 63 {
        let c = *st.firmware_vendor.add(i);
        if c == 0 { break; }
        FIRMWARE[i] = if c < 128 { c as u8 } else { b'?' };
        i += 1;
    }
    FIRMWARE_LEN = i;
}

pub fn firmware_str() -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&FIRMWARE[..FIRMWARE_LEN]) }
}

pub unsafe fn get_time() -> Option<EfiTime> {
    if let Some(t) = cmos_time() { return Some(t); }
    let st = &*ST;
    if st.runtime_services.is_null() { return None; }
    let rt = st.runtime_services as *mut RuntimeServices;
    let f: extern "efiapi" fn(*mut RuntimeServices, *mut EfiTime, *mut core::ffi::c_void) -> Status =
        core::mem::transmute((*rt).get_time);
    let mut t = EfiTime {
        year: 0,
        month: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        pad1: 0,
        nanosecond: 0,
        timezone: 0,
        daylight: 0,
        pad2: 0,
    };
    let stt = f(rt, &mut t, core::ptr::null_mut());
    serial_print("[rtc] status");
    serial_u32(stt as u32);
    serial_u32(t.year as u32);
    serial_u32(t.month as u32);
    serial_u32(t.day as u32);
    serial_u32(t.hour as u32);
    serial_u32(t.minute as u32);
    if stt == EFI_SUCCESS { Some(t) } else { None }
}

static mut BOOT_TIME: EfiTime = EfiTime {
    year: 0,
    month: 0,
    day: 0,
    hour: 0,
    minute: 0,
    second: 0,
    pad1: 0,
    nanosecond: 0,
    timezone: 0,
    daylight: 0,
    pad2: 0,
};
static mut HAVE_TIME: bool = false;

pub unsafe fn cache_time() {
    if HAVE_TIME { return; }
    if let Some(t) = get_time() {
        BOOT_TIME = t;
        HAVE_TIME = true;
    }
}

pub fn cached_time() -> Option<EfiTime> {
    unsafe {
        if HAVE_TIME {
            Some(core::ptr::read(core::ptr::addr_of!(BOOT_TIME)))
        } else {
            None
        }
    }
}

pub fn num2(buf: &mut [u8], mut v: u32, min_digits: usize) -> usize {
    let mut tmp = [0u8; 12];
    let mut n = 0;
    if v == 0 {
        tmp[0] = b'0';
        n = 1;
    }
    while v > 0 {
        tmp[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    while n < min_digits {
        tmp[n] = b'0';
        n += 1;
    }
    let mut i = 0;
    while i < n {
        buf[i] = tmp[n - 1 - i];
        i += 1;
    }
    n
}
#[repr(C)]
struct MemDesc {
    kind: u32,
    pad: u32,
    phys: u64,
    virt: u64,
    pages: u64,
    attr: u64,
}

pub unsafe fn mem_kb() -> (u64, u64) {
    let f: extern "efiapi" fn(*mut usize, *mut core::ffi::c_void, *mut usize, *mut usize, *mut u32) -> Status =
        core::mem::transmute(bs_slot(4));
    let mut size = 0usize;
    let mut key = 0usize;
    let mut dsz = 0usize;
    let mut dver = 0u32;
    let _ = f(&mut size, core::ptr::null_mut(), &mut key, &mut dsz, &mut dver);
    let mut buf = [0u8; 32768];
    size = buf.len();
    if !status_ok(f(&mut size, buf.as_mut_ptr() as *mut core::ffi::c_void, &mut key, &mut dsz, &mut dver)) || dsz == 0 {
        return (0, 0);
    }
    let mut total = 0u64;
    let mut free = 0u64;
    let mut off = 0usize;
    while off + dsz <= size {
        let d = &*(buf.as_ptr().add(off) as *const MemDesc);
        let sz = d.pages * 4096;
        if (d.kind >= 1 && d.kind <= 7) || d.kind == 9 || d.kind == 10 {
            total += sz;
        }
        if d.kind == 7 { free += sz; }
        off += dsz;
    }
    (total / 1024, free / 1024)
}

pub fn uptime_secs() -> u32 {
    unsafe {
        let b = match cached_time() { Some(t) => t, None => return 0 };
        let n = match get_time() { Some(t) => t, None => return 0 };
        let bs = (b.hour as u32) * 3600 + (b.minute as u32) * 60 + b.second as u32;
        let ns = (n.hour as u32) * 3600 + (n.minute as u32) * 60 + n.second as u32;
        if ns >= bs { ns - bs } else { ns + 86400 - bs }
    }
}

pub fn cores() -> usize {
    unsafe { (*core::ptr::addr_of!(CPU)).enabled }
}
unsafe fn cmos_r(idx: u8) -> u8 {
    crate::port::outb(0x70, idx);
    crate::port::io_delay();
    crate::port::inb(0x71)
}

unsafe fn cmos_time() -> Option<EfiTime> {
    let mut guard = 0u32;
    while guard < 200_000 {
        if cmos_r(0x0A) & 0x80 == 0 { break; }
        guard += 1;
    }
    let mut sec = cmos_r(0x00);
    let mut min = cmos_r(0x02);
    let mut hour = cmos_r(0x04);
    let mut day = cmos_r(0x07);
    let mut mon = cmos_r(0x08);
    let mut year = cmos_r(0x09);
    let cen = cmos_r(0x32);
    let b = cmos_r(0x0B);
    if b & 0x04 == 0 {
        sec = (sec >> 4) * 10 + (sec & 0x0f);
        min = (min >> 4) * 10 + (min & 0x0f);
        let pm = hour & 0x80;
        hour = (hour & 0x7f);
        hour = (hour >> 4) * 10 + (hour & 0x0f);
        if b & 0x02 == 0 && pm != 0 { hour = (hour + 12) % 24; }
        year = (year >> 4) * 10 + (year & 0x0f);
        day = (day >> 4) * 10 + (day & 0x0f);
        mon = (mon >> 4) * 10 + (mon & 0x0f);
    }
    let mut y = year as u16;
    let c = if b & 0x04 == 0 { (cen >> 4) * 10 + (cen & 0x0f) } else { cen };
    if c >= 19 { y = c as u16 * 100 + year as u16; } else { y = 2000 + year as u16; }
    if mon < 1 || mon > 12 || day < 1 || day > 31 || hour > 23 || min > 59 || sec > 59 || y < 2000 {
        return None;
    }
    Some(EfiTime {
        year: y, month: mon, day, hour, minute: min, second: sec,
        pad1: 0, nanosecond: 0, timezone: 0, daylight: 0, pad2: 0,
    })
}
