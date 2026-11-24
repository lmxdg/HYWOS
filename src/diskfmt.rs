#![allow(dead_code)]

use crate::efi::*;
use crate::fs::{BlockIo, BlockIoMedia};
use core::ffi::c_void;

pub const PART_START: u64 = 2048;
const MAX_PART: u64 = 262144;
const FAT_SECS: u32 = 128;
const ROOT_SECS: u32 = 32;
const SPC: u32 = 8;
const ROOT_SEC_START: u32 = 1 + FAT_SECS * 2;
const DATA_START: u32 = ROOT_SEC_START + ROOT_SECS;
const MAX_CLUSTERS: u32 = 32760;
const MIN_CAP_512: u64 = 40_960;

#[repr(C, align(512))]
struct Align512([u8; 512]);

#[repr(C, align(512))]
struct Align4K([u8; 4096]);

#[repr(C, align(512))]
struct Align16K([u8; 16384]);

#[repr(C, align(512))]
struct Align64K([u8; 65536]);

static mut MBR: Align512 = Align512([0; 512]);
static mut BPB: Align512 = Align512([0; 512]);
static mut FATBUF: Align64K = Align64K([0; 65536]);
static mut ROOTBUF: Align16K = Align16K([0; 16384]);
static mut CLUSBUF: Align4K = Align4K([0; 4096]);
static mut IOBLK: Align4K = Align4K([0; 4096]);
static mut CHUNK: Align64K = Align64K([0; 65536]);

pub struct Target {
    pub bio: *mut BlockIo,
    pub blocks: u64,
    pub media_id: u32,
    pub part_secs: u64,
    pub bs: u32,
}

#[derive(Clone, Copy)]
pub struct BlkDiag {
    pub mb: u64,
    pub bs: u32,
    pub ro: u8,
    pub removable: u8,
    pub logical: u8,
    pub present: u8,
}

static mut DIAG: [BlkDiag; 12] = [BlkDiag {
    mb: 0, bs: 0, ro: 0, removable: 0, logical: 0, present: 0,
}; 12];
static mut DIAG_N: usize = 0;

pub unsafe fn diagnostics() -> &'static [BlkDiag] {
    core::slice::from_raw_parts(core::ptr::addr_of!(DIAG) as *const BlkDiag, DIAG_N)
}

unsafe fn dev_write(t: &Target, dev_lba: u64, buf: *mut u8, bytes: usize) -> bool {
    let f: extern "efiapi" fn(*mut BlockIo, u32, u64, usize, *mut u8) -> Status =
        core::mem::transmute((*t.bio).write_blocks);
    f(t.bio, t.media_id, dev_lba, bytes, buf) == EFI_SUCCESS
}

unsafe fn dev_read(t: &Target, dev_lba: u64, buf: *mut u8, bytes: usize) -> bool {
    let f: extern "efiapi" fn(*mut BlockIo, u32, u64, usize, *mut u8) -> Status =
        core::mem::transmute((*t.bio).read_blocks);
    f(t.bio, t.media_id, dev_lba, bytes, buf) == EFI_SUCCESS
}

unsafe fn wr(t: &Target, lba512: u64, buf: *mut u8, bytes: usize) -> bool {
    if t.bs == 512 {
        return dev_write(t, lba512, buf, bytes);
    }
    let per = (t.bs / 512) as u64;
    let io = core::ptr::addr_of_mut!(IOBLK) as *mut u8;
    let mut l = lba512;
    let mut done = 0usize;
    while done < bytes {
        let dev_lba = l / per;
        let in_blk = ((l % per) * 512) as usize;
        let mut n = bytes - done;
        let room = t.bs as usize - in_blk;
        if n > room {
            n = room;
        }
        if in_blk == 0 && n == t.bs as usize {
            if !dev_write(t, dev_lba, buf.add(done), t.bs as usize) {
                return false;
            }
        } else {
            if !dev_read(t, dev_lba, io, t.bs as usize) {
                return false;
            }
            let mut i = 0;
            while i < n {
                *io.add(in_blk + i) = *buf.add(done + i);
                i += 1;
            }
            if !dev_write(t, dev_lba, io, t.bs as usize) {
                return false;
            }
        }
        done += n;
        l += (n / 512) as u64;
    }
    true
}

