#![allow(dead_code)]

use crate::net;
use crate::efi::bs_stall;

pub struct Cfg {
    pub ip: [u8; 4],
    pub mask: [u8; 4],
    pub gw: [u8; 4],
    pub dns: [u8; 4],
    pub dhcp: bool,
    pub ready: bool,
    pub tried: bool,
}

static mut CFG: Cfg = Cfg {
    ip: [10, 0, 2, 15],
    mask: [255, 255, 255, 0],
    gw: [10, 0, 2, 2],
    dns: [10, 0, 2, 3],
    dhcp: false,
    ready: false,
    tried: false,
};

pub fn cfg() -> &'static Cfg {
    unsafe { &*core::ptr::addr_of!(CFG) }
}

#[derive(Clone, Copy)]
struct ArpEnt {
    ip: [u8; 4],
    mac: [u8; 6],
    ok: bool,
}

static mut ARP: [ArpEnt; 8] = [ArpEnt { ip: [0; 4], mac: [0; 6], ok: false }; 8];
static mut ARP_N: usize = 0;

static mut XID: u32 = 0x4859_4f53;
static mut IP_ID: u16 = 1;
static mut SPORT: u16 = 49152;

static mut TCP: Tcb = Tcb {
    ip: [0; 4], mac: [0; 6], sport: 0, dport: 0,
    snd_nxt: 0, snd_una: 0, rcv_nxt: 0, state: 0,
};
struct Tcb {
    ip: [u8; 4],
    mac: [u8; 6],
    sport: u16,
    dport: u16,
    snd_nxt: u32,
    snd_una: u32,
    rcv_nxt: u32,
    state: u8,
}

static mut HTTPBUF: [u8; 8192] = [0; 8192];

fn put16(d: &mut [u8], o: usize, v: u16) {
    d[o] = (v >> 8) as u8;
    d[o + 1] = v as u8;
}

fn get16(d: &[u8], o: usize) -> u16 {
    ((d[o] as u16) << 8) | d[o + 1] as u16
}

fn put32(d: &mut [u8], o: usize, v: u32) {
    d[o] = (v >> 24) as u8;
    d[o + 1] = (v >> 16) as u8;
    d[o + 2] = (v >> 8) as u8;
    d[o + 3] = v as u8;
}

fn get32(d: &[u8], o: usize) -> u32 {
    ((d[o] as u32) << 24) | ((d[o + 1] as u32) << 16) | ((d[o + 2] as u32) << 8) | d[o + 3] as u32
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

fn eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() < b.len() { return false; }
    let mut i = 0;
    while i < b.len() {
        if a[i] != b[i] { return false; }
        i += 1;
    }
    true
}

fn is_ip(s: &str) -> Option<[u8; 4]> {
    net::parse_ip(s)
}

pub fn parse_url<'a>(url: &'a str) -> Option<(&'a str, &'a str, u16)> {
    {
        let b = url.as_bytes();
        if !eq(b, b"http://") { return None; }
        let rest = &url[7..];
        let rb = rest.as_bytes();
        let mut i = 0;
        let mut host_end = rest.len();
        let mut path_start = rest.len();
        while i < rb.len() {
            if rb[i] == b'/' { host_end = i; path_start = i; break; }
            i += 1;
        }
        let host = &rest[..host_end];
        let path = if path_start < rest.len() { &rest[path_start..] } else { "/" };
        let mut port = 80u16;
        let mut h = host;
        let mut j = 0;
        while j < host.len() {
            if host.as_bytes()[j] == b':' {
                h = &host[..j];
                let mut p = 0u16;
                let mut k = j + 1;
                while k < host.len() { p = p * 10 + (host.as_bytes()[k] - b'0') as u16; k += 1; }
                port = p;
                break;
            }
            j += 1;
        }
        Some((h, path, port))
    }
}

unsafe fn send_frame(dst: [u8; 6], et: u16, payload: &[u8]) -> bool {
    let mac = net::info().mac;
    let mut f = [0u8; 1600];
    let mut i = 0;
    while i < 6 { f[i] = dst[i]; f[6 + i] = mac[i]; i += 1; }
    put16(&mut f, 12, et);
    let n = if payload.len() > 1580 { 1580 } else { payload.len() };
    i = 0;
    while i < n { f[14 + i] = payload[i]; i += 1; }
    net::send(&f[..14 + n])
}

