#![allow(dead_code)]

use crate::efi::*;
use crate::port::*;

pub const KIND_NONE: u8 = 0;
pub const KIND_E1000: u8 = 1;
pub const KIND_RTL8139: u8 = 2;
pub const KIND_VIRTIO: u8 = 3;

pub struct Nic {
    pub present: bool,
    pub kind: u8,
    pub name: &'static str,
    pub mac: [u8; 6],
    pub link: bool,
    pub tx: u32,
    pub rx: u32,
    pub ip: [u8; 4],
    pub gw: [u8; 4],
    pub arp_ok: bool,
    pub arp_mac: [u8; 6],
}

static mut NIC: Nic = Nic {
    present: false,
    kind: KIND_NONE,
    name: "无网卡",
    mac: [0; 6],
    link: false,
    tx: 0,
    rx: 0,
    ip: [10, 0, 2, 15],
    gw: [10, 0, 2, 2],
    arp_ok: false,
    arp_mac: [0; 6],
};

pub unsafe fn set_ip(ip: [u8; 4], gw: [u8; 4]) {
    let n = &mut *core::ptr::addr_of_mut!(NIC);
    n.ip = ip;
    n.gw = gw;
}

pub fn info() -> &'static Nic {
    unsafe { &*core::ptr::addr_of!(NIC) }
}

#[repr(C, align(4096))]
struct P4K([u8; 4096]);
#[repr(C, align(4096))]
struct P32K([u8; 32768]);

#[derive(Clone, Copy)]
#[repr(C)]
struct TxD2 {
    addr: u64,
    len: u16,
    cso: u8,
    cmd: u8,
    status: u8,
    css: u8,
    special: u16,
}

#[derive(Clone, Copy)]
#[repr(C)]
struct RxD2 {
    addr: u64,
    len: u16,
    csum: u16,
    status: u8,
    err: u8,
    special: u16,
}

#[repr(C, align(128))]
struct TxdRing([TxD2; 8]);
#[repr(C, align(128))]
struct RxdRing([RxD2; 32]);
static mut E_TXD: TxdRing = TxdRing([TxD2 { addr: 0, len: 0, cso: 0, cmd: 0, status: 0, css: 0, special: 0 }; 8]);
static mut E_RXD: RxdRing = RxdRing([RxD2 { addr: 0, len: 0, csum: 0, status: 0, err: 0, special: 0 }; 32]);
static mut E_TXB: P32K = P32K([0; 32768]);
static mut E_RXB: P32K = P32K([0; 32768]);
static mut E_TX_TAIL: usize = 0;
static mut E_RX_TAIL: usize = 0;

static mut R_RXB: P32K = P32K([0; 32768]);
static mut R_LAST: usize = 0;
static mut R_TX: usize = 0;
static mut R_TXB: [u8; 2048] = [0; 2048];

const VQN: usize = 256;
static mut V_RXQ: [P4K; 4] = [P4K([0; 4096]), P4K([0; 4096]), P4K([0; 4096]), P4K([0; 4096])];
static mut V_TXQ: [P4K; 4] = [P4K([0; 4096]), P4K([0; 4096]), P4K([0; 4096]), P4K([0; 4096])];
static mut V_RX_QSZ: usize = 0;
static mut V_TX_QSZ: usize = 0;
static mut V_RX_ACTIVE: usize = 0;
static mut V_RXB: P32K = P32K([0; 32768]);
static mut V_TXB: P4K = P4K([0; 4096]);
static mut V_RX_SEEN: u16 = 0;
static mut V_TX_SEEN: u16 = 0;
static mut V_RX_USED: usize = 0;
static mut V_IO: u16 = 0;
static mut V_LINK: bool = true;

static mut MMIO: *mut u32 = core::ptr::null_mut();
static mut IO_BASE: u16 = 0;

fn phys_u32(p: *const u8) -> u32 {
    p as usize as u32
}

unsafe fn mr(off: u32) -> u32 {
    core::ptr::read_volatile((MMIO as usize + off as usize) as *const u32)
}

unsafe fn mw(off: u32, v: u32) {
    core::ptr::write_volatile((MMIO as usize + off as usize) as *mut u32, v);
}

fn delay_ms(ms: usize) {
    unsafe { bs_stall(ms * 1000) }
}

pub unsafe fn init() -> bool {
    unsafe {
        if virtio_init() { return true; }
        if e1000_init() { return true; }
        if rtl_init() { return true; }
    }
    false
}

