#![allow(non_snake_case)]

use core::ffi::c_void;

pub type Status = usize;
pub type Handle = *mut c_void;

pub const EFI_SUCCESS: Status = 0;
pub const EFI_ERR: Status = 0x8000_0000_0000_0000;
pub const EFI_NOT_READY: Status = 0x8000_0000_0000_0006;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Guid {
    pub a: u32,
    pub b: u16,
    pub c: u16,
    pub d: [u8; 8],
}

impl Guid {
    pub const fn new(a: u32, b: u16, c: u16, d: [u8; 8]) -> Self {
        Guid { a, b, c, d }
    }
}

pub const GUID_GOP: Guid = Guid::new(0x9042a9de, 0x23dc, 0x4a38, [0x96, 0xfb, 0x7a, 0xde, 0xd0, 0x80, 0x51, 0x6a]);
pub const GUID_SIMPLE_POINTER: Guid = Guid::new(0x31878c87, 0x0b75, 0x11d5, [0x9a, 0x4b, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d]);
pub const GUID_SIMPLE_FS: Guid = Guid::new(0x964e5b22, 0x6459, 0x11d2, [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]);
pub const GUID_BLOCK_IO: Guid = Guid::new(0x964e5b21, 0x6459, 0x11d2, [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]);
pub const GUID_SIMPLE_NETWORK: Guid = Guid::new(0xa19832b9, 0xac25, 0x11d3, [0x9a, 0x2d, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d]);
pub const GUID_LOADED_IMAGE: Guid = Guid::new(0x5b1b31a1, 0x9562, 0x11d2, [0x8e, 0x3f, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]);
pub const GUID_ACPI20_TABLE: Guid = Guid::new(0x8868e871, 0xe4f1, 0x11d3, [0xbc, 0x22, 0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81]);
pub const GUID_ACPI10_TABLE: Guid = Guid::new(0xeb9d2d30, 0x2d88, 0x11d3, [0x9a, 0x16, 0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d]);
pub const GUID_DEVICE_PATH: Guid = Guid::new(0x09576e91, 0x6d3f, 0x11d2, [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]);
pub const GUID_FILE_INFO: Guid = Guid::new(0x09576e92, 0x6d3f, 0x11d2, [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]);
pub const GUID_HTTP_SERVICE_BINDING: Guid = Guid::new(0xbdc8e6af, 0xd9bc, 0x4379, [0xa7, 0x2a, 0xe0, 0xc4, 0xe7, 0x5d, 0xae, 0x1c]);
pub const GUID_HTTP_PROTOCOL: Guid = Guid::new(0x7a59b29b, 0x910b, 0x4171, [0x82, 0x42, 0xa8, 0x5a, 0x0d, 0xf2, 0x5b, 0x5b]);
pub const GUID_USB_IO: Guid = Guid::new(0x2b2f68d6, 0x0cd2, 0x44cf, [0x8e, 0x8b, 0xbb, 0xa2, 0x0b, 0x1b, 0x5b, 0x75]);
pub const GUID_IP4_CONFIG2: Guid = Guid::new(0x5b446ed1, 0xe30b, 0x4faa, [0x87, 0x1a, 0x36, 0x54, 0xec, 0xa3, 0x60, 0x80]);

#[repr(C)]
pub struct TableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

#[repr(C)]
pub struct ConfigEntry {
    pub guid: Guid,
    pub table: *mut c_void,
}

#[repr(C)]
pub struct TextOutputMode {
    pub max_mode: i32,
    pub mode: i32,
    pub attribute: i32,
    pub cursor_column: i32,
    pub cursor_row: i32,
    pub cursor_visible: u8,
}

#[repr(C)]
pub struct SimpleTextOutput {
    pub reset: usize,
    pub output_string: usize,
    pub test_string: usize,
    pub query_mode: usize,
    pub set_mode: usize,
    pub set_attribute: usize,
    pub clear_screen: usize,
    pub set_cursor_position: usize,
    pub enable_cursor: usize,
    pub mode: *mut TextOutputMode,
}