unsafe fn arp_find(ip: [u8; 4]) -> Option<[u8; 6]> {
    let a = &*core::ptr::addr_of!(ARP);
    let mut i = 0;
    while i < ARP_N {
        if a[i].ok && a[i].ip == ip { return Some(a[i].mac); }
        i += 1;
    }
    None
}

unsafe fn arp_add(ip: [u8; 4], mac: [u8; 6]) {
    let a = &mut *core::ptr::addr_of_mut!(ARP);
    let mut i = 0;
    while i < ARP_N {
        if a[i].ip == ip { a[i].mac = mac; a[i].ok = true; return; }
        i += 1;
    }
    if ARP_N < 8 {
        a[ARP_N].ip = ip;
        a[ARP_N].mac = mac;
        a[ARP_N].ok = true;
        ARP_N += 1;
    } else {
        a[0].ip = ip; a[0].mac = mac; a[0].ok = true;
    }
}

unsafe fn send_arp_req(tip: [u8; 4]) {
    let mac = net::info().mac;
    let ip = cfg().ip;
    let mut f = [0u8; 42];
    let mut i = 0;
    while i < 6 { f[i] = 0xff; f[6 + i] = mac[i]; i += 1; }
    put16(&mut f, 12, 0x0806);
    put16(&mut f, 14, 1);
    put16(&mut f, 16, 0x0800);
    f[18] = 6; f[19] = 4;
    put16(&mut f, 20, 1);
    i = 0;
    while i < 6 { f[22 + i] = mac[i]; f[32 + i] = 0; i += 1; }
    i = 0;
    while i < 4 { f[28 + i] = ip[i]; f[38 + i] = tip[i]; i += 1; }
    net::send(&f);
}

unsafe fn next_ip(buf: &mut [u8]) -> Option<usize> {
    let mut tmp = [0u8; 1600];
    let mut g = 0;
    while g < 16 {
        match net::recv(&mut tmp) {
            Some(n) => {
                if n >= 42 && get16(&tmp, 12) == 0x0806 && get16(&tmp, 20) == 2 {
                    let mut ip = [0u8; 4];
                    let mut mac = [0u8; 6];
                    let mut i = 0;
                    while i < 4 { ip[i] = tmp[28 + i]; i += 1; }
                    i = 0;
                    while i < 6 { mac[i] = tmp[22 + i]; i += 1; }
                    arp_add(ip, mac);
                } else if n >= 34 && get16(&tmp, 12) == 0x0800 {
                    let ihl = (tmp[14] & 0x0f) as usize * 4;
                    let tot = get16(&tmp, 16) as usize;
                    if ihl >= 20 && tot >= ihl && 14 + tot <= n {
                        let l = tot;
                        if l <= buf.len() {
                            let mut k = 0;
                            while k < l { buf[k] = tmp[14 + k]; k += 1; }
                            return Some(l);
                        }
                    }
                }
            }
            None => break,
        }
        g += 1;
    }
    None
}

unsafe fn arp_resolve(ip: [u8; 4], ms: u32) -> Option<[u8; 6]> {
    if let Some(m) = arp_find(ip) { return Some(m); }
    send_arp_req(ip);
    let mut t = 0u32;
    while t < ms {
        let mut b = [0u8; 1600];
        let _ = next_ip(&mut b);
        if let Some(m) = arp_find(ip) { return Some(m); }
        bs_stall(1000);
        t += 1;
    }
    None
}

unsafe fn ip_build(src: [u8; 4], dst: [u8; 4], proto: u8, payload: &[u8], out: &mut [u8]) -> usize {
    let tot = 20 + payload.len();
    out[0] = 0x45;
    out[1] = 0;
    put16(out, 2, tot as u16);
    IP_ID = IP_ID.wrapping_add(1);
    put16(out, 4, IP_ID);
    put16(out, 6, 0);
    out[8] = 64;
    out[9] = proto;
    put16(out, 10, 0);
    let mut i = 0;
    while i < 4 { out[12 + i] = src[i]; out[16 + i] = dst[i]; i += 1; }
    put16(out, 10, csum(&out[..20]));
    i = 0;
    while i < payload.len() { out[20 + i] = payload[i]; i += 1; }
    tot
}

