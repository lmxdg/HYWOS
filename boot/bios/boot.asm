; HYWOS 1.2.0 BIOS 兼容引导扇区（NASM, 16 位实模式）
; 有限时加载动画后给出明确提示，不再无限卡在 Loading。

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
    cmp cx, 12
    jae .done

    mov dx, 0x3000
.delay:
    dec dx
    jnz .delay
    jmp .spin

.done:
    mov ah, 0x02
    mov bh, 0
    mov dh, 5
    mov dl, 0
    int 0x10
    mov si, blank
    call puts

    mov ah, 0x02
    mov bh, 0
    mov dh, 7
    mov dl, 0
    int 0x10
    mov si, notice
    call puts

.wait:
    mov ah, 0x00
    int 0x16
    cmp al, 'r'
    je .reboot
    cmp al, 'R'
    je .reboot
    cmp al, 'c'
    je .info
    cmp al, 'C'
    je .info
    jmp .wait

.reboot:
    mov si, rebootmsg
    call puts
    mov al, 0xFE
    out 0x64, al
    jmp .wait

.info:
    mov si, infomsg
    call puts
    jmp .wait

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
    db "HYWOS 1.2.0  BIOS Boot", 13, 10
    db "Hybrid kernel: C/C++ core + Rust UI", 13, 10
    db "Dual boot: BIOS compatibility mode", 13, 10
    db "Use UEFI for the full graphical desktop.", 13, 10, 13, 10
    db "Loading ", 0

blank:
    db "        ", 0

notice:
    db "BIOS compatibility boot finished.", 13, 10
    db "Full desktop requires UEFI boot.", 13, 10
    db "Press R to reboot, C for help.", 13, 10, 0

infomsg:
    db "BIOS stage: El Torito compatibility loader.", 13, 10, 0

rebootmsg:
    db "Rebooting...", 13, 10, 0

frames:
    db "|/-\"

times 510-($-$$) db 0
dw 0xAA55
times 2048-($-$$) db 0