pub unsafe fn pick_target() -> Option<Target> {
    DIAG_N = 0;
    let mut buf: *mut Handle = core::ptr::null_mut();
    let mut n: usize = 0;
    if !status_ok(bs_locate_handle_buffer(&GUID_BLOCK_IO, &mut n, &mut buf)) || buf.is_null() {
        return None;
    }
    let mut best: Option<Target> = None;
    let mut best_score = 0u64;
    let mut i = 0;
    while i < n {
        let mut b: *mut BlockIo = core::ptr::null_mut();
        if status_ok(bs_handle_protocol(
            *buf.add(i),
            &GUID_BLOCK_IO,
            &mut b as *mut *mut BlockIo as *mut *mut c_void,
        )) && !b.is_null()
        {
            let reset: extern "efiapi" fn(*mut BlockIo, u8) -> Status =
                core::mem::transmute((*b).reset);
            let _ = reset(b, 0);
            let m = (*b).media;
            if !m.is_null() {
                let bs = (*m).block_size;
                let last = (*m).last_block;
                let cap512 = if bs == 0 { 0 } else { last * bs as u64 / 512 };
                let mb = cap512 * 512 / (1024 * 1024);
                if DIAG_N < 12 {
                    DIAG[DIAG_N] = BlkDiag {
                        mb,
                        bs,
                        ro: (*m).read_only,
                        removable: (*m).removable,
                        logical: (*m).logical_partition,
                        present: (*m).present,
                    };
                    DIAG_N += 1;
                }
                let supported = bs == 512 || bs == 4096;
                if (*m).present == 1
                    && (*m).read_only == 0
                    && supported
                    && cap512 >= MIN_CAP_512
                {
                    let score = (if (*m).logical_partition == 0 { 1u64 << 40 } else { 0 })
                        + cap512;
                    if best.is_none() || score > best_score {
                        best_score = score;
                        best = Some(Target {
                            bio: b,
                            blocks: cap512,
                            media_id: (*m).media_id,
                            part_secs: 0,
                            bs,
                        });
                    }
                }
            }
        }
        i += 1;
    }
    let mut t = best?;
    let per = (t.bs / 512) as u64;
    let mut secs = t.blocks.saturating_sub(PART_START);
    if secs > MAX_PART {
        secs = MAX_PART;
    }
    let max_allowed = (MAX_CLUSTERS as u64) * SPC as u64 + DATA_START as u64;
    if secs > max_allowed {
        secs = max_allowed;
    }
    secs -= secs % per;
    if secs < 20_000 {
        return None;
    }
    t.part_secs = secs;
    Some(t)
}

fn put16(buf: *mut u8, off: usize, v: u16) {
    unsafe {
        core::ptr::write_unaligned(buf.add(off) as *mut u16, v);
    }
}

fn put32(buf: *mut u8, off: usize, v: u32) {
    unsafe {
        core::ptr::write_unaligned(buf.add(off) as *mut u32, v);
    }
}

unsafe fn build_mbr(bs: u32, part_secs: u64) {
    let m = core::ptr::addr_of_mut!(MBR) as *mut u8;
    let mut i = 0;
    while i < 512 {
        *m.add(i) = 0;
        i += 1;
    }
    let per = (bs / 512) as u64;
    let e = m.add(446);
    *e = 0x80;
    *e.add(1) = 0xFE;
    *e.add(2) = 0xFF;
    *e.add(3) = 0xFF;
    *e.add(4) = 0x0E;
    *e.add(5) = 0xFE;
    *e.add(6) = 0xFF;
    *e.add(7) = 0xFF;
    put32(e, 8, (PART_START / per) as u32);
    put32(e, 12, (part_secs / per) as u32);
    *m.add(510) = 0x55;
    *m.add(511) = 0xAA;
}

