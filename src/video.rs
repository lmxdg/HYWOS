#![allow(dead_code)]

use crate::efi::*;
use crate::gfx::Gfx;
use crate::port::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Gop,
    Vbe,
    Svga,
    Virtio,
}

static mut KIND: Kind = Kind::Gop;
static mut NAME: [u8; 40] = [0; 40];
static mut NLEN: usize = 0;

pub fn kind() -> Kind {
    unsafe { KIND }
}

pub fn name() -> &'static str {
    unsafe { core::str::from_utf8_unchecked(&NAME[..NLEN]) }
}

unsafe fn set_name(s: &str, k: Kind) {
    KIND = k;
    NLEN = 0;
    for b in s.bytes() {
        if NLEN < 39 {
            NAME[NLEN] = b;
            NLEN += 1;
        }
    }
    serial_print("[video] ");
    serial_print(s);
}

pub unsafe fn probe(gop: Gfx) -> Gfx {
    if let Some(g) = virtio_init() {
        set_name("virtio-gpu 2D (legacy)", Kind::Virtio);
        return g;
    }
    if let Some(g) = svga_init() {
        set_name("VMware SVGA II", Kind::Svga);
        return g;
    }
    if let Some(g) = vbe_init() {
        set_name("VBE 2.0 standard VGA", Kind::Vbe);
        return g;
    }
    set_name("UEFI GOP", Kind::Gop);
    gop
}

pub unsafe fn flush() {
    match KIND {
        Kind::Virtio => virtio_flush(),
        Kind::Svga => svga_flush(),
        _ => {}
    }
}

static mut VBE_W: usize = 0;
static mut VBE_H: usize = 0;

unsafe fn vbe_put(idx: u16, v: u16) {
    outw(0x1CE, idx);
    outw(0x1CF, v);
}

unsafe fn vbe_get(idx: u16) -> u16 {
    outw(0x1CE, idx);
    inw(0x1CF)
}

unsafe fn vbe_set(w: u16, h: u16) -> bool {
    vbe_put(4, 0);
    vbe_put(1, w);
    vbe_put(2, h);
    vbe_put(3, 32);
    vbe_put(4, 0x41);
    vbe_get(1) == w && vbe_get(2) == h && vbe_get(3) == 32
}

unsafe fn vbe_init() -> Option<Gfx> {
    let d = find(0x1234, 0x1111)?;
    let (is_io, base) = bar(&d, 0);
    if is_io || base == 0 {
        return None;
    }
    enable_bus_master(&d);
    let mut ok = vbe_set(1024, 768);
    if !ok {
        ok = vbe_set(800, 600);
    }
    if !ok {
        vbe_put(4, 0);
        return None;
    }
    let w = vbe_get(1) as usize;
    let h = vbe_get(2) as usize;
    VBE_W = w;
    VBE_H = h;
    Some(Gfx::from_device(base as *mut u32, w, h, w, false, true))
}

static mut SVGA_IO: u16 = 0;

unsafe fn sreg(idx: u32) -> u32 {
    outl(SVGA_IO, idx);
    inl(SVGA_IO + 4)
}

unsafe fn sset(idx: u32, val: u32) {
    outl(SVGA_IO, idx);
    outl(SVGA_IO + 4, val);
}

unsafe fn svga_init() -> Option<Gfx> {
    let d = find(0x15AD, 0x0405)?;
    let (is_io, base) = bar(&d, 0);
    if !is_io || base == 0 {
        return None;
    }
    SVGA_IO = base as u16;
    if sreg(0) != 0x9000_0002 {
        serial_print("[svga] SVGA II not available in this firmware");
        return None;
    }
    let (fb_io, fb) = bar(&d, 1);
    if fb_io || fb == 0 {
        return None;
    }
    let maxw = sreg(4);
    let maxh = sreg(5);
    if maxw < 640 || maxh < 480 {
        return None;
    }
    let (tw, th) = if maxw >= 1024 && maxh >= 768 {
        (1024u32, 768u32)
    } else if maxw >= 800 && maxh >= 600 {
        (800, 600)
    } else {
        (640, 480)
    };
    sset(1, 0);
    sset(2, tw);
    sset(3, th);
    sset(7, 32);
    sset(1, 1);
    let w = sreg(2) as usize;
    let h = sreg(3) as usize;
    let bpp = sreg(7) as usize;
    let bpl = sreg(12) as usize;
    if w < 640 || h < 480 || bpp != 32 {
        sset(1, 0);
        return None;
    }
    let stride = if bpl >= w * 4 { bpl / 4 } else { w };
    serial_print("[svga] mode ok");
    Some(Gfx::from_device(fb as *mut u32, w, h, stride, false, true))
}

