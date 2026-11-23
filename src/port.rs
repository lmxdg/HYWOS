#![allow(dead_code)]

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn inb(port: u16) -> u8 {
    let v: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") v);
    v
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn outb(port: u16, val: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") val);
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn inw(port: u16) -> u16 {
    let v: u16;
    core::arch::asm!("in ax, dx", in("dx") port, out("ax") v);
    v
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn outw(port: u16, val: u16) {
    core::arch::asm!("out dx, ax", in("dx") port, in("ax") val);
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn inl(port: u16) -> u32 {
    let v: u32;
    core::arch::asm!("in eax, dx", in("dx") port, out("eax") v);
    v
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn outl(port: u16, val: u32) {
    core::arch::asm!("out dx, eax", in("dx") port, in("eax") val);
}

#[cfg(target_arch = "x86_64")]
#[inline(always)]
pub unsafe fn io_delay() {
    outb(0x80, 0);
}

pub const CFG_ADDR: u16 = 0xCF8;
pub const CFG_DATA: u16 = 0xCFC;

#[derive(Clone, Copy)]
pub struct Dev {
    pub bus: u8,
    pub dev: u8,
    pub func: u8,
    pub vendor: u16,
    pub device: u16,
    pub class: u8,
    pub subclass: u8,
}

pub unsafe fn cfg32(bus: u8, dev: u8, func: u8, off: u8) -> u32 {
    let addr = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    outl(CFG_ADDR, addr);
    inl(CFG_DATA)
}

pub unsafe fn cfg_write32(bus: u8, dev: u8, func: u8, off: u8, val: u32) {
    let addr = 0x8000_0000u32
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((off as u32) & 0xFC);
    outl(CFG_ADDR, addr);
    outl(CFG_DATA, val);
}

pub unsafe fn find(vendor: u16, device: u16) -> Option<Dev> {
    let mut bus = 0u8;
    while bus < 8 {
        let mut d = 0u8;
        while d < 32 {
            let mut f = 0u8;
            while f < 8 {
                let v = cfg32(bus, d, f, 0x00);
                let vid = (v & 0xffff) as u16;
                let did = (v >> 16) as u16;
                if vid == 0xffff {
                    if f == 0 { break; }
                } else if vid == vendor && did == device {
                    let c = cfg32(bus, d, f, 0x08);
                    return Some(Dev {
                        bus,
                        dev: d,
                        func: f,
                        vendor: vid,
                        device: did,
                        class: (c >> 24) as u8,
                        subclass: (c >> 16) as u8,
                    });
                }
                f += 1;
            }
            d += 1;
        }
        bus += 1;
    }
    None
}

pub unsafe fn find_class(class: u8, subclass: u8) -> Option<Dev> {
    let mut bus = 0u8;
    while bus < 8 {
        let mut d = 0u8;
        while d < 32 {
            let mut f = 0u8;
            while f < 8 {
                let v = cfg32(bus, d, f, 0x00);
                let vid = (v & 0xffff) as u16;
                if vid == 0xffff {
                    if f == 0 { break; }
                } else {
                    let c = cfg32(bus, d, f, 0x08);
                    if (c >> 24) as u8 == class && (c >> 16) as u8 == subclass {
                        return Some(Dev {
                            bus,
                            dev: d,
                            func: f,
                            vendor: vid,
                            device: (v >> 16) as u16,
                            class,
                            subclass,
                        });
                    }
                }
                f += 1;
            }
            d += 1;
        }
        bus += 1;
    }
    None
}

pub unsafe fn bar(d: &Dev, idx: u8) -> (bool, u64) {
    let off = 0x10 + idx * 4;
    let v = cfg32(d.bus, d.dev, d.func, off);
    if v & 1 == 1 {
        (true, (v & !0x3) as u64)
    } else {
        let typ = (v >> 1) & 0x3;
        if typ == 0x2 && idx < 5 {
            let hi = cfg32(d.bus, d.dev, d.func, off + 4);
            (false, ((v & !0xF) as u64) | ((hi as u64) << 32))
        } else {
            (false, (v & !0xF) as u64)
        }
    }
}

pub unsafe fn enable_bus_master(d: &Dev) {
    let cmd = cfg32(d.bus, d.dev, d.func, 0x04) & 0xffff;
    let new = cmd | 0x7;
    let hi = cfg32(d.bus, d.dev, d.func, 0x04) & 0xffff_0000;
    cfg_write32(d.bus, d.dev, d.func, 0x04, hi | new);
}

pub unsafe fn subsystem_id(d: &Dev) -> u16 {
    (cfg32(d.bus, d.dev, d.func, 0x2C) >> 16) as u16
}