#![allow(dead_code)]

use crate::efi::*;
use core::arch::{asm, global_asm};

pub const USER_BASE: u64 = 0x0000_4000_0000_0000;
pub const USER_STACK: u64 = USER_BASE + 0x1000;

#[repr(C, align(4096))]
struct Page([u8; 4096]);

static mut KPML4: Page = Page([0; 4096]);
static mut KPDPT: Page = Page([0; 4096]);
static mut KPD0: Page = Page([0; 4096]);
static mut KPD1: Page = Page([0; 4096]);
static mut KPD2: Page = Page([0; 4096]);
static mut KPD3: Page = Page([0; 4096]);
static mut UPML4: Page = Page([0; 4096]);
static mut UPDPT: Page = Page([0; 4096]);
static mut UPD: Page = Page([0; 4096]);
static mut UPT: Page = Page([0; 4096]);

#[repr(C, align(4096))]
struct CodePage([u8; 4096]);
static mut UCODE: CodePage = CodePage([0; 4096]);
static mut USTACK: CodePage = CodePage([0; 4096]);

#[repr(C, align(16))]
struct KStack([u8; 16384]);
static mut KSTACK: KStack = KStack([0; 16384]);

#[repr(C, packed)]
struct Tss {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    reserved3: u16,
    iomap: u16,
}
static mut TSS: Tss = Tss {
    reserved0: 0, rsp0: 0, rsp1: 0, rsp2: 0, reserved1: 0, ist: [0; 7],
    reserved2: 0, reserved3: 0, iomap: 104,
};

#[repr(C, align(16))]
struct B1024([u8; 1024]);
#[repr(C, align(16))]
struct B4096([u8; 4096]);
static mut GDT_BUF: B1024 = B1024([0; 1024]);
static mut GDT_LIMIT: u16 = 0;
static mut GDT_BASE_OLD: u64 = 0;
static mut GDTR_OLD: [u8; 10] = [0; 10];
static mut GDTR_NEW: [u8; 10] = [0; 10];
static mut IDT_BUF: B4096 = B4096([0; 4096]);
static mut IDTR_OLD: [u8; 10] = [0; 10];
static mut IDTR_NEW: [u8; 10] = [0; 10];
static mut UCODE_SEL: u16 = 0;
static mut UDATA_SEL: u16 = 0;
static mut TSS_SEL: u16 = 0;
static mut LTR_SEL: u16 = 0;
static mut KCODE_SEL: u16 = 0;
static mut KDATA_SEL: u16 = 0;
static mut KDATA_MEM: u16 = 0;
static mut INITED: bool = false;

static mut KCR3: u64 = 0;
static mut KCR4: u64 = 0;
static mut KFLAGS: u64 = 0;
static mut IN_USER: bool = false;
static mut FAULT: bool = false;
static mut EXIT_CALLED: bool = false;
static mut RESULT: [u8; 768] = [0; 768];
static mut RESULT_LEN: usize = 0;
static mut CR3_USER: u64 = 0;

fn push(s: &str) {
    unsafe {
        let r = &mut *core::ptr::addr_of_mut!(RESULT);
        for c in s.bytes() {
            if RESULT_LEN < r.len() - 1 {
                r[RESULT_LEN] = c;
                RESULT_LEN += 1;
            }
        }
    }
}

fn push_num(v: u64) {
    let mut b = [0u8; 20];
    let mut n = 0usize;
    let mut x = v;
    if x == 0 { push("0"); return; }
    while x > 0 { b[n] = b'0' + (x % 10) as u8; x /= 10; n += 1; }
    let mut out = [0u8; 20];
    let mut i = 0;
    while i < n { out[i] = b[n - 1 - i]; i += 1; }
    push(unsafe { core::str::from_utf8_unchecked(&out[..n]) });
}

fn push_hex(v: u64) {
    let hex = b"0123456789abcdef";
    push("0x");
    let mut started = false;
    let mut i = 15i32;
    while i >= 0 {
        let d = ((v >> (i * 4)) & 0xf) as usize;
        if d != 0 || started || i == 0 {
            started = true;
            let c = [hex[d]];
            push(unsafe { core::str::from_utf8_unchecked(&c) });
        }
        i -= 1;
    }
}