unsafe fn send_ip(dst: [u8; 4], proto: u8, payload: &[u8]) -> bool {
    let c = cfg();
    let nexthop = if (dst[0] == c.ip[0] && dst[1] == c.ip[1] && dst[2] == c.ip[2]) || dst == [255, 255, 255, 255] { dst } else { c.gw };
    let mac = if dst == [255, 255, 255, 255] {
        [0xff; 6]
    } else {
        match arp_resolve(nexthop, 800) { Some(m) => m, None => return false }
    };
    let mut ip = [0u8; 1600];
    let n = ip_build(c.ip, dst, proto, payload, &mut ip);
    send_frame(mac, 0x0800, &ip[..n])
}

unsafe fn udp_send(dst: [u8; 4], sport: u16, dport: u16, data: &[u8]) -> bool {
    let mut u = [0u8; 1500];
    let ulen = 8 + data.len();
    put16(&mut u, 0, sport);
    put16(&mut u, 2, dport);
    put16(&mut u, 4, ulen as u16);
    put16(&mut u, 6, 0);
    let mut i = 0;
    while i < data.len() { u[8 + i] = data[i]; i += 1; }
    let c = cfg();
    let mut ph = [0u8; 12];
    i = 0;
    while i < 4 { ph[i] = c.ip[i]; ph[4 + i] = dst[i]; i += 1; }
    ph[8] = 0; ph[9] = 17;
    put16(&mut ph, 10, ulen as u16);
    let mut sum = 0u32;
    let mut k = 0;
    while k < 12 { sum += get16(&ph, k) as u32; k += 2; }
    let mut j = 0;
    while j + 1 < ulen { sum += get16(&u, j) as u32; j += 2; }
    if j < ulen { sum += (u[j] as u32) << 8; }
    while sum >> 16 != 0 { sum = (sum & 0xffff) + (sum >> 16); }
    let ck = !(sum as u16);
    put16(&mut u, 6, if ck == 0 { 0xffff } else { ck });
    send_ip(dst, 17, &u[..ulen])
}

struct UdpPkt {
    src: [u8; 4],
    sport: u16,
    dport: u16,
    off: usize,
    len: usize,
}

unsafe fn parse_udp(ip: &[u8]) -> Option<UdpPkt> {
    if ip.len() < 28 || ip[9] != 17 { return None; }
    let ihl = (ip[0] & 0x0f) as usize * 4;
    if ip.len() < ihl + 8 { return None; }
    let ulen = get16(ip, ihl + 4) as usize;
    if ihl + ulen > ip.len() { return None; }
    let mut src = [0u8; 4];
    let mut i = 0;
    while i < 4 { src[i] = ip[12 + i]; i += 1; }
    Some(UdpPkt { src, sport: get16(ip, ihl), dport: get16(ip, ihl + 2), off: ihl + 8, len: ulen - 8 })
}