// ---------------- Intel e1000 ----------------
unsafe fn e1000_init() -> bool {
    let d = match find(0x8086, 0x100E)
        .or_else(|| find(0x8086, 0x100F))
        .or_else(|| find(0x8086, 0x10D3))
        .or_else(|| find(0x8086, 0x153A))
    {
        Some(d) => d,
        None => return false,
    };
    let (io, base) = bar(&d, 0);
    if io || base == 0 { return false; }
    enable_bus_master(&d);
    MMIO = base as *mut u32;
    mw(0x0000, 0x0400_0000);
    delay_ms(10);
    mw(0x0000, 0x0000_0002);
    delay_ms(5);
    let ral = mr(0x5400);
    let rah = mr(0x5404);
    let mut mac = [0u8; 6];
    mac[0] = ral as u8;
    mac[1] = (ral >> 8) as u8;
    mac[2] = (ral >> 16) as u8;
    mac[3] = (ral >> 24) as u8;
    mac[4] = rah as u8;
    mac[5] = (rah >> 8) as u8;
    if mac == [0; 6] || mac == [0xff; 6] {
        mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
    }
    mw(0x5400, ral);
    mw(0x5404, rah | 0x8000_0000);
    let rxd = core::ptr::addr_of_mut!(E_RXD) as *mut RxD2;
    let rxbuf = core::ptr::addr_of_mut!(E_RXB) as *mut u8;
    let mut i = 0;
    while i < 32 {
        let dd = &mut *rxd.add(i);
        dd.addr = phys_u32(rxbuf.add(i * 2048)) as u64;
        dd.status = 0;
        i += 1;
    }
    mw(0x2800, phys_u32(rxd as *const u8));
    mw(0x2804, 0);
    mw(0x2808, 32 * 16);
    mw(0x2810, 0);
    mw(0x2818, 31);
    mw(0x0100, 0x0400_801A);
    let txd = core::ptr::addr_of_mut!(E_TXD) as *mut TxD2;
    let mut i = 0;
    while i < 8 {
        let dd = &mut *txd.add(i);
        dd.status = 1;
        i += 1;
    }
    mw(0x3800, phys_u32(txd as *const u8));
    mw(0x3804, 0);
    mw(0x3808, 8 * 16);
    mw(0x3810, 0);
    mw(0x3818, 0);
    mw(0x0400, 0x0004_010A);
    let st = mr(0x0008);
    let n = &mut *core::ptr::addr_of_mut!(NIC);
    n.present = true;
    n.kind = KIND_E1000;
    n.name = "Intel 8254x (e1000)";
    n.mac = mac;
    n.link = st & 2 != 0;
    E_TX_TAIL = 0;
    E_RX_TAIL = 0;
    serial_print("[net] e1000 ready");
    true
}

unsafe fn e1000_send(f: &[u8]) -> bool {
    if !info().present || info().kind != KIND_E1000 || f.len() > 2048 { return false; }
    let idx = E_TX_TAIL;
    let txd = core::ptr::addr_of_mut!(E_TXD) as *mut TxD2;
    let txb = core::ptr::addr_of_mut!(E_TXB) as *mut u8;
    let buf = txb.add(idx * 2048);
    let mut i = 0;
    while i < f.len() { *buf.add(i) = f[i]; i += 1; }
    let dd = &mut *txd.add(idx);
    dd.addr = phys_u32(buf) as u64;
    dd.len = f.len() as u16;
    dd.cso = 0;
    dd.cmd = 0x0B;
    dd.status = 0;
    dd.css = 0;
    mw(0x3818, ((idx + 1) % 8) as u32);
    let mut g = 0;
    while (*txd.add(idx)).status & 1 == 0 && g < 2000 { delay_ms(1); g += 1; }
    E_TX_TAIL = (idx + 1) % 8;
    (*core::ptr::addr_of_mut!(NIC)).tx += 1;
    true
}

unsafe fn e1000_recv(buf: &mut [u8]) -> Option<usize> {
    if !info().present || info().kind != KIND_E1000 { return None; }
    let rxd = core::ptr::addr_of_mut!(E_RXD) as *mut RxD2;
    let idx = E_RX_TAIL;
    let dd = &mut *rxd.add(idx);
    if dd.status & 1 == 0 { return None; }
    let mut len = dd.len as usize;
    if len > buf.len() { len = buf.len(); }
    let rxb = core::ptr::addr_of_mut!(E_RXB) as *mut u8;
    let src = rxb.add(idx * 2048);
    let mut i = 0;
    while i < len { buf[i] = *src.add(i); i += 1; }
    dd.status = 0;
    mw(0x2818, idx as u32);
    E_RX_TAIL = (idx + 1) % 32;
    (*core::ptr::addr_of_mut!(NIC)).rx += 1;
    Some(len)
}