unsafe fn cr3() -> u64 {
    let v: u64;
    asm!("mov {}, cr3", out(reg) v, options(nomem, nostack));
    v
}

unsafe fn cr4() -> u64 {
    let v: u64;
    asm!("mov {}, cr4", out(reg) v, options(nomem, nostack));
    v
}

unsafe fn set_cr3(v: u64) {
    asm!("mov cr3, {}", in(reg) v, options(nostack));
}

unsafe fn set_cr4(v: u64) {
    asm!("mov cr4, {}", in(reg) v, options(nostack));
}

unsafe fn rflags() -> u64 {
    let v: u64;
    asm!("pushfq", "pop {}", out(reg) v, options(nomem));
    v
}

fn rd_cr2() -> u64 {
    unsafe {
        let v: u64;
        asm!("mov {}, cr2", out(reg) v, options(nomem, nostack));
        v
    }
}

fn desc(buf: &mut [u8], idx: usize, base: u64, limit: u32, access: u8, flags: u8) {
    let o = idx * 8;
    buf[o] = limit as u8;
    buf[o + 1] = (limit >> 8) as u8;
    buf[o + 2] = base as u8;
    buf[o + 3] = (base >> 8) as u8;
    buf[o + 4] = (base >> 16) as u8;
    buf[o + 5] = access;
    buf[o + 6] = ((limit >> 16) as u8 & 0x0f) | (flags & 0xf0);
    buf[o + 7] = (base >> 24) as u8;
}

fn tss_desc(buf: &mut [u8], idx: usize, base: u64, limit: u32) {
    desc(buf, idx, base, limit, 0x89, 0x00);
    let o = idx * 8;
    buf[o + 8] = (base >> 32) as u8;
    buf[o + 9] = (base >> 40) as u8;
    buf[o + 10] = (base >> 48) as u8;
    buf[o + 11] = (base >> 56) as u8;
}

fn gate(idt: &mut [u8], vec: usize, handler: u64, sel: u16, attr: u8) {
    let o = vec * 16;
    idt[o] = handler as u8;
    idt[o + 1] = (handler >> 8) as u8;
    idt[o + 2] = sel as u8;
    idt[o + 3] = (sel >> 8) as u8;
    idt[o + 4] = 0;
    idt[o + 5] = attr;
    idt[o + 6] = (handler >> 16) as u8;
    idt[o + 7] = (handler >> 24) as u8;
    idt[o + 8] = (handler >> 32) as u8;
    idt[o + 9] = (handler >> 40) as u8;
    idt[o + 10] = (handler >> 48) as u8;
    idt[o + 11] = (handler >> 56) as u8;
    idt[o + 12] = 0;
    idt[o + 13] = 0;
    idt[o + 14] = 0;
    idt[o + 15] = 0;
}

unsafe fn pml4e(tbl: *mut u8, idx: usize, v: u64) {
    core::ptr::write_volatile(tbl.add(idx * 8) as *mut u64, v);
}

unsafe fn build_pages() {
    let kp = core::ptr::addr_of_mut!(KPML4) as *mut u8;
    let pdpt = core::ptr::addr_of_mut!(KPDPT) as *mut u8;
    let pds = [
        core::ptr::addr_of_mut!(KPD0) as *mut u8,
        core::ptr::addr_of_mut!(KPD1) as *mut u8,
        core::ptr::addr_of_mut!(KPD2) as *mut u8,
        core::ptr::addr_of_mut!(KPD3) as *mut u8,
    ];
    pml4e(kp, 0, (pdpt as u64) | 0x3);
    let mut i = 0;
    while i < 4 {
        pml4e(pdpt, i, (pds[i] as u64) | 0x3);
        let mut j = 0;
        while j < 512 {
            let addr = (i as u64) * 0x4000_0000 + (j as u64) * 0x20_0000;
            pml4e(pds[i], j, addr | 0x83);
            j += 1;
        }
        i += 1;
    }
    let up = core::ptr::addr_of_mut!(UPML4) as *mut u8;
    let mut k = 0;
    while k < 512 {
        let v = core::ptr::read_volatile(kp.add(k * 8) as *const u64);
        pml4e(up, k, v);
        k += 1;
    }
    let updpt = core::ptr::addr_of_mut!(UPDPT) as *mut u8;
    let upd = core::ptr::addr_of_mut!(UPD) as *mut u8;
    let upt = core::ptr::addr_of_mut!(UPT) as *mut u8;
    pml4e(up, 128, (updpt as u64) | 0x7);
    pml4e(updpt, 0, (upd as u64) | 0x7);
    pml4e(upd, 0, (upt as u64) | 0x7);
    let code = core::ptr::addr_of!(UCODE) as u64;
    let stack = core::ptr::addr_of!(USTACK) as u64;
    pml4e(upt, 0, code | 0x7);
    pml4e(upt, 1, stack | 0x7);
}