unsafe fn dhcp_try() -> bool {
    let mac = net::info().mac;
    XID = XID.wrapping_add(0x1234_5678);
    let xid = XID;
    let mut p = [0u8; 300];
    p[0] = 1; p[1] = 1; p[2] = 6; p[3] = 0;
    put32(&mut p, 4, xid);
    put16(&mut p, 8, 0);
    put16(&mut p, 10, 0x8000);
    let mut i = 0;
    while i < 6 { p[28 + i] = mac[i]; i += 1; }
    put32(&mut p, 236, 0x6382_5363);
    let mut o = 240;
    p[o] = 53; p[o + 1] = 1; p[o + 2] = 1; o += 3;
    p[o] = 55; p[o + 1] = 3; p[o + 2] = 1; p[o + 3] = 3; p[o + 4] = 6; o += 5;
    p[o] = 255; o += 1;
    let mut u = [0u8; 320];
    put16(&mut u, 0, 68);
    put16(&mut u, 2, 67);
    put16(&mut u, 4, (8 + o) as u16);
    let mut k = 0;
    while k < o { u[8 + k] = p[k]; k += 1; }
    put16(&mut u, 6, 0);
    let mut ip = [0u8; 400];
    let n = ip_build([0; 4], [255, 255, 255, 255], 17, &u[..8 + o], &mut ip);
    send_frame([0xff; 6], 0x0800, &ip[..n]);

    let mut stage = 0u8;
    let mut offered = [0u8; 4];
    let mut mask = [255, 255, 255, 0];
    let mut gw = [0u8; 4];
    let mut dns = [0u8; 4];
    let mut server = [0u8; 4];
    let mut t = 0u32;
    while t < 1600 {
        let mut b = [0u8; 1600];
        if let Some(n) = next_ip(&mut b) {
            if let Some(u) = parse_udp(&b[..n]) {
                if u.src == [255, 255, 255, 255] || u.sport == 67 {
                    if u.sport == 67 && u.dport == 68 && n >= u.off + 240 {
                        let bp = u.off;
                        if b[bp] == 2 && get32(&b, bp + 4) == xid {
                            offered.copy_from_slice(&b[bp + 16..bp + 20]);
                            let mut q = bp + 240;
                            let mut typ = 0u8;
                            while q + 1 < n && b[q] != 255 {
                                let code = b[q];
                                let len = b[q + 1] as usize;
                                if code == 53 && len == 1 { typ = b[q + 2]; }
                                if code == 1 && len == 4 { mask.copy_from_slice(&b[q + 2..q + 6]); }
                                if code == 3 && len >= 4 { gw.copy_from_slice(&b[q + 2..q + 6]); }
                                if code == 6 && len >= 4 { dns.copy_from_slice(&b[q + 2..q + 6]); }
                                if code == 54 && len == 4 { server.copy_from_slice(&b[q + 2..q + 6]); }
                                q += 2 + len;
                            }
                            if stage == 0 && typ == 2 {
                                stage = 1;
                                let mut r = [0u8; 300];
                                r[0] = 1; r[1] = 1; r[2] = 6; r[3] = 0;
                                put32(&mut r, 4, xid);
                                put16(&mut r, 10, 0x8000);
                                i = 0;
                                while i < 6 { r[28 + i] = mac[i]; i += 1; }
                                put32(&mut r, 236, 0x6382_5363);
                                let mut oo = 240;
                                r[oo] = 53; r[oo + 1] = 1; r[oo + 2] = 3; oo += 3;
                                r[oo] = 50; r[oo + 1] = 4; i = 0;
                                while i < 4 { r[oo + 2 + i] = offered[i]; i += 1; }
                                oo += 6;
                                r[oo] = 54; r[oo + 1] = 4; i = 0;
                                while i < 4 { r[oo + 2 + i] = server[i]; i += 1; }
                                oo += 6;
                                r[oo] = 255; oo += 1;
                                let mut uu = [0u8; 320];
                                put16(&mut uu, 0, 68);
                                put16(&mut uu, 2, 67);
                                put16(&mut uu, 4, (8 + oo) as u16);
                                k = 0;
                                while k < oo { uu[8 + k] = r[k]; k += 1; }
                                let mut ip2 = [0u8; 400];
                                let nn = ip_build([0; 4], [255, 255, 255, 255], 17, &uu[..8 + oo], &mut ip2);
                                send_frame([0xff; 6], 0x0800, &ip2[..nn]);
                            } else if stage == 1 && typ == 5 {
                                let c = &mut *core::ptr::addr_of_mut!(CFG);
                                c.ip = offered;
                                c.mask = mask;
                                c.gw = if gw == [0; 4] { [10, 0, 2, 2] } else { gw };
                                c.dns = if dns == [0; 4] { c.gw } else { dns };
                                c.dhcp = true;
                                c.ready = true;
                                net::set_ip(c.ip, c.gw);
                                return true;
                            }
                        }
                    }
                }
            }
        }
        bs_stall(1000);
        t += 1;
    }
    false
}