// ---------------- Realtek RTL8139 ----------------
unsafe fn rtl_init() -> bool {
    let d = match find(0x10EC, 0x8139) { Some(d) => d, None => return false };
    let (io, base) = bar(&d, 0);
    if !io || base == 0 { return false; }
    enable_bus_master(&d);
    IO_BASE = base as u16;
    let b = IO_BASE;
    outb(b + 0x52, 0x00);
    outb(b + 0x37, 0x10);
    let mut g = 0;
    while inb(b + 0x37) & 0x10 != 0 && g < 1000 { delay_ms(1); g += 1; }
    let mut mac = [0u8; 6];
    let mut i = 0;
    while i < 6 { mac[i] = inb(b + i as u16); i += 1; }
    if mac == [0; 6] || mac == [0xff; 6] { mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x57]; }
    let rb = core::ptr::addr_of_mut!(R_RXB) as *mut u8;
    outl(b + 0x30, phys_u32(rb));
    outl(b + 0x44, 0x0000_008F);
    outw(b + 0x3C, 0x0000);
    outb(b + 0x37, 0x0C);
    R_LAST = 0;
    R_TX = 0;
    outw(b + 0x38, 0);
    let n = &mut *core::ptr::addr_of_mut!(NIC);
    n.present = true;
    n.kind = KIND_RTL8139;
    n.name = "Realtek RTL8139";
    n.mac = mac;
    n.link = true;
    serial_print("[net] rtl8139 ready");
    true
}

unsafe fn rtl_send(f: &[u8]) -> bool {
    if !info().present || info().kind != KIND_RTL8139 || f.len() > 1792 { return false; }
    let b = IO_BASE;
    let idx = R_TX % 4;
    let mut g = 0;
    while inl(b + 0x10 + (idx as u16) * 4) & 0x2000 != 0 && g < 2000 { delay_ms(1); g += 1; }
    let tb = core::ptr::addr_of_mut!(R_TXB) as *mut u8;
    let mut i = 0;
    while i < f.len() { *tb.add(i) = f[i]; i += 1; }
    outl(b + 0x20 + (idx as u16) * 4, phys_u32(tb));
    outl(b + 0x10 + (idx as u16) * 4, f.len() as u32);
    R_TX += 1;
    (*core::ptr::addr_of_mut!(NIC)).tx += 1;
    true
}

unsafe fn rtl_recv(buf: &mut [u8]) -> Option<usize> {
    if !info().present || info().kind != KIND_RTL8139 { return None; }
    let b = IO_BASE;
    let cbr = inw(b + 0x3A) as usize;
    if cbr == R_LAST { return None; }
    let rb = core::ptr::addr_of_mut!(R_RXB) as *mut u8;
    let mut off = R_LAST;
    if off + 4 > 32768 { off = 0; }
    let status = (*(rb.add(off)) as u16) | ((*(rb.add(off + 1)) as u16) << 8);
    let plen = (*(rb.add(off + 2)) as u16) | ((*(rb.add(off + 3)) as u16) << 8);
    let mut len = plen as usize;
    if status & 1 == 0 || len < 14 || len > 1700 || off + 4 + len > 32768 {
        R_LAST = 0;
        outw(b + 0x38, 0);
        return None;
    }
    if len > 4 { len -= 4; }
    if len > buf.len() { len = buf.len(); }
    let mut i = 0;
    while i < len { buf[i] = *rb.add(off + 4 + i); i += 1; }
    let mut next = (off + 4 + plen as usize + 3) & !3;
    if next >= 8192 { next = 0; }
    R_LAST = next;
    outw(b + 0x38, (next as u16).wrapping_sub(16));
    (*core::ptr::addr_of_mut!(NIC)).rx += 1;
    Some(len)
}