unsafe fn init_once() -> bool {
    if INITED { return true; }
    // 复制固件 GDT 并追加用户段/TSS
    asm!("sgdt [{}]", in(reg) core::ptr::addr_of_mut!(GDTR_OLD) as *mut u8, options(nostack));
    let old = &*core::ptr::addr_of!(GDTR_OLD);
    let limit = u16::from_le_bytes([old[0], old[1]]) as usize;
    GDT_BASE_OLD = u64::from_le_bytes([old[2], old[3], old[4], old[5], old[6], old[7], old[8], old[9]]);
    let copy = if limit + 1 > 800 { 800 } else { limit + 1 };
    let g = &mut (*core::ptr::addr_of_mut!(GDT_BUF)).0;
    let mut i = 0;
    while i < copy { g[i] = *((GDT_BASE_OLD as *const u8).add(i)); i += 1; }
    let mut idx = (limit + 1 + 7) / 8;
    if idx < 8 { idx = 8; }
    if idx + 4 > 120 { return false; }
    UCODE_SEL = ((idx as u16) << 3) | 3;
    UDATA_SEL = (((idx + 1) as u16) << 3) | 3;
    TSS_SEL = ((idx + 2) as u16) << 3;
    KDATA_SEL = ((idx + 4) as u16) << 3;
    KCODE_SEL = ((idx + 5) as u16) << 3;
    desc(g, idx, 0, 0xFFFFF, 0xFA, 0xA0);
    desc(g, idx + 1, 0, 0xFFFFF, 0xF2, 0xC0);
    TSS.rsp0 = (core::ptr::addr_of!(KSTACK) as u64) + 16384 - 16;
    tss_desc(g, idx + 2, core::ptr::addr_of!(TSS) as u64, 0x67);
    desc(g, idx + 4, 0, 0xFFFFF, 0x92, 0xC0);
    desc(g, idx + 5, 0, 0xFFFFF, 0x9A, 0xA0);
    let nlimit = ((idx + 6) * 8 - 1) as u16;
    GDT_LIMIT = nlimit;
    let gn = &mut *core::ptr::addr_of_mut!(GDTR_NEW);
    gn[0] = nlimit as u8;
    gn[1] = (nlimit >> 8) as u8;
    let gb = core::ptr::addr_of!(GDT_BUF) as u64;
    let mut b = gb.to_le_bytes();
    let mut k = 0;
    while k < 8 { gn[2 + k] = b[k]; k += 1; }
    asm!("lgdt [{}]", in(reg) gn.as_ptr(), options(nostack));

    // 复制固件 IDT 并安装 syscall/异常门
    asm!("sidt [{}]", in(reg) core::ptr::addr_of_mut!(IDTR_OLD) as *mut u8, options(nostack));
    let oi = &*core::ptr::addr_of!(IDTR_OLD);
    let ilim = u16::from_le_bytes([oi[0], oi[1]]) as usize;
    let ibase = u64::from_le_bytes([oi[2], oi[3], oi[4], oi[5], oi[6], oi[7], oi[8], oi[9]]);
    let icopy = if ilim + 1 > 4096 { 4096 } else { ilim + 1 };
    let idt = &mut (*core::ptr::addr_of_mut!(IDT_BUF)).0;
    i = 0;
    while i < icopy { idt[i] = *((ibase as *const u8).add(i)); i += 1; }
    let newlim = if ilim < 0x80F { 0x80F } else { ilim };
    gate(idt, 0x80, ring3_isr_syscall as *const () as u64, KCODE_SEL, 0xEE);
    gate(idt, 14, ring3_isr_pf as *const () as u64, KCODE_SEL, 0x8E);
    gate(idt, 13, ring3_isr_gp as *const () as u64, KCODE_SEL, 0x8E);
    gate(idt, 6, ring3_isr_ud as *const () as u64, KCODE_SEL, 0x8E);
    gate(idt, 8, ring3_isr_df as *const () as u64, KCODE_SEL, 0x8E);
    let inew = &mut *core::ptr::addr_of_mut!(IDTR_NEW);
    inew[0] = newlim as u8;
    inew[1] = (newlim >> 8) as u8;
    let ib = (core::ptr::addr_of!(IDT_BUF) as u64).to_le_bytes();
    k = 0;
    while k < 8 { inew[2 + k] = ib[k]; k += 1; }
    asm!("lidt [{}]", in(reg) inew.as_ptr(), options(nostack));

    LTR_SEL = TSS_SEL;
    asm!("ltr [{0}]", in(reg) core::ptr::addr_of!(LTR_SEL), options(nostack));

    build_pages();

    INITED = true;
    true
}

