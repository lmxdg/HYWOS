; HYWOS 1.1.6 BIOS 兼容引导扇区（NASM, 16 位实模式）
; 由 El Torito 以 no-emulation 方式加载到 0x7C00，显示启动画面与旋转加载圈。
; 完整图形桌面依赖 UEFI；BIOS 路径提供兼容引导与自检画面。

[org 0x7c00]
[bits 16]

start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7c00
    sti

    mov ax, 0x0003
    int 0x10

    mov si, banner
    call puts

    xor cx, cx
.spin:
    ; 光标定位到第 9 行、第 30 列
    mov ah, 0x02
    mov bh, 0
    mov dh, 5
    mov dl, 9
    int 0x10

    push cx
    mov ax, cx
    and ax, 3
    mov bx, frames
    add bx, ax
    mov al, [bx]
    mov ah, 0x0e
    mov bh, 0
    int 0x10
    pop cx

    inc cx

    mov dx, 0x3000
.delay:
    dec dx
    jnz .delay
    jmp .spin

puts:
    push ax
    push bx
.next:
    lodsb
    test al, al
    jz .done
    mov ah, 0x0e
    mov bh, 0
    int 0x10
    jmp .next
.done:
    pop bx
    pop ax
    ret

banner:
    db "HYWOS 1.1.6  BIOS Boot", 13, 10
    db "Hybrid kernel: C/C++ core + Rust UI", 13, 10
    db "Dual boot: BIOS compatibility mode", 13, 10
    db "Use UEFI for the full graphical desktop.", 13, 10, 13, 10
    db "Loading ", 0

frames:
    db "|/-\"

times 510-($-$$) db 0
dw 0xAA55
times 2048-($-$$) db 0