unsafe fn svga_flush() {
    if SVGA_IO == 0 {
        return;
    }
    sset(0x15, 1);
    let mut guard = 0u32;
    while sreg(0x16) != 0 && guard < 2_000_000 {
        guard += 1;
    }
}

#[repr(C, align(4096))]
struct Align4K([u8; 16384]);

#[repr(C)]
struct Desc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

static mut VQ: Align4K = Align4K([0; 16384]);
static mut VFB: [u8; 1024 * 768 * 4] = [0; 1024 * 768 * 4];
static mut VCMD: [u8; 1024] = [0; 1024];
static mut VRESP: [u8; 1024] = [0; 1024];
static mut VIO: u16 = 0;
static mut VQSZ: u16 = 0;
static mut VAIDX: u16 = 0;
static mut VLAST: u16 = 0;
static mut VFENCE: u64 = 0;
static mut VW: usize = 0;
static mut VH: usize = 0;

unsafe fn vdesc() -> *mut u8 {
    core::ptr::addr_of_mut!(VQ) as *mut u8
}

unsafe fn vavail() -> *mut u8 {
    vdesc().add(1024)
}

unsafe fn vused() -> *mut u8 {
    vdesc().add(4096)
}

unsafe fn put32(p: *mut u8, off: usize, v: u32) {
    core::ptr::write_unaligned(p.add(off) as *mut u32, v);
}

unsafe fn put64(p: *mut u8, off: usize, v: u64) {
    core::ptr::write_unaligned(p.add(off) as *mut u64, v);
}

unsafe fn get32(p: *const u8, off: usize) -> u32 {
    core::ptr::read_unaligned(p.add(off) as *const u32)
}

unsafe fn vcmd(typ: u32, payload: &[u8], resp_len: usize) -> bool {
    let cmd = core::ptr::addr_of_mut!(VCMD) as *mut u8;
    VFENCE += 1;
    put32(cmd, 0, typ);
    put32(cmd, 4, 0);
    put64(cmd, 8, VFENCE);
    put32(cmd, 16, 0);
    put32(cmd, 20, 0);
    let mut i = 0;
    while i < payload.len() {
        *cmd.add(24 + i) = payload[i];
        i += 1;
    }
    let clen = 24 + payload.len();
    let resp = core::ptr::addr_of_mut!(VRESP) as *mut u8;
    let mut k = 0;
    while k < resp_len {
        *resp.add(k) = 0;
        k += 1;
    }
    let d = vdesc() as *mut Desc;
    (*d.add(0)).addr = cmd as u64;
    (*d.add(0)).len = clen as u32;
    (*d.add(0)).flags = 1;
    (*d.add(0)).next = 1;
    (*d.add(1)).addr = resp as u64;
    (*d.add(1)).len = resp_len as u32;
    (*d.add(1)).flags = 2;
    (*d.add(1)).next = 0;
    let av = vavail();
    let idx = VAIDX;
    let slot = (idx % VQSZ) as usize;
    core::ptr::write_unaligned(av.add(4 + slot * 2) as *mut u16, 0);
    core::ptr::write_unaligned(av.add(2) as *mut u16, idx.wrapping_add(1));
    VAIDX = idx.wrapping_add(1);
    vnotify();
    let used = vused();
    let mut spins = 0u32;
    loop {
        let u = core::ptr::read_unaligned(used.add(2) as *const u16);
        if u != VLAST {
            VLAST = u;
            break;
        }
        spins += 1;
        if spins > 8_000_000 {
            serial_print("[virtio] cmd timeout");
            return false;
        }
    }
    let _ = (typ, resp);
    true
}