fn emit(code: &mut [u8], o: &mut usize, bytes: &[u8]) {
    for b in bytes { code[*o] = *b; *o += 1; }
}

unsafe fn build_user(fault: bool) {
    let page = &mut *core::ptr::addr_of_mut!(UCODE);
    let code = &mut page.0;
    let mut i = 0;
    while i < code.len() { code[i] = 0; i += 1; }
    let msg = if fault { b"Ring3 isolation test".as_slice() } else { b"Hello from Ring3 via int 0x80".as_slice() };
    let moff = 0x100usize;
    let mut k = 0;
    while k < msg.len() { code[moff + k] = msg[k]; k += 1; }
    let maddr = USER_BASE + moff as u64;
    let mut o = 0usize;
    emit(code, &mut o, &[0x48, 0xC7, 0xC0, 0x01, 0x00, 0x00, 0x00]); // mov rax,1
    emit(code, &mut o, &[0x48, 0xBF]);
    emit(code, &mut o, &maddr.to_le_bytes()); // mov rdi,msg
    emit(code, &mut o, &[0x48, 0xC7, 0xC6]);
    emit(code, &mut o, &(msg.len() as u32).to_le_bytes()); // mov rsi,len
    emit(code, &mut o, &[0xCD, 0x80]); // int 0x80
    if fault {
        emit(code, &mut o, &[0x48, 0xA1, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]); // mov rax,[0x1000]
    }
    emit(code, &mut o, &[0x48, 0xC7, 0xC0, 0x02, 0x00, 0x00, 0x00]); // mov rax,2
    emit(code, &mut o, &[0xCD, 0x80]);
    emit(code, &mut o, &[0xF4]);
}

#[no_mangle]
pub extern "C" fn ring3_syscall_dispatch() {
    unsafe {
        let num = core::ptr::read_volatile(core::ptr::addr_of!(ring3_s_rax));
        let a1 = core::ptr::read_volatile(core::ptr::addr_of!(ring3_s_rdi));
        let a2 = core::ptr::read_volatile(core::ptr::addr_of!(ring3_s_rsi));
        let r = syscall_impl(num, a1, a2);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(ring3_s_rax), r);
    }
}

#[no_mangle]
pub extern "C" fn ring3_exception_dispatch() {
    unsafe {
        let v = core::ptr::read_volatile(core::ptr::addr_of!(ring3_exc_vec));
        exception_impl(v);
    }
}

fn syscall_impl(num: u64, a1: u64, a2: u64) -> u64 {
    unsafe {
        match num {
            1 => {
                if a1 >= USER_BASE && a1 < USER_BASE + 0x2000 && a2 <= 256 {
                    let p = a1 as *const u8;
                    let mut b = [0u8; 256];
                    let mut i = 0;
                    while i < a2 as usize { b[i] = *p.add(i); i += 1; }
                    if let Ok(s) = core::str::from_utf8(&b[..a2 as usize]) { push(s); }
                }
                0
            }
            2 => {
                EXIT_CALLED = true;
                push("\n[ring3] exit syscall, return to kernel\n");
                leave_user();
            }
            3 => crate::hw::uptime_secs() as u64,

            _ => 0xFFFF_FFFF,
        }
    }
}