// ---------------- virtio-net (legacy) ----------------
unsafe fn virtio_init() -> bool {
    let d = match find(0x1AF4, 0x1000) { Some(d) => d, None => return false };
    let (io, base) = bar(&d, 0);
    if !io || base == 0 { return false; }
    enable_bus_master(&d);
    let b = base as u16;
    V_IO = b;
    outb(b + 0x12, 0);
    delay_ms(2);
    outb(b + 0x12, 1);
    delay_ms(1);
    outb(b + 0x12, 3);
    delay_ms(1);
    let host = inl(b);
    let guest = host & ((1 << 5) | (1 << 16));
    outl(b + 0x04, guest);
    outb(b + 0x12, 1 | 2 | 8);
    delay_ms(1);
    let rx0 = core::ptr::addr_of_mut!(V_RXQ) as *mut u8;
    let tx0 = core::ptr::addr_of_mut!(V_TXQ) as *mut u8;
    outw(b + 0x0E, 0);
    let rn = inw(b + 0x0C) as usize;
    outl(b + 0x08, (rx0 as u32) >> 12);
    outw(b + 0x0E, 1);
    let tn = inw(b + 0x0C) as usize;
    outl(b + 0x08, (tx0 as u32) >> 12);
    outw(b + 0x0E, 0);
    if rn < 2 || tn < 1 { return false; }
    let mut i = 0;
    while i < 4 * 4096 { *rx0.add(i) = 0; i += 1; }
    i = 0;
    while i < 4 * 4096 { *tx0.add(i) = 0; i += 1; }
    V_RX_QSZ = rn;
    V_TX_QSZ = tn;
    let n = if rn < 32 { rn } else { 32 };
    V_RX_USED = n;
    V_RX_ACTIVE = n;
    let desc = rx0 as *mut VDesc;
    let rxb = core::ptr::addr_of_mut!(V_RXB) as *mut u8;
    i = 0;
    while i < n {
        let dd = &mut *desc.add(i);
        dd.addr = phys_u32(rxb.add(i * 2048)) as u64;
        dd.len = 2048;
        dd.flags = 2;
        dd.next = 0;
        i += 1;
    }
    let avail = rx0.add(rn * 16) as *mut u16;
    *avail.add(1) = n as u16;
    i = 0;
    while i < n { *avail.add(2 + i) = i as u16; i += 1; }
    let mut mac = [0u8; 6];
    if guest & (1 << 5) != 0 {
        i = 0;
        while i < 6 { mac[i] = inb(b + 0x14 + i as u16); i += 1; }
    }
    if mac == [0; 6] || mac == [0xff; 6] { mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x58]; }
    if guest & (1 << 16) != 0 {
        let s = inw(b + 0x1A);
        V_LINK = s & 1 != 0;
    }
    outb(b + 0x12, 1 | 2 | 8 | 4);
    delay_ms(1);
    outw(b + 0x10, 0);
    let ni = &mut *core::ptr::addr_of_mut!(NIC);
    ni.present = true;
    ni.kind = KIND_VIRTIO;
    ni.name = "virtio-net (legacy)";
    ni.mac = mac;
    ni.link = V_LINK;
    V_RX_SEEN = 0;
    V_TX_SEEN = 0;
    serial_print("[net] virtio-net ready");
    true
}

#[repr(C)]
struct VDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
struct VUsedElem {
    id: u32,
    len: u32,
}

unsafe fn virtio_send(f: &[u8]) -> bool {
    if !info().present || info().kind != KIND_VIRTIO || f.len() > 4096 { return false; }
    let b = V_IO;
    let tx0 = core::ptr::addr_of_mut!(V_TXQ) as *mut u8;
    let tb = core::ptr::addr_of_mut!(V_TXB) as *mut u8;
    let mut i = 0;
    while i < 10 { *tb.add(i) = 0; i += 1; }
    i = 0;
    while i < f.len() { *tb.add(10 + i) = f[i]; i += 1; }
    let tn = V_TX_QSZ;
    let desc = tx0 as *mut VDesc;
    let dd = &mut *desc.add(0);
    dd.addr = phys_u32(tb) as u64;
    dd.len = (f.len() + 10) as u32;
    dd.flags = 0;
    dd.next = 0;
    let avail = tx0.add(tn * 16) as *mut u16;
    let idx = *avail.add(1);
    *avail.add(2 + (idx as usize % tn)) = 0;
    *avail.add(1) = idx.wrapping_add(1);
    outw(b + 0x10, 1);
    let uoff = ((tn * 16 + 4 + tn * 2 + 2 + 4095) / 4096) * 4096;
    let used = tx0.add(uoff) as *mut u16;
    let mut g = 0;
    while *used.add(1) == V_TX_SEEN && g < 3000 { delay_ms(1); g += 1; }
    V_TX_SEEN = *used.add(1);
    (*core::ptr::addr_of_mut!(NIC)).tx += 1;
    true
}