unsafe fn virtio_legacy() -> Option<Gfx> {
    serial_print("[virtio] legacy probe");
    let mut dev = find(0x1AF4, 0x1110);
    if dev.is_none() {
        dev = find(0x1AF4, 0x1050);
    }
    let d = dev?;
    let (is_io, base) = bar(&d, 0);
    if !is_io || base == 0 {
        return None;
    }
    VIO = base as u16;
    enable_bus_master(&d);
    outb(VIO + 0x12, 0);
    outb(VIO + 0x12, 1);
    outb(VIO + 0x12, 3);
    outl(VIO + 0x04, 0);
    outw(VIO + 0x0E, 0);
    let qsz = inw(VIO + 0x0C);
    if qsz == 0 {
        return None;
    }
    VQSZ = qsz;
    serial_print("[virtio] queue ready");
    let pfn = (core::ptr::addr_of!(VQ) as u32) >> 12;
    outl(VIO + 0x08, pfn);
    outb(VIO + 0x12, 7);
    let mut probe_payload = [0u8; 4];
    probe_payload[0] = 0;
    if !vcmd(0x0100, &[], 408) {
        serial_print("[video] virtio cmd timeout");
        return None;
    }
    let resp = core::ptr::addr_of!(VRESP) as *const u8;
    let mut dw = get32(resp, 24 + 8) as usize;
    let mut dh = get32(resp, 24 + 12) as usize;
    if dw < 640 || dh < 480 || dw > 1024 || dh > 768 {
        dw = 1024;
        dh = 768;
    }
    VW = dw;
    VH = dh;
    let mut p = [0u8; 32];
    put32(p.as_mut_ptr(), 0, 1);
    put32(p.as_mut_ptr(), 4, 1);
    put32(p.as_mut_ptr(), 8, dw as u32);
    put32(p.as_mut_ptr(), 12, dh as u32);
    if !vcmd(0x0101, &p[..16], 24) {
        return None;
    }
    let fb = core::ptr::addr_of!(VFB) as u64;
    let mut a = [0u8; 32];
    put32(a.as_mut_ptr(), 0, 1);
    put32(a.as_mut_ptr(), 4, 1);
    put64(a.as_mut_ptr(), 8, fb);
    put32(a.as_mut_ptr(), 16, (dw * dh * 4) as u32);
    put32(a.as_mut_ptr(), 20, 0);
    if !vcmd(0x0106, &a[..24], 24) {
        return None;
    }
    if !vcmd(0x0103, &scanout_payload(0, 1, dw, dh), 24) {
        return None;
    }
    if !vcmd(0x0105, &transfer_payload(1, dw, dh), 24) {
        return None;
    }
    if !vcmd(0x0104, &flush_payload(1, dw, dh), 24) {
        return None;
    }
    Some(Gfx::from_device(
        core::ptr::addr_of_mut!(VFB) as *mut u32,
        dw,
        dh,
        dw,
        false,
        true,
    ))
}

fn scanout_payload(x: usize, rid: u32, w: usize, h: usize) -> [u8; 24] {
    let mut p = [0u8; 24];
    unsafe {
        put32(p.as_mut_ptr(), 0, 0);
        put32(p.as_mut_ptr(), 4, 0);
        put32(p.as_mut_ptr(), 8, w as u32);
        put32(p.as_mut_ptr(), 12, h as u32);
        put32(p.as_mut_ptr(), 16, x as u32);
        put32(p.as_mut_ptr(), 20, rid);
    }
    p
}

fn transfer_payload(rid: u32, w: usize, h: usize) -> [u8; 32] {
    let mut p = [0u8; 32];
    unsafe {
        put32(p.as_mut_ptr(), 0, 0);
        put32(p.as_mut_ptr(), 4, 0);
        put32(p.as_mut_ptr(), 8, w as u32);
        put32(p.as_mut_ptr(), 12, h as u32);
        put64(p.as_mut_ptr(), 16, 0);
        put32(p.as_mut_ptr(), 24, rid);
        put32(p.as_mut_ptr(), 28, 0);
    }
    p
}