#[repr(C)]
pub struct InputKey {
    pub scan_code: u16,
    pub unicode_char: u16,
}

#[repr(C)]
pub struct SimpleTextInput {
    pub reset: usize,
    pub read_key_stroke: usize,
    pub wait_for_event: usize,
}

#[repr(C)]
pub struct SystemTable {
    pub hdr: TableHeader,
    pub firmware_vendor: *mut u16,
    pub firmware_revision: u32,
    pub console_in_handle: Handle,
    pub con_in: *mut SimpleTextInput,
    pub console_out_handle: Handle,
    pub con_out: *mut SimpleTextOutput,
    pub std_err_handle: Handle,
    pub std_err: *mut SimpleTextOutput,
    pub runtime_services: *mut c_void,
    pub boot_services: *mut c_void,
    pub num_table_entries: usize,
    pub config_table: *mut ConfigEntry,
}

pub static mut ST: *mut SystemTable = core::ptr::null_mut();
pub static mut BS: *mut c_void = core::ptr::null_mut();

#[inline(always)]
pub fn sys() -> &'static mut SystemTable {
    unsafe { &mut *ST }
}

#[inline(always)]
pub fn bs() -> *mut c_void {
    unsafe { BS }
}

#[inline(always)]
pub unsafe fn bs_slot(idx: usize) -> *mut c_void {
    let base = BS as *mut u8;
    let p = base.add(24 + idx * 8) as *mut *mut c_void;
    *p
}

pub unsafe fn bs_stall(us: usize) {
    let f: extern "efiapi" fn(usize) = core::mem::transmute(bs_slot(28));
    f(us);
}

pub unsafe fn bs_create_event(kind: u32, notify: *mut c_void, ctx: *mut c_void, out: *mut *mut c_void) -> Status {
    let f: extern "efiapi" fn(u32, *mut c_void, *mut c_void, *mut *mut c_void) -> Status =
        core::mem::transmute(bs_slot(7));
    f(kind, notify, ctx, out)
}

pub unsafe fn bs_check_event(event: *mut c_void) -> Status {
    let f: extern "efiapi" fn(*mut c_void) -> Status = core::mem::transmute(bs_slot(12));
    f(event)
}

pub unsafe fn bs_close_event(event: *mut c_void) {
    let f: extern "efiapi" fn(*mut c_void) -> Status = core::mem::transmute(bs_slot(11));
    let _ = f(event);
}

pub unsafe fn bs_connect_all_network() {
    let mut buf: *mut Handle = core::ptr::null_mut();
    let mut n: usize = 0;
    if status_ok(bs_locate_handle_buffer(&GUID_SIMPLE_NETWORK, &mut n, &mut buf)) && !buf.is_null() {
        let mut i = 0;
        while i < n {
            bs_connect_controller(*buf.add(i));
            i += 1;
        }
        bs_free_pool(buf as *mut u8);
    }
    let mut n2: usize = 0;
    let mut buf2: *mut Handle = core::ptr::null_mut();
    if status_ok(bs_locate_handle_buffer(&GUID_DEVICE_PATH, &mut n2, &mut buf2)) && !buf2.is_null() {
        let mut i = 0;
        while i < n2 {
            bs_connect_controller(*buf2.add(i));
            i += 1;
        }
        bs_free_pool(buf2 as *mut u8);
    }
}

pub unsafe fn bs_copy_mem(dest: *mut c_void, src: *const c_void, n: usize) {
    let f: extern "efiapi" fn(*mut c_void, *const c_void, usize) = core::mem::transmute(bs_slot(41));
    f(dest, src, n);
}

pub unsafe fn bs_set_mem(dest: *mut c_void, n: usize, val: u8) {
    let f: extern "efiapi" fn(*mut c_void, usize, u8) = core::mem::transmute(bs_slot(42));
    f(dest, n, val);
}