pub unsafe fn ensure() -> bool {
    let c = &mut *core::ptr::addr_of_mut!(CFG);
    if c.ready { return true; }
    if !net::info().present { return false; }
    if !c.tried {
        c.tried = true;
        if dhcp_try() { return true; }
    }
    c.ip = net::info().ip;
    c.gw = net::info().gw;
    c.ready = true;
    false
}

pub unsafe fn dhcp() -> bool {
    let c = &mut *core::ptr::addr_of_mut!(CFG);
    c.ready = false;
    c.tried = false;
    if dhcp_try() { true } else { false }
}

pub unsafe fn dns_resolve(name: &str) -> Option<[u8; 4]> {
    if let Some(ip) = is_ip(name) { return Some(ip); }
    ensure();
    let c = cfg();
    if c.dns == [0; 4] { return None; }
    let mut q = [0u8; 256];
    let id = (XID & 0xffff) as u16 | 0x0100;
    put16(&mut q, 0, id);
    put16(&mut q, 2, 0x0100);
    put16(&mut q, 4, 1);
    put16(&mut q, 6, 0);
    put16(&mut q, 8, 0);
    put16(&mut q, 10, 0);
    let mut o = 12;
    let b = name.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i <= b.len() {
        if i == b.len() || b[i] == b'.' {
            let l = i - start;
            if l == 0 || l > 63 { return None; }
            q[o] = l as u8; o += 1;
            let mut k = 0;
            while k < l { q[o] = b[start + k]; o += 1; k += 1; }
            start = i + 1;
        }
        i += 1;
    }
    q[o] = 0; o += 1;
    put16(&mut q, o, 1); o += 2;
    put16(&mut q, o, 1); o += 2;
    let _ = udp_send(c.dns, 49153, 53, &q[..o]);
    let mut t = 0u32;
    while t < 1200 {
        let mut b2 = [0u8; 1600];
        if let Some(n) = next_ip(&mut b2) {
            if let Some(u) = parse_udp(&b2[..n]) {
                if u.sport == 53 && u.dport == 49153 && n >= u.off + 12 {
                    let p = &b2[u.off..n];
                    if get16(p, 0) == id {
                        let an = get16(p, 6) as usize;
                        let mut off = 12;
                        // skip question
                        while off < p.len() && p[off] != 0 {
                            off += 1 + p[off] as usize;
                        }
                        off += 5;
                        let mut a = 0;
                        while a < an && off + 12 <= p.len() {
                            if p[off] & 0xc0 == 0xc0 { off += 2; }
                            else { while off < p.len() && p[off] != 0 { off += 1 + p[off] as usize; } off += 1; }
                            if off + 10 > p.len() { break; }
                            let typ = get16(p, off);
                            let rdlen = get16(p, off + 8) as usize;
                            off += 10;
                            if typ == 1 && rdlen == 4 && off + 4 <= p.len() {
                                let mut ip = [0u8; 4];
                                ip.copy_from_slice(&p[off..off + 4]);
                                return Some(ip);
                            }
                            off += rdlen;
                            a += 1;
                        }
                    }
                }
            }
        }
        bs_stall(1000);
        t += 1;
    }
    None
}

unsafe fn tcp_build(dst: [u8; 4], mac: [u8; 6], sport: u16, dport: u16, seq: u32, ack: u32, flags: u8, data: &[u8]) -> bool {
    let mut seg = [0u8; 1460];
    let hl = if flags & 0x02 != 0 { 24 } else { 20 };
    put16(&mut seg, 0, sport);
    put16(&mut seg, 2, dport);
    put32(&mut seg, 4, seq);
    put32(&mut seg, 8, ack);
    seg[12] = (hl / 4) as u8;
    seg[13] = flags;
    put16(&mut seg, 14, 64240);
    put16(&mut seg, 16, 0);
    put16(&mut seg, 18, 0);
    let mut o = 20;
    if hl == 24 {
        seg[20] = 2; seg[21] = 4; put16(&mut seg, 22, 1460); o = 24;
    }
    let mut i = 0;
    while i < data.len() { seg[o + i] = data[i]; i += 1; }
    let seglen = o + data.len();
    let c = cfg();
    let mut ph = [0u8; 12];
    i = 0;
    while i < 4 { ph[i] = c.ip[i]; ph[4 + i] = dst[i]; i += 1; }
    ph[8] = 0; ph[9] = 6;
    put16(&mut ph, 10, seglen as u16);
    let mut sum = 0u32;
    let mut k = 0;
    while k < 12 { sum += get16(&ph, k) as u32; k += 2; }
    let mut j = 0;
    while j + 1 < seglen { sum += get16(&seg, j) as u32; j += 2; }
    if j < seglen { sum += (seg[j] as u32) << 8; }
    while sum >> 16 != 0 { sum = (sum & 0xffff) + (sum >> 16); }
    let ck = !(sum as u16);
    put16(&mut seg, 16, if ck == 0 { 0xffff } else { ck });
    let mut ip = [0u8; 1500];
    let n = ip_build(c.ip, dst, 6, &seg[..seglen], &mut ip);
    send_frame(mac, 0x0800, &ip[..n])
}