fn flush_payload(rid: u32, w: usize, h: usize) -> [u8; 24] {
    let mut p = [0u8; 24];
    unsafe {
        put32(p.as_mut_ptr(), 0, 0);
        put32(p.as_mut_ptr(), 4, 0);
        put32(p.as_mut_ptr(), 8, w as u32);
        put32(p.as_mut_ptr(), 12, h as u32);
        put32(p.as_mut_ptr(), 16, rid);
        put32(p.as_mut_ptr(), 20, 0);
    }
    p
}

unsafe fn virtio_flush() {
    if VIO == 0 || VW == 0 {
        return;
    }
    let _ = vcmd(0x0105, &transfer_payload(1, VW, VH), 24);
    let _ = vcmd(0x0104, &flush_payload(1, VW, VH), 24);
}

static mut VNOTIFY: u64 = 0;
static mut VMODERN: bool = false;
static mut VCOMMON: u64 = 0;

unsafe fn vnotify() {
    if VMODERN {
        core::ptr::write_volatile(VNOTIFY as *mut u16, 0u16);
        core::ptr::write_volatile(VNOTIFY as *mut u32, 0u32);
    } else {
        outw(VIO + 0x10, 0);
    }
}

unsafe fn cw32(addr: u64, off: u32, v: u32) {
    core::ptr::write_volatile((addr + off as u64) as *mut u32, v);
}

unsafe fn cr32(addr: u64, off: u32) -> u32 {
    core::ptr::read_volatile((addr + off as u64) as *const u32)
}

unsafe fn cw64(addr: u64, off: u32, v: u64) {
    core::ptr::write_volatile((addr + off as u64) as *mut u64, v);
}

unsafe fn cstat(addr: u64, v: u8) {
    core::ptr::write_volatile((addr + 0x14) as *mut u8, v);
}

unsafe fn cfg8(bus: u8, dev: u8, func: u8, off: u8) -> u8 {
    let v = cfg32(bus, dev, func, off & 0xfc);
    ((v >> (((off & 3) * 8) as u32)) & 0xff) as u8
}

unsafe fn cfg32u(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    let mut v = 0u32;
    let mut i = 0u8;
    while i < 4 {
        v |= (cfg8(bus, dev, func, off.wrapping_add(i)) as u32) << ((i as u32) * 8);
        i += 1;
    }
    v
}

unsafe fn pci_cap(d: &Dev, want: u8) -> Option<(u8, u32, u32)> {
    let status = (cfg32(d.bus, d.dev, d.func, 0x06) >> 16) as u16;
    if status & 0x10 == 0 {
        return None;
    }
    let mut ptr = cfg8(d.bus, d.dev, d.func, 0x34) & 0xfc;
    let mut guard = 0;
    while ptr != 0 && guard < 64 {
        let id = cfg8(d.bus, d.dev, d.func, ptr);
        let next = cfg8(d.bus, d.dev, d.func, ptr.wrapping_add(1)) & 0xfc;
        let cfg_type = cfg8(d.bus, d.dev, d.func, ptr.wrapping_add(3));
        if id == 0x09 && cfg_type == want {
            let bar = cfg8(d.bus, d.dev, d.func, ptr.wrapping_add(4));
            let offset = cfg32u(d.bus, d.dev, d.func, ptr.wrapping_add(8));
            let mult = cfg32u(d.bus, d.dev, d.func, ptr.wrapping_add(16));
            return Some((bar, offset, mult));
        }
        if next == ptr {
            break;
        }
        ptr = next;
        guard += 1;
    }
    None
}