unsafe fn virtio_recv(buf: &mut [u8]) -> Option<usize> {
    if !info().present || info().kind != KIND_VIRTIO { return None; }
    let b = V_IO;
    let rx0 = core::ptr::addr_of_mut!(V_RXQ) as *mut u8;
    let rn = V_RX_QSZ;
    let uoff = ((rn * 16 + 4 + rn * 2 + 2 + 4095) / 4096) * 4096;
    let used = rx0.add(uoff) as *mut u16;
    let uidx = *used.add(1);
    if uidx == V_RX_SEEN { return None; }
    let n = V_RX_ACTIVE;
    let elem = rx0.add(uoff + 4) as *mut VUsedElem;
    let e = &*elem.add((V_RX_SEEN as usize) % rn);
    let id = e.id as usize;
    let mut len = e.len as usize;
    V_RX_SEEN = V_RX_SEEN.wrapping_add(1);
    if id >= n { return None; }
    if len < 10 { return None; }
    len -= 10;
    if len > buf.len() { len = buf.len(); }
    let rxb = core::ptr::addr_of_mut!(V_RXB) as *mut u8;
    let mut i = 0;
    while i < len { buf[i] = *rxb.add(id * 2048 + 10 + i); i += 1; }
    let desc = rx0 as *mut VDesc;
    desc.add(id).as_mut().unwrap().len = 2048;
    desc.add(id).as_mut().unwrap().flags = 2;
    let avail = rx0.add(rn * 16) as *mut u16;
    let ai = *avail.add(1);
    *avail.add(2 + (ai as usize % rn)) = id as u16;
    *avail.add(1) = ai.wrapping_add(1);
    outw(b + 0x10, 0);
    (*core::ptr::addr_of_mut!(NIC)).rx += 1;
    Some(len)
}

// ---------------- 公共接口 ----------------
pub unsafe fn send(frame: &[u8]) -> bool {
    match info().kind {
        KIND_E1000 => e1000_send(frame),
        KIND_RTL8139 => rtl_send(frame),
        KIND_VIRTIO => virtio_send(frame),
        _ => false,
    }
}

pub unsafe fn recv(buf: &mut [u8]) -> Option<usize> {
    match info().kind {
        KIND_E1000 => e1000_recv(buf),
        KIND_RTL8139 => rtl_recv(buf),
        KIND_VIRTIO => virtio_recv(buf),
        _ => None,
    }
}

pub unsafe fn refresh_link() {
    if info().kind == KIND_E1000 && !MMIO.is_null() {
        (*core::ptr::addr_of_mut!(NIC)).link = mr(0x0008) & 2 != 0;
    } else if info().kind == KIND_VIRTIO {
        (*core::ptr::addr_of_mut!(NIC)).link = V_LINK;
    }
}

fn csum(data: &[u8]) -> u16 {
    let mut s = 0u32;
    let mut i = 0;
    while i + 1 < data.len() {
        s += ((data[i] as u32) << 8) | data[i + 1] as u32;
        i += 2;
    }
    if i < data.len() { s += (data[i] as u32) << 8; }
    while s >> 16 != 0 { s = (s & 0xffff) + (s >> 16); }
    !(s as u16)
}

fn put16(d: &mut [u8], o: usize, v: u16) {
    d[o] = (v >> 8) as u8;
    d[o + 1] = v as u8;
}

unsafe fn send_arp(tip: [u8; 4]) {
    let mut f = [0u8; 42];
    let mac = info().mac;
    let ip = info().ip;
    let mut i = 0;
    while i < 6 { f[i] = 0xff; f[6 + i] = mac[i]; i += 1; }
    put16(&mut f, 12, 0x0806);
    put16(&mut f, 14, 1);
    put16(&mut f, 16, 0x0800);
    f[18] = 6;
    f[19] = 4;
    put16(&mut f, 20, 1);
    i = 0;
    while i < 6 { f[22 + i] = mac[i]; f[32 + i] = 0; i += 1; }
    i = 0;
    while i < 4 { f[28 + i] = ip[i]; f[38 + i] = tip[i]; i += 1; }
    send(&f);
}