unsafe fn build_bpb(part_secs: u64) {
    let b = core::ptr::addr_of_mut!(BPB) as *mut u8;
    let mut i = 0;
    while i < 512 {
        *b.add(i) = 0;
        i += 1;
    }
    *b = 0xEB;
    *b.add(1) = 0x3C;
    *b.add(2) = 0x90;
    let oem = b"HYWOS   ";
    i = 0;
    while i < 8 {
        *b.add(3 + i) = oem[i];
        i += 1;
    }
    put16(b, 0x0B, 512);
    *b.add(0x0D) = SPC as u8;
    put16(b, 0x0E, 1);
    *b.add(0x10) = 2;
    put16(b, 0x11, 512);
    put16(b, 0x13, if part_secs < 65536 { part_secs as u16 } else { 0 });
    *b.add(0x15) = 0xF8;
    put16(b, 0x16, FAT_SECS as u16);
    put16(b, 0x18, 63);
    put16(b, 0x1A, 255);
    put32(b, 0x1C, PART_START as u32);
    put32(b, 0x20, part_secs as u32);
    *b.add(0x24) = 0x80;
    *b.add(0x26) = 0x29;
    put32(b, 0x27, 0x4859_5753);
    let label = b"HYWOS ESP  ";
    i = 0;
    while i < 11 {
        *b.add(0x2B + i) = label[i];
        i += 1;
    }
    let fs = b"FAT16   ";
    i = 0;
    while i < 8 {
        *b.add(0x36 + i) = fs[i];
        i += 1;
    }
    *b.add(510) = 0x55;
    *b.add(511) = 0xAA;
}

unsafe fn clear(buf: *mut u8, n: usize) {
    let mut i = 0;
    while i < n {
        *buf.add(i) = 0;
        i += 1;
    }
}

unsafe fn dir_item(base: *mut u8, idx: usize, name: &[u8; 11], attr: u8, cluster: u16, size: u32) {
    let p = base.add(idx * 32);
    let mut i = 0;
    while i < 11 {
        *p.add(i) = name[i];
        i += 1;
    }
    *p.add(11) = attr;
    *p.add(12) = 0;
    put16(p, 14, 0);
    put16(p, 16, 0x4A21);
    put16(p, 18, 0x4A21);
    put16(p, 20, 0);
    put16(p, 22, 0);
    put16(p, 24, 0x4A21);
    put16(p, 26, cluster);
    put32(p, 28, size);
}

unsafe fn set_fat(cluster: u16, val: u16) {
    let f = core::ptr::addr_of_mut!(FATBUF) as *mut u8;
    put16(f, cluster as usize * 2, val);
}

pub type ProgFn = unsafe fn(u32);

unsafe fn noop_prog(_p: u32) {}

unsafe fn format(t: &Target, clusters_needed: u32, cb: ProgFn) -> bool {
    build_mbr(t.bs, t.part_secs);
    build_bpb(t.part_secs);
    let mbr = core::ptr::addr_of_mut!(MBR) as *mut u8;
    if !wr(t, 0, mbr, 512) {
        return false;
    }
    let bpb = core::ptr::addr_of_mut!(BPB) as *mut u8;
    if !wr(t, PART_START, bpb, 512) {
        return false;
    }
    cb(6);
    let fat = core::ptr::addr_of_mut!(FATBUF) as *mut u8;
    clear(fat, 65536);
    put16(fat, 0, 0xFFF8);
    put16(fat, 2, 0xFFFF);
    put16(fat, 4, 0xFFFF);
    put16(fat, 6, 0xFFFF);
    let mut c = 4u16;
    let last = 3 + clusters_needed as u16;
    while c < last {
        put16(fat, c as usize * 2, c + 1);
        c += 1;
    }
    put16(fat, last as usize * 2, 0xFFFF);
    if !wr(t, PART_START + 1, fat, 65536) {
        return false;
    }
    if !wr(t, PART_START + 1 + FAT_SECS as u64, fat, 65536) {
        return false;
    }
    cb(18);
    let root = core::ptr::addr_of_mut!(ROOTBUF) as *mut u8;
    clear(root, 16384);
    dir_item(root, 0, b"EFI        ", 0x10, 2, 0);
    if !wr(t, PART_START + ROOT_SEC_START as u64, root, 16384) {
        return false;
    }
    cb(26);
    let clus = core::ptr::addr_of_mut!(CLUSBUF) as *mut u8;
    clear(clus, 4096);
    dir_item(clus, 0, b".          ", 0x10, 2, 0);
    dir_item(clus, 1, b"..         ", 0x10, 0, 0);
    dir_item(clus, 2, b"BOOT       ", 0x10, 3, 0);
    if !wr(t, PART_START + DATA_START as u64, clus, 4096) {
        return false;
    }
    true
}