unsafe fn virtio_modern() -> Option<Gfx> {
    let d = find(0x1AF4, 0x1050)?;
    serial_print("[virtio] modern probe");
    enable_bus_master(&d);
    let (cbar, coff, _cm) = pci_cap(&d, 1)?;
    let (nbar, noff, nmult) = pci_cap(&d, 2)?;
    let (ci, cb) = bar(&d, cbar);
    let (ni, nb) = bar(&d, nbar);
    if ci || ni || cb == 0 || nb == 0 {
        return None;
    }
    let common = cb + coff as u64;
    let notify = nb + noff as u64;
    VCOMMON = common;
    cstat(common, 0);
    cstat(common, 1);
    cstat(common, 3);
    cw32(common, 0x00, 0);
    let _ = cr32(common, 0x04);
    cw32(common, 0x00, 1);
    let _ = cr32(common, 0x04);
    cw32(common, 0x08, 0);
    cw32(common, 0x0C, 0);
    cw32(common, 0x08, 1);
    cw32(common, 0x0C, 1);
    cstat(common, 3 | 8);
    let mut guard = 0u32;
    while guard < 200000 {
        if cr32(common, 0x14) & 0x08 != 0 {
            break;
        }
        guard += 1;
    }
    cw32(common, 0x16, 0);
    let qmax = (cr32(common, 0x18) & 0xffff) as u16;
    if qmax == 0 {
        return None;
    }
    let want = if qmax >= 64 { 64 } else { qmax };
    cw32(common, 0x18, want as u32);
    let got = (cr32(common, 0x18) & 0xffff) as u16;
    VQSZ = if got == 0 { want } else { got };
    let base = core::ptr::addr_of!(VQ) as u64;
    cw64(common, 0x20, base);
    cw64(common, 0x28, base + 1024);
    cw64(common, 0x30, base + 4096);
    cw32(common, 0x1C, 1);
    let qoff = (cr32(common, 0x1E) & 0xffff) as u64;
    serial_print("[virtio] qoff/mult");
    serial_u32(qoff as u32);
    serial_u32(nmult);
    VNOTIFY = notify + qoff * nmult as u64;
    VMODERN = true;
    cstat(common, 3 | 8 | 4);
    serial_print("[virtio] modern queue ready");
    serial_print("[virtio] caps");
    serial_u32(cbar as u32);
    serial_u32(coff);
    serial_u32(nbar as u32);
    serial_u32(noff);
    serial_print("[virtio] bars hi/lo");
    serial_u32((cb >> 32) as u32);
    serial_u32(cb as u32);
    serial_u32((nb >> 32) as u32);
    serial_u32(nb as u32);
    serial_print("[virtio] common");
    serial_u32(common as u32);
    serial_u32((common >> 32) as u32);
    serial_print("[virtio] notify");
    serial_u32(VNOTIFY as u32);
    serial_print("[virtio] qsz");
    serial_u32(VQSZ as u32);
    serial_print("[virtio] status");
    serial_u32((cr32(common, 0x14) & 0xff) as u32);
    if !vcmd(0x0100, &[], 408) {
        return None;
    }
    let resp = core::ptr::addr_of!(VRESP) as *const u8;
    let mut dw = get32(resp, 24 + 8) as usize;
    let mut dh = get32(resp, 24 + 12) as usize;
    if dw < 640 || dh < 480 || dw > 1024 || dh > 768 {
        dw = 1024;
        dh = 768;
    }
    VW = dw;
    VH = dh;
    let mut p = [0u8; 32];
    put32(p.as_mut_ptr(), 0, 1);
    put32(p.as_mut_ptr(), 4, 1);
    put32(p.as_mut_ptr(), 8, dw as u32);
    put32(p.as_mut_ptr(), 12, dh as u32);
    if !vcmd(0x0101, &p[..16], 24) {
        return None;
    }
    let mut a = [0u8; 32];
    put32(a.as_mut_ptr(), 0, 1);
    put32(a.as_mut_ptr(), 4, 1);
    put64(a.as_mut_ptr(), 8, core::ptr::addr_of!(VFB) as u64);
    put32(a.as_mut_ptr(), 16, (dw * dh * 4) as u32);
    if !vcmd(0x0106, &a[..24], 24) {
        return None;
    }
    if !vcmd(0x0103, &scanout_payload(0, 1, dw, dh), 24) {
        return None;
    }
    if !vcmd(0x0105, &transfer_payload(1, dw, dh), 24) {
        return None;
    }
    if !vcmd(0x0104, &flush_payload(1, dw, dh), 24) {
        return None;
    }
    Some(Gfx::from_device(
        core::ptr::addr_of_mut!(VFB) as *mut u32,
        dw,
        dh,
        dw,
        false,
        true,
    ))
}

unsafe fn virtio_init() -> Option<Gfx> {
    if let Some(g) = virtio_legacy() {
        return Some(g);
    }
    virtio_modern()
}