unsafe fn exception_impl(vec: u64) {
    {
        if !IN_USER {
            serial_print("[ring3] kernel exception");
            serial_u32(vec as u32);
            serial_print("[ring3] halted");
            loop { asm!("cli", "hlt"); }
        }
        FAULT = true;

        push("\n[ring3] exception vector ");
        push_num(vec);
        if vec == 14 {
            let a = rd_cr2();
            push(" at ");
            push_hex(a);
            push("：用户态无法访问内核页\n");
        } else {
            push("\n");
        }
        leave_user();
    }
}

unsafe fn leave_user() -> ! {
    let kc = KCR3;
    let kf = KFLAGS;
    let k4 = KCR4;
    KDATA_MEM = KDATA_SEL;
    let ks = core::ptr::read_volatile(core::ptr::addr_of!(ring3_ksp));
    let sv = core::ptr::addr_of!(ring3_save) as usize;
    let kd = core::ptr::addr_of!(KDATA_MEM) as usize;
    IN_USER = false;
    asm!(
        "mov cr4, {c4}",
        "mov cr3, {c3}",
        "mov rsp, {sp}",
        "mov ds, [{kd}]",
        "mov es, [{kd}]",
        "mov ss, [{kd}]",
        "mov rbx, [{sv}]",
        "mov rbp, [{sv} + 8]",
        "mov r12, [{sv} + 16]",
        "mov r13, [{sv} + 24]",
        "mov r14, [{sv} + 32]",
        "mov r15, [{sv} + 40]",
        "push {fl}",
        "popfq",
        "ret",
        c4 = in(reg) k4, c3 = in(reg) kc, sp = in(reg) ks, fl = in(reg) kf,
        kd = in(reg) kd, sv = in(reg) sv,
        options(noreturn)
    );
}

/// 运行用户态程序。fault=true 时额外执行一次内核地址读取，用于验证页表隔离。
pub unsafe fn run(fault: bool) -> &'static str {
    if !init_once() {
        push("[ring3] 初始化失败\n");
        return core::str::from_utf8_unchecked(&(&*core::ptr::addr_of!(RESULT))[..RESULT_LEN]);
    }
    build_user(fault);
    RESULT_LEN = 0;
    FAULT = false;
    EXIT_CALLED = false;
    let kp = core::ptr::addr_of!(KPML4) as u64;
    let up = core::ptr::addr_of!(UPML4) as u64;
    KCR3 = cr3();
    KCR4 = cr4();
    KFLAGS = rflags();
    CR3_USER = up;
    push("[ring3] 内核页表 CR3=");
    push_hex(KCR3);
    push("  用户页表 CR3=");
    push_hex(up);
    push("\n");
    push("[ring3] 进入 CPL3，CS=");
    push_hex(UCODE_SEL as u64);
    push("  SS=");
    push_hex(UDATA_SEL as u64);
    push("\n");
    let _ = kp;
    IN_USER = true;
    let user_rsp = USER_STACK + 0x1000 - 16;
    let uc = UCODE_SEL as u64;
    let ud = UDATA_SEL as u64;
    asm!("mov cr4, {}", in(reg) (KCR4 & !((1u64 << 20) | (1u64 << 21))), options(nostack));
    core::ptr::write_volatile(core::ptr::addr_of_mut!(ring3_target), up);
    core::ptr::write_volatile(core::ptr::addr_of_mut!(ring3_v_rip), USER_BASE);
    core::ptr::write_volatile(core::ptr::addr_of_mut!(ring3_v_rsp), user_rsp);
    core::ptr::write_volatile(core::ptr::addr_of_mut!(ring3_v_cs), uc);
    core::ptr::write_volatile(core::ptr::addr_of_mut!(ring3_v_ss), ud);
    core::ptr::write_volatile(core::ptr::addr_of_mut!(ring3_v_fl), 0x2);
    ring3_enter();

    // 用户态退出后从这里继续（leave_user 的 ret 返回）。
    push("\n[ring3] 已返回内核，CR3=");
    push_hex(cr3());
    push("\n");
    if FAULT {
        push("[ring3] 结果：用户态访问内核页被 #PF 拦截，隔离生效\n");
    } else if EXIT_CALLED {
        push("[ring3] 结果：syscall 输出与 exit 均正常\n");
    }
    core::str::from_utf8_unchecked(&(&*core::ptr::addr_of!(RESULT))[..RESULT_LEN])
}

