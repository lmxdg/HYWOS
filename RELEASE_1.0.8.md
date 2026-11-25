# HYWOS 1.0.8 «星河 · 显示与输入增强版»

## 显示修复

- **高分辨率桌面空白**：UI 的合成/快照/背景缓冲原先固定为 1024×768。
  在 VMware 常见的 1280×800、1920×1080 等下，`Ui::new` 会退化为直接绘制，
  而 `present()` 又把同一块 framebuffer 按行错位自拷贝，结果只剩 wallpaper、
  状态栏与 Dock 全部消失。
  - 三块缓冲上限提升到 **1920×1080**，1280×800 走正常双缓冲路径。
  - `present()` 检测到源/目标为同一 framebuffer 时直接 flush，不再自拷贝。
  - GOP 模式选择限制在 ≤1920×1080，优先 1024×768。
- **启动动画 O 错位**：粒子 O 的目标中心原为 `tx+112 / ty+32`，落在字母 S 右侧；
  现改为 `tx+56 / ty+16`（第 4 个字母位），并收窄椭圆半径，`HYWOS` 显示完整。
- **VMware SVGA II**：
  - 读取 `SVGA_REG_MAX_WIDTH`（4）/`MAX_HEIGHT`（5），按上限选择 1024×768、
    800×600 或 640×480，避免强制不支持的模式。
  - 启用后校验 `WIDTH/HEIGHT/BPP==32`，失败则关闭并回退 VBE/GOP。
  - 同步寄存器由错误的 0x41/0x42 修正为 `SVGA_REG_SYNC=0x15`、
    `SVGA_REG_BUSY=0x16`。

## 输入驱动

- **PS/2 重写**：
  - 原实现 `KBD_OK` 从未置位、按键队列把 `(scan<<16)|char` 截断成 `u16`，
    功能键与方向键全部丢失；现队列改为 `u32`，完整保留扫描码与字符。
  - 键盘初始化补齐 0xAE、配置字节（IRQ1/翻译/使能）、0xF4 使能扫描。
  - 鼠标初始化改为 0xA8 → 配置字节（启用 aux）→ F6 复位 → F3 200Hz →
    E8 分辨率 8 → F4 使能。
  - 相对位移超过 127 时钳制，不再丢弃整包；支持左右键与 Shift/Caps 符号。
  - 桌面启动时，若固件未提供 `AbsolutePointer/SimplePointer`，自动初始化
    PS/2 鼠标；键盘在 `ConIn` 不可用、返回错误，或连续 3 次检测到未被消费的
    8042 键盘数据时自动接管。
- **UEFI 现代键鼠**：
  - `ConIn` 读取前先 `CheckEvent(WaitForKey)`，兼容只在事件就绪时才返回按键的固件。
  - 优先级：`ConIn` → `SimpleTextInputEx` → `AbsolutePointer` →
    `SimplePointer` → PS/2 → USB HID。
- **新增 USB HID 驱动（`src/usbhid.rs`）**：
  - 基于 `EFI_USB_IO_PROTOCOL` 枚举 HID 接口，使用端点描述符接口查找中断 IN 端点。
  - 支持 HID 启动协议键盘（8 字节：修饰键 + 6 键码）与鼠标
    （3/4 字节相对、5/6 字节绝对）；绝对鼠标通过报告描述符中的
    Logical Maximum `0x7FFF` 识别。
  - 仅在固件未提供对应输入驱动时接管，避免与 UsbKbDxe/UsbMouseDxe 争抢数据。
  - 键盘映射覆盖字母/数字/符号、Shift/Caps、Enter、Esc、Backspace、Tab、
    方向键与 F1–F12。

## 版本

- 版本统一升级到 **1.0.8**（Cargo.toml、启动横幅、状态栏、终端、关于本机、
  设置、安装程序、ISO 构建脚本与更新包生成器）。
- 关于本机显示实际输入路径，例如「UEFI Ex 键盘 + PS/2 鼠标」。

## 实测

- QEMU 11.1.3 + OVMF 202308，`-M q35`：
  - 1024×768 与 1280×800 下桌面、状态栏、Dock 均完整显示。
  - PS/2 鼠标可移动并点击设置页；PS/2 初始化后固件键盘仍正常（F3 打开设置）。
  - 关闭 i8042 后：USB 相对鼠标与 USB 绝对平板（QMP 绝对事件）均可点击。
  - 启动动画 `HYWOS` 的粒子 O 位于 W 与 S 之间。