pub unsafe fn install(t: &Target, image: *const u8, size: usize) -> bool {
    let clusters_needed = (size + 4095) / 4096;
    if clusters_needed + 4 > 32760 {
        return false;
    }
    if !format(t, clusters_needed as u32, noop_prog) {
        return false;
    }
    let clus = core::ptr::addr_of_mut!(CLUSBUF) as *mut u8;
    clear(clus, 4096);
    dir_item(clus, 0, b".          ", 0x10, 3, 0);
    dir_item(clus, 1, b"..         ", 0x10, 2, 0);
    dir_item(clus, 2, b"BOOTX64 EFI", 0x20, 4, size as u32);
    if !wr(t, PART_START + (DATA_START + SPC) as u64, clus, 4096) {
        return false;
    }
    let mut off = 0usize;
    let mut cluster = 4u64;
    while off < size {
        let n = if size - off < 4096 { size - off } else { 4096 };
        clear(clus, 4096);
        let src = image.add(off);
        let mut i = 0;
        while i < n {
            *clus.add(i) = *src.add(i);
            i += 1;
        }
        if !wr(t, PART_START + DATA_START as u64 + (cluster - 2) * SPC as u64, clus, 4096) {
            return false;
        }
        off += n;
        cluster += 1;
    }
    let fl: extern "efiapi" fn(*mut BlockIo) -> Status = core::mem::transmute((*t.bio).flush);
    fl(t.bio);
    true
}

pub unsafe fn install_file_p(t: &Target, f: *mut crate::fs::FileProtocol, size: usize, cb: ProgFn) -> bool {
    let clusters_needed = (size + 4095) / 4096;
    if clusters_needed + 4 > 32760 {
        return false;
    }
    if !format(t, clusters_needed as u32, cb) {
        return false;
    }
    let clus = core::ptr::addr_of_mut!(CLUSBUF) as *mut u8;
    clear(clus, 4096);
    dir_item(clus, 0, b".          ", 0x10, 3, 0);
    dir_item(clus, 1, b"..         ", 0x10, 2, 0);
    dir_item(clus, 2, b"BOOTX64 EFI", 0x20, 4, size as u32);
    if !wr(t, PART_START + (DATA_START + SPC) as u64, clus, 4096) {
        return false;
    }
    cb(32);
    let chunk = core::ptr::addr_of_mut!(CHUNK) as *mut u8;
    let mut off = 0usize;
    let mut cluster = 4u64;
    while off < size {
        let n = if size - off < 65536 { size - off } else { 65536 };
        let got = match crate::fs::read_at(f, off as u64, core::slice::from_raw_parts_mut(chunk, n)) {
            Some(g) => g,
            None => return false,
        };
        let mut co = 0usize;
        while co < got {
            let m = if got - co < 4096 { got - co } else { 4096 };
            clear(clus, 4096);
            let mut i = 0;
            while i < m {
                *clus.add(i) = *chunk.add(co + i);
                i += 1;
            }
            if !wr(t, PART_START + DATA_START as u64 + (cluster - 2) * SPC as u64, clus, 4096) {
                return false;
            }
            co += 4096;
            cluster += 1;
        }
        off += got;
        let pct = 32 + (off * 62 / if size == 0 { 1 } else { size }) as u32;
        cb(if pct > 94 { 94 } else { pct });
    }
    cb(100);
    let fl: extern "efiapi" fn(*mut BlockIo) -> Status = core::mem::transmute((*t.bio).flush);
    fl(t.bio);
    true
}

pub unsafe fn install_file(t: &Target, f: *mut crate::fs::FileProtocol, size: usize) -> bool {
    install_file_p(t, f, size, noop_prog)
}

pub unsafe fn target_info(t: &Target) -> (u64, u64) {
    (t.blocks * 512 / (1024 * 1024), t.part_secs * 512 / (1024 * 1024))
}