unsafe fn tcp_connect(ip: [u8; 4], port: u16) -> bool {
    ensure();
    let c = cfg();
    let nexthop = if ip[0] == c.ip[0] && ip[1] == c.ip[1] && ip[2] == c.ip[2] { ip } else { c.gw };
    let mac = match arp_resolve(nexthop, 900) { Some(m) => m, None => return false };
    let sport = SPORT;
    SPORT = SPORT.wrapping_add(1);
    let t = &mut *core::ptr::addr_of_mut!(TCP);
    t.ip = ip; t.mac = mac; t.sport = sport; t.dport = port;
    t.snd_nxt = 0x2000_0000u32.wrapping_add((sport as u32) << 8);
    t.snd_una = t.snd_nxt;
    t.rcv_nxt = 0;
    t.state = 1;
    if !tcp_build(ip, mac, sport, port, t.snd_nxt, 0, 0x02, &[]) { return false; }
    t.snd_nxt = t.snd_nxt.wrapping_add(1);
    let mut ms = 0u32;
    while ms < 1500 {
        let mut b = [0u8; 1600];
        if let Some(n) = next_ip(&mut b) {
            if n >= 34 && b[9] == 6 {
                let ihl = (b[0] & 0x0f) as usize * 4;
                if n >= ihl + 20 {
                    let sp = get16(&b, ihl);
                    let dp = get16(&b, ihl + 2);
                    if dp == sport && sp == port {
                        let seq = get32(&b, ihl + 4);
                        let ack = get32(&b, ihl + 8);
                        let flags = b[ihl + 13];
                        if flags & 0x12 == 0x12 && ack == t.snd_nxt {
                            t.rcv_nxt = seq.wrapping_add(1);
                            t.snd_una = ack;
                            t.state = 2;
                            let _ = tcp_build(ip, mac, sport, port, t.snd_nxt, t.rcv_nxt, 0x10, &[]);
                            return true;
                        }
                    }
                }
            }
        }
        bs_stall(1000);
        ms += 1;
    }
    false
}

unsafe fn tcp_send_data(data: &[u8]) -> bool {
    let t = &mut *core::ptr::addr_of_mut!(TCP);
    if t.state != 2 { return false; }
    if !tcp_build(t.ip, t.mac, t.sport, t.dport, t.snd_nxt, t.rcv_nxt, 0x18, data) { return false; }
    t.snd_nxt = t.snd_nxt.wrapping_add(data.len() as u32);
    let mut ms = 0u32;
    while ms < 800 {
        let mut b = [0u8; 1600];
        if let Some(n) = next_ip(&mut b) {
            if n >= 34 && b[9] == 6 {
                let ihl = (b[0] & 0x0f) as usize * 4;
                if n >= ihl + 20 && get16(&b, ihl + 2) == t.sport {
                    let ack = get32(&b, ihl + 8);
                    if ack.wrapping_sub(t.snd_una) > 0 && ack.wrapping_sub(t.snd_nxt) <= 0 {
                        t.snd_una = ack;
                        return true;
                    }
                }
            }
        }
        bs_stall(1000);
        ms += 1;
    }
    false
}