pub unsafe fn bs_crc32(data: *mut u8, len: usize) -> u32 {
    let f: extern "efiapi" fn(*mut u8, usize, *mut u32) -> Status = core::mem::transmute(bs_slot(40));
    let mut out: u32 = 0;
    let st = f(data, len, &mut out);
    if st == EFI_SUCCESS { out } else { 0 }
}

pub unsafe fn bs_locate_handle_buffer(proto: *const Guid, out_count: *mut usize, out_buf: *mut *mut Handle) -> Status {
    let f: extern "efiapi" fn(u32, *const Guid, *mut c_void, *mut usize, *mut *mut Handle) -> Status =
        core::mem::transmute(bs_slot(36));
    f(2, proto, core::ptr::null_mut(), out_count, out_buf)
}

pub unsafe fn bs_connect_controller(handle: Handle) {
    let f: extern "efiapi" fn(Handle, *mut Handle, *mut c_void, u8) -> Status =
        core::mem::transmute(bs_slot(30));
    f(handle, core::ptr::null_mut(), core::ptr::null_mut(), 1);
}

pub unsafe fn bs_connect_all_storage() {
    let mut buf: *mut Handle = core::ptr::null_mut();
    let mut n: usize = 0;
    if status_ok(bs_locate_handle_buffer(&GUID_BLOCK_IO, &mut n, &mut buf)) && !buf.is_null() {
        let mut i = 0;
        while i < n {
            bs_connect_controller(*buf.add(i));
            i += 1;
        }
        bs_free_pool(buf as *mut u8);
    }
    let mut n2: usize = 0;
    let mut buf2: *mut Handle = core::ptr::null_mut();
    if status_ok(bs_locate_handle_buffer(&GUID_DEVICE_PATH, &mut n2, &mut buf2)) && !buf2.is_null() {
        let mut i = 0;
        while i < n2 {
            bs_connect_controller(*buf2.add(i));
            i += 1;
        }
        bs_free_pool(buf2 as *mut u8);
    }
}

pub unsafe fn bs_handle_protocol(handle: Handle, proto: *const Guid, out: *mut *mut c_void) -> Status {
    let f: extern "efiapi" fn(Handle, *const Guid, *mut *mut c_void) -> Status =
        core::mem::transmute(bs_slot(16));
    f(handle, proto, out)
}

pub unsafe fn bs_allocate_pool(size: usize) -> *mut u8 {
    let f: extern "efiapi" fn(u32, usize, *mut *mut u8) -> Status = core::mem::transmute(bs_slot(5));
    let mut p: *mut u8 = core::ptr::null_mut();
    let st = f(0, size, &mut p);
    if st == EFI_SUCCESS { p } else { core::ptr::null_mut() }
}

pub unsafe fn bs_free_pool(p: *mut u8) {
    let f: extern "efiapi" fn(*mut u8) -> Status = core::mem::transmute(bs_slot(6));
    f(p);
}

pub unsafe fn bs_locate_protocol(proto: *const Guid) -> *mut c_void {
    let f: extern "efiapi" fn(*const Guid, *mut c_void, *mut *mut c_void) -> Status =
        core::mem::transmute(bs_slot(37));
    let mut p: *mut c_void = core::ptr::null_mut();
    let st = f(proto, core::ptr::null_mut(), &mut p);
    if st == EFI_SUCCESS { p } else { core::ptr::null_mut() }
}

pub fn status_ok(st: Status) -> bool {
    st == EFI_SUCCESS
}