global_asm!(
    ".section .data",
    ".global ring3_ksp",
    ".align 8",
    "ring3_ksp: .quad 0",
    ".global ring3_save",
    "ring3_save: .quad 0, 0, 0, 0, 0, 0",
    ".global ring3_target",
    "ring3_target: .quad 0",
    ".global ring3_v_rip",
    "ring3_v_rip: .quad 0",
    ".global ring3_v_rsp",
    "ring3_v_rsp: .quad 0",
    ".global ring3_v_cs",
    "ring3_v_cs: .quad 0",
    ".global ring3_v_ss",
    "ring3_v_ss: .quad 0",
    ".global ring3_v_fl",
    "ring3_v_fl: .quad 0",
    ".global ring3_s_rax",
    "ring3_s_rax: .quad 0",
    ".global ring3_s_rdi",
    "ring3_s_rdi: .quad 0",
    ".global ring3_s_rsi",
    "ring3_s_rsi: .quad 0",
    ".global ring3_exc_vec",
    "ring3_exc_vec: .quad 0",
    ".section .text",
    ".global ring3_enter",
    "ring3_enter:",
    "mov [rip + ring3_ksp], rsp",
    "mov [rip + ring3_save], rbx",
    "mov [rip + ring3_save + 8], rbp",
    "mov [rip + ring3_save + 16], r12",
    "mov [rip + ring3_save + 24], r13",
    "mov [rip + ring3_save + 32], r14",
    "mov [rip + ring3_save + 40], r15",
    "mov rax, [rip + ring3_target]",
    "mov cr3, rax",
    "mov rax, [rip + ring3_v_ss]",
    "mov ds, ax",
    "mov es, ax",
    "push rax",
    "mov rax, [rip + ring3_v_rsp]",
    "push rax",
    "mov rax, [rip + ring3_v_fl]",
    "push rax",
    "mov rax, [rip + ring3_v_cs]",
    "push rax",
    "mov rax, [rip + ring3_v_rip]",
    "push rax",
    "iretq",
    ".global ring3_isr_syscall",
    "ring3_isr_syscall:",
    "mov [rip + ring3_s_rax], rax",
    "mov [rip + ring3_s_rdi], rdi",
    "mov [rip + ring3_s_rsi], rsi",
    "call ring3_syscall_dispatch",
    "mov rax, [rip + ring3_s_rax]",
    "iretq",
    ".global ring3_isr_pf",
    "ring3_isr_pf:",
    "mov qword ptr [rip + ring3_exc_vec], 14",
    "call ring3_exception_dispatch",
    "hlt",
    ".Lpf_halt:",
    "jmp .Lpf_halt",
    ".global ring3_isr_gp",
    "ring3_isr_gp:",
    "mov qword ptr [rip + ring3_exc_vec], 13",
    "call ring3_exception_dispatch",
    "hlt",
    ".Lgp_halt:",
    "jmp .Lgp_halt",
    ".global ring3_isr_ud",
    "ring3_isr_ud:",
    "mov qword ptr [rip + ring3_exc_vec], 6",
    "call ring3_exception_dispatch",
    "hlt",
    ".Lud_halt:",
    "jmp .Lud_halt",
    ".global ring3_isr_df",
    "ring3_isr_df:",
    "mov qword ptr [rip + ring3_exc_vec], 8",
    "call ring3_exception_dispatch",
    "hlt",
    ".Ldf_halt:",
    "jmp .Ldf_halt",
);

extern "C" {
    fn ring3_enter();
    static mut ring3_ksp: u64;
    static mut ring3_save: [u64; 6];
    static mut ring3_target: u64;
    static mut ring3_v_rip: u64;
    static mut ring3_v_rsp: u64;
    static mut ring3_v_cs: u64;
    static mut ring3_v_ss: u64;
    static mut ring3_v_fl: u64;
    static mut ring3_s_rax: u64;
    static mut ring3_s_rdi: u64;
    static mut ring3_s_rsi: u64;
    static mut ring3_exc_vec: u64;
    fn ring3_isr_syscall();
    fn ring3_isr_pf();
    fn ring3_isr_gp();
    fn ring3_isr_ud();
    fn ring3_isr_df();
}