unsafe fn tcp_recv(out: &mut [u8], timeout_ms: u32) -> Option<usize> {
    let t = &mut *core::ptr::addr_of_mut!(TCP);
    let mut got = 0usize;
    let mut ms = 0u32;
    while ms < timeout_ms {
        let mut b = [0u8; 1600];
        if let Some(n) = next_ip(&mut b) {
            if n >= 34 && b[9] == 6 {
                let ihl = (b[0] & 0x0f) as usize * 4;
                if n >= ihl + 20 {
                    let sp = get16(&b, ihl);
                    let dp = get16(&b, ihl + 2);
                    if dp == t.sport && sp == t.dport {
                        let seq = get32(&b, ihl + 4);
                        let ack = get32(&b, ihl + 8);
                        let flags = b[ihl + 13];
                        let hl = ((b[ihl + 12] >> 4) as usize) * 4;
                        if flags & 0x10 != 0 { t.snd_una = ack; }
                        let dlen = n.saturating_sub(ihl + hl);
                        if dlen > 0 && seq == t.rcv_nxt {
                            let mut k = 0;
                            while k < dlen && got < out.len() {
                                out[got] = b[ihl + hl + k];
                                got += 1; k += 1;
                            }
                            t.rcv_nxt = t.rcv_nxt.wrapping_add(dlen as u32);
                            let _ = tcp_build(t.ip, t.mac, t.sport, t.dport, t.snd_nxt, t.rcv_nxt, 0x10, &[]);
                        } else if dlen > 0 && seq.wrapping_add(dlen as u32) == t.rcv_nxt {
                            let _ = tcp_build(t.ip, t.mac, t.sport, t.dport, t.snd_nxt, t.rcv_nxt, 0x10, &[]);
                        }
                        if flags & 0x01 != 0 {
                            t.rcv_nxt = t.rcv_nxt.wrapping_add(1);
                            let _ = tcp_build(t.ip, t.mac, t.sport, t.dport, t.snd_nxt, t.rcv_nxt, 0x10, &[]);
                            t.state = 0;
                            return if got > 0 { Some(got) } else { None };
                        }
                    }
                }
            }
        }
        bs_stall(1000);
        ms += 1;
    }
    if got > 0 { Some(got) } else { None }
}

/// 自研 TCP/IP 栈的 HTTP GET（仅明文 http）。
pub unsafe fn http_get(url: &str, out: &mut [u8]) -> Option<usize> {
    let (host, path, port) = parse_url(url)?;
    let ip = dns_resolve(host)?;
    if !ensure() { return None; }
    if !tcp_connect(ip, port) { return None; }
    let mut req = [0u8; 256];
    let mut rl = 0;
    for c in "GET ".bytes() { req[rl] = c; rl += 1; }
    for c in path.bytes() { if rl < 180 { req[rl] = c; rl += 1; } }
    for c in " HTTP/1.0\r\nHost: ".bytes() { req[rl] = c; rl += 1; }
    for c in host.bytes() { if rl < 220 { req[rl] = c; rl += 1; } }
    for c in "\r\nConnection: close\r\n\r\n".bytes() { req[rl] = c; rl += 1; }
    if !tcp_send_data(&req[..rl]) { return None; }

    let buf = &mut *core::ptr::addr_of_mut!(HTTPBUF);
    let mut total = 0usize;
    let mut ms = 0u32;
    while ms < 3000 {
        let mut tmp = [0u8; 1460];
        match tcp_recv(&mut tmp, 300) {
            Some(n) => {
                let mut i = 0;
                while i < n && total < buf.len() { buf[total] = tmp[i]; total += 1; i += 1; }
                if total >= buf.len() { break; }
            }
            None => {
                if TCP.state == 0 { break; }
                ms += 300;
            }
        }
        if TCP.state == 0 { break; }
    }
    let mut hdr_end = 0usize;
    let mut i = 0;
    while i + 3 < total {
        if buf[i] == b'\r' && buf[i + 1] == b'\n' && buf[i + 2] == b'\r' && buf[i + 3] == b'\n' {
            hdr_end = i + 4;
            break;
        }
        i += 1;
    }
    let body = &buf[hdr_end..total];
    let mut n = body.len();
    if n > out.len() { n = out.len(); }
    let mut k = 0;
    while k < n { out[k] = body[k]; k += 1; }
    Some(n)
}