#[cfg(target_arch = "x86_64")]
pub fn serial_print(s: &str) {
    unsafe {
        for b in s.bytes() {
            let mut guard = 0;
            loop {
                let lsr: u8;
                core::arch::asm!("in al, dx", in("dx") 0x3FDu16, out("al") lsr);
                if lsr & 0x20 != 0 || guard > 200000 { break; }
                guard += 1;
            }
            core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") b);
        }
        core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") 13u8);
        core::arch::asm!("out dx, al", in("dx") 0x3F8u16, in("al") 10u8);
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn serial_print(_s: &str) {}

pub fn serial_u32(mut v: u32) {
    if v == 0 {
        serial_print("0");
        return;
    }
    let mut b = [0u8; 12];
    let mut n = 0;
    while v > 0 {
        b[n] = b'0' + (v % 10) as u8;
        v /= 10;
        n += 1;
    }
    let mut out = [0u8; 12];
    let mut i = 0;
    while i < n {
        out[i] = b[n - 1 - i];
        i += 1;
    }
    unsafe { serial_print(core::str::from_utf8_unchecked(&out[..n])) };
}

pub fn debug_print(s: &str) {
    serial_print(s);
}

static mut OUTBUF: [u16; 512] = [0; 512];

pub fn con_print(s: &str) {
    unsafe {
        let st = &*ST;
        if st.con_out.is_null() { return; }
        let bytes = s.as_bytes();
        let n = if bytes.len() > 400 { 400 } else { bytes.len() };
        let buf = &mut *core::ptr::addr_of_mut!(OUTBUF);
        let mut i = 0;
        while i < n {
            buf[i] = bytes[i] as u16;
            i += 1;
        }
        buf[n] = 0x0d;
        buf[n + 1] = 0x0a;
        buf[n + 2] = 0;
        let f: extern "efiapi" fn(*mut SimpleTextOutput, *const u16) -> Status =
            core::mem::transmute(st.con_out.read().output_string);
        f(st.con_out, buf.as_ptr());
    }
}

pub fn read_key() -> Option<(u16, u16)> {
    unsafe { crate::ps2::poll(); }
    let mut conin_err = false;
    unsafe {
        let st = &*ST;
        if !st.con_in.is_null() {
            if (*st.con_in).wait_for_event != 0 {
                let _ = bs_check_event((*st.con_in).wait_for_event as *mut core::ffi::c_void);
            }
            let f: extern "efiapi" fn(*mut SimpleTextInput, *mut InputKey) -> Status =
                core::mem::transmute((*st.con_in).read_key_stroke);
            let mut k = InputKey { scan_code: 0, unicode_char: 0 };
            let s = f(st.con_in, &mut k);
            if s == EFI_SUCCESS {
                serial_print("[key] conin");
                serial_u32(k.scan_code as u32);
                serial_u32(k.unicode_char as u32);
                return Some((k.scan_code, k.unicode_char));
            }
            if s != EFI_NOT_READY {
                conin_err = true;
            }
        }
    }
    unsafe {
        if let Some(k) = crate::ps2::take_key() {
            return Some(k);
        }
        if let Some(k) = crate::usbhid::take_key() {
            return Some(k);
        }
        if let Some(k) = crate::input::read_key_ex() {
            serial_print("[key] ex");
            serial_u32(k.0 as u32);
            serial_u32(k.1 as u32);
            return Some(k);
        }
        if conin_err {
            crate::ps2::keyboard_takeover();
            serial_print("[key] ps2 takeover");
        }
    }
    None
}
pub unsafe fn to_utf16(src: &str, dst: &mut [u16]) -> usize {
    let b = src.as_bytes();
    let mut i = 0;
    while i < b.len() && i + 1 < dst.len() {
        dst[i] = b[i] as u16;
        i += 1;
    }
    dst[i] = 0;
    i
}

pub unsafe fn utf16_to_ascii(src: *const u16, dst: &mut [u8]) -> usize {
    let mut i = 0;
    while i + 1 < dst.len() {
        let c = *src.add(i);
        if c == 0 { break; }
        dst[i] = if c < 128 { c as u8 } else { b'?' };
        i += 1;
    }
    dst[i] = 0;
    i
}