unsafe fn send_icmp(tip: [u8; 4]) {
    let mac = info().arp_mac;
    let ip = info().ip;
    let mut f = [0u8; 42 + 16];
    let mut i = 0;
    while i < 6 { f[i] = mac[i]; f[6 + i] = info().mac[i]; i += 1; }
    put16(&mut f, 12, 0x0800);
    f[14] = 0x45;
    f[15] = 0;
    put16(&mut f, 16, 20 + 16);
    put16(&mut f, 18, 0x1234);
    put16(&mut f, 20, 0);
    f[22] = 64;
    f[23] = 1;
    put16(&mut f, 24, 0);
    i = 0;
    while i < 4 { f[26 + i] = ip[i]; f[30 + i] = tip[i]; i += 1; }
    let ipck = csum(&f[14..34]);
    put16(&mut f, 24, ipck);
    let o = 34;
    f[o] = 8;
    f[o + 1] = 0;
    put16(&mut f, o + 2, 0);
    put16(&mut f, o + 4, 0x0001);
    put16(&mut f, o + 6, 0x0001);
    let mut k = 0;
    while k < 8 { f[o + 8 + k] = k as u8 + 0x30; k += 1; }
    let ck = csum(&f[o..o + 16]);
    put16(&mut f, o + 2, ck);
    send(&f);
}

unsafe fn handle_frame(f: &[u8]) {
    if f.len() < 14 { return; }
    let et = ((f[12] as u16) << 8) | f[13] as u16;
    if et == 0x0806 && f.len() >= 42 {
        let op = ((f[20] as u16) << 8) | f[21] as u16;
        if op == 2 {
            let n = &mut *core::ptr::addr_of_mut!(NIC);
            let mut i = 0;
            while i < 6 { n.arp_mac[i] = f[22 + i]; i += 1; }
            n.arp_ok = true;
        }
    } else if et == 0x0800 && f.len() >= 42 {
        let ihl = (f[14] & 0x0f) as usize * 4;
        let proto = f[23];
        if proto == 1 && f.len() >= 14 + ihl + 8 {
            let t = f[14 + ihl];
            if t == 0 { PING_OK = true; }
        }
    }
}

static mut PING_OK: bool = false;

pub unsafe fn poll_once() {
    let mut buf = [0u8; 1600];
    let mut g = 0;
    while g < 8 {
        match recv(&mut buf) {
            Some(n) => handle_frame(&buf[..n]),
            None => break,
        }
        g += 1;
    }
}

pub unsafe fn ping(tip: [u8; 4], timeout_ms: u32) -> bool {
    if !info().present { return false; }
    let n = &mut *core::ptr::addr_of_mut!(NIC);
    n.arp_ok = false;
    PING_OK = false;
    send_arp(tip);
    let mut t = 0u32;
    while t < 600 && !(*core::ptr::addr_of!(NIC)).arp_ok {
        poll_once();
        delay_ms(1);
        t += 1;
    }
    if !(*core::ptr::addr_of!(NIC)).arp_ok { return false; }
    send_icmp(tip);
    t = 0;
    while t < timeout_ms {
        poll_once();
        if PING_OK { return true; }
        delay_ms(1);
        t += 1;
    }
    false
}

pub fn parse_ip(s: &str) -> Option<[u8; 4]> {
    let b = s.as_bytes();
    let mut out = [0u8; 4];
    let mut part = 0usize;
    let mut val = 0u32;
    let mut any = false;
    for c in b.iter() {
        if *c >= b'0' && *c <= b'9' {
            val = val * 10 + (*c - b'0') as u32;
            if val > 255 { return None; }
            any = true;
        } else if *c == b'.' {
            if part >= 3 || !any { return None; }
            out[part] = val as u8;
            part += 1;
            val = 0;
            any = false;
        } else {
            return None;
        }
    }
    if part != 3 || !any { return None; }
    out[3] = val as u8;
    Some(out)
}

pub unsafe fn fmt_mac(out: &mut [u8]) -> usize {
    let m = info().mac;
    let mut l = 0usize;
    let mut i = 0;
    while i < 6 {
        let hi = m[i] >> 4;
        let lo = m[i] & 0xf;
        if l < out.len() { out[l] = if hi < 10 { b'0' + hi } else { b'a' + hi - 10 }; l += 1; }
        if l < out.len() { out[l] = if lo < 10 { b'0' + lo } else { b'a' + lo - 10 }; l += 1; }
        if i < 5 && l < out.len() { out[l] = b':'; l += 1; }
        i += 1;
    }
    l
}

pub unsafe fn fmt_ip(out: &mut [u8], ip: [u8; 4]) -> usize {
    let mut l = 0usize;
    let mut i = 0;
    while i < 4 {
        l += crate::hw::num2(&mut out[l..], ip[i] as u32, 1);
        if i < 3 && l < out.len() { out[l] = b'.'; l += 1; }
        i += 1;
    }
    l
}
