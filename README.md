# HYWOS

<p align="center">
  <img src="assets/logo-256.png" width="140" alt="HYWOS logo">
</p>

<p align="center">
  <b>开源 · MIT · x86_64 · UEFI + BIOS</b>
</p>

HYWOS 1.2.2 是一个 **C/C++ 混合内核 + Rust 桌面** 的 64 位操作系统，支持
**UEFI 与 BIOS 双引导**。内核核心、子系统注册表与混合驱动总线由 C/C++ 实现，
界面、合成器、窗口与应用仍由 Rust 编写；不链接任何第三方 crate，自带显卡、
输入、音频驱动、桌面环境、应用套件与安全中心，可制作启动 U 盘或 ISO 安装到本地磁盘。

![桌面](docs/screenshots/desktop.png)

## 目录

- [设计目标](#设计目标)
- [应用套件](#应用套件)
- [安全中心](#安全中心)
- [显示](#显示)
- [输入](#输入)
- [音频](#音频)
- [硬件与驱动](#硬件与驱动)
- [混合内核（Hybrid）](#混合内核hybrid)
- [Ring3 与双页表](#ring3-与双页表)
- [系统架构](#系统架构)
- [文件系统与数据](#文件系统与数据)
- [构建](#构建)
- [在-QEMU-中运行](#在-qemu-中运行)
- [应用开发](#应用开发)
- [快捷键](#快捷键)
- [界面截图](#界面截图)
- [现实边界](#现实边界)
- [开源协议](#开源协议)

## 设计目标

- **零依赖**：不引入任何 crate，直接手写 UEFI 协议绑定、图形合成、文件系统与驱动。
- **混合内核**：内核核心、子系统注册表与驱动总线由 C/C++ 实现，兼顾内核态服务与
  模块化驱动；UI 与桌面保留 Rust，Rust 通过 FFI 与 C/C++ 内核交互。
- **混合驱动**：内置驱动（GOP、PS/2、VFS、AC'97）与模块驱动（virtio-gpu、SVGA、
  e1000、RTL8139、virtio-net、USB HID、UEFI SNP）由 C++ 驱动总线统一匹配与绑定。
- **真实可用**：桌面上的每个应用都有实际功能与持久化数据，不使用占位页面。
- **本地优先**：数据写入用户卷；自研 TCP/IP 栈提供明文 HTTP，TLS 由固件 HTTP 承担，离线时功能完整。
- **可安装、可更新**：ISO 安装到本地磁盘，`.updpkg` 更新内核并保留用户数据。

## 应用套件

| 应用 | 能力 | 数据位置 |
|---|---|---|
| 终端 | 文件、系统、网络（DHCP/DNS/TCP/HTTP）、截屏、更新 | 当前目录 |
| 文件 | 浏览用户卷，双击打开文本/更新包 | 用户卷 |
| 设置 | 显示、权限、软件更新、关于本机 | `\home\user\.apps` |
| 应用商店 | 安装状态持久化 | `\home\user\.apps` |
| 关于本机 | 显卡/输入/音频/CPU/内存真实信息 | 只读 |
| 浏览器 | 本地页面、自研 HTTP 与固件 HTTPS 打开网址 | `\HYWOS\SYSTEM` |
| 邮件 | 本地收件箱/发件箱，撰写保存 | `\home\user\Mail` |
| 照片 | 解码浏览 24 位 BMP | `\home\user\Pictures` |
| 音乐 | 播放 PCM WAV，内置音阶 | `\home\user\Music` |
| 日历 | RTC 月历、切月、当天高亮 | RTC |
| 计算器 | 四则、括号、小数、键盘与按钮 | 内存 |
| 备忘录 | 新建/编辑/保存，Ctrl+S | `\home\user\Notes` |
| 地图 | 经纬网、大陆轮廓、城市、平移缩放 | 内置 |
| 时钟 | 模拟钟面、数字时间、秒表 | RTC |
| 相机 | 实时取景，保存全屏 BMP | `\home\user\Screenshots` |
| 视频 | BMP 序列循环播放 | `\home\user\Videos` |
| 通讯录 | 联系人浏览与新增 | `\home\user\Contacts` |
| 信息 | 本地消息记录与发送 | `\home\user\Messages` |
| 安全中心 | 实时监控、快速/全盘查杀、病毒库 | `\HYWOS\SECURITY` |
| 天气 | 5 城市离线 5 日预报（开始菜单） | 内置 |
| HYWOffice | 打开 txt/doc/docx/ppt/pptx 文本预览 | `\home\user\Documents` |

桌面采用 Windows 风格：**顶部全宽任务栏**包含系统 LOGO 开始按钮、搜索框与
6 个固定应用，开始菜单展示全部 21 个应用；
任务栏下方左侧提供「此电脑 / 终端 / 浏览器 / 安全中心」图标，双击打开。
应用首次打开时会自动创建缺失目录与示例记录。

## 用户密码与锁屏

- 设置 → 账户 可设置、修改或清除登录密码，支持 `P` / `X` 快捷键。
- 密码以 FNV-1a 哈希保存在 `\home\user\.apps\account.kv`，不保存明文。
- 已设置密码时，启动动画结束后显示锁屏，输入密码并按 Enter 解锁。

| 账户设置 | 启动锁屏 |
|---|---|
| ![账户](docs/screenshots/account.png) | ![锁屏](docs/screenshots/lock-screen.png) |

## HYWOffice 与壁纸

- **HYWOffice**：开始菜单应用，支持 `txt`、`doc`、`docx`、`ppt`、`pptx` 文本预览。
  docx/pptx 通过 ZIP + DEFLATE + XML 提取正文；doc/ppt 为二进制兼容提取。
  不保留原始排版、图片与表格结构。
- **5 张内置壁纸**：深空、极光、山脉、海洋、日落；设置 → 显示 → 桌面壁纸，
  单击缩略图或按 `1`–`5` 切换，选择保存到 `\home\user\.apps\wallpaper.kv`。

| HYWOffice | 壁纸切换 |
|---|---|
| ![Office](docs/screenshots/office.png) | ![壁纸](docs/screenshots/wallpaper.png) |

## 安全中心

- **实时监控**：逻辑核心数、内存总量/可用量、运行时间、实时防护状态。
  内存可用率每 24 帧采样并绘制趋势线，数据来自 `GetMemoryMap` 真实结果。
- **快速查杀**：检查内核、版本文件、安装标记、系统资源与权限文件。
- **全盘查杀**：递归系统卷与用户卷（深度 4、最多 400 个文件），
  逐个计算 FNV-1a 签名并与病毒库比对，显示进度、当前文件与日志。
- **病毒库**：`\HYWOS\SECURITY\VIRUS.DB`，每行 `8位十六进制签名|名称`。
  可从用户卷或系统卷加载；未提供病毒库时签名数量为 0，扫描结果为
  “未发现匹配威胁”。

## 显示

- 显卡探测顺序：virtio-gpu（legacy 2D）→ VMware SVGA II → VBE → UEFI GOP。
- SVGA 读取 `MAX_WIDTH/MAX_HEIGHT` 选择安全模式，并校验 32bpp。
- 软件合成器支持到 **1920×1080**；1024×768 与 1280×800 均完整显示任务栏与开始菜单。
- 启动动画：LOGO 由小到大缓入，`HYWOS` 字标同步淡入，底部旋转加载圈，约 2.4 秒。

## 输入

| 优先级 | 键盘 | 指针 |
|---|---|---|
| 1 | UEFI `ConIn`（先 CheckEvent） | `AbsolutePointer` |
| 2 | UEFI `SimpleTextInputEx` | `SimplePointer` |
| 3 | PS/2 8042（自动接管） | PS/2 8042 鼠标 |
| 4 | USB HID（EFI_USB_IO） | USB HID 绝对/相对鼠标 |

- PS/2 驱动支持 Shift/Caps、方向键、F1–F12、左右键；鼠标位移钳制不丢包。
- USB HID 解析接口、端点与报告描述符，支持启动协议键盘、相对与绝对鼠标；
  仅在固件未提供对应驱动时接管。

## 网络

- 自研 PCI 网卡驱动：Intel 82540EM/82574L（e1000）、Realtek RTL8139、
  virtio-net（legacy）。驱动完成 PCI 枚举、总线主控、DMA 收发环、MAC 与链路状态。
- 自研 TCP/IP 栈（`src/netstack.rs`）：
  - **DHCP**：DISCOVER → OFFER → REQUEST → ACK，动态获取地址、网关与 DNS；
  - **DNS**：UDP 53 A 记录查询，支持域名解析；
  - **TCP**：三次握手、数据收发、ACK/FIN 关闭；
  - **HTTP/1.0**：`GET` 请求与响应头解析，明文 HTTP。
- 网络配置优先 DHCP，失败后回退静态 `10.0.2.15/24`、网关 `10.0.2.2`、DNS `10.0.2.2`。
- 终端命令：`net`（驱动与配置状态）、`dhcp`（重新获取）、`resolve <域名>`、
  `http <URL>`（自研栈 GET）、`ping <IP>`。
- 浏览器访问 `http://` 使用自研栈，`https://` 使用固件 `EFI_HTTP_PROTOCOL`（TLS）。
- QEMU 实测：e1000 下完成 DHCP、DNS、TCP、HTTP 全链路；virtio-net（legacy）
  完成 ARP/ICMP；RTL8139 驱动已实现，收包验证仍在进行。

## 音频

- Intel AC'97（8086:2415）DMA 播放，48 kHz 16 位立体声。
- 启动提示音、音乐应用 WAV 播放与音阶演示。

## 硬件与驱动

| 类别 | 驱动 |
|---|---|
| 显卡 | virtio-gpu 2D、VMware SVGA II、Bochs VBE、UEFI GOP 回退 |
| 输入 | PS/2 8042、UEFI Ex/绝对指针/SimplePointer、USB HID |
| 音频 | Intel AC'97；无设备时 PC Speaker |
| 存储 | UEFI Block I/O + SimpleFileSystem，4Kn 支持 |
| 网络 | Intel e1000/82574L、Realtek RTL8139、virtio-net（legacy）；UEFI SNP 回退 |
| 固件 | ACPI MADT、CPUID、RTC、SMBIOS 字符串 |

## 混合内核（Hybrid）

HYWOS 1.2.2 采用混合内核架构：

- **C/C++ 内核核心**（`kernel/`）：子系统注册表、状态机、内核能力位、运行计数、
  自检与驱动总线。所有导出函数使用 `ms_abi`，Rust 以 `extern "efiapi"` 调用。
- **Rust 桌面**（`src/`）：显卡/输入/存储/网络/音频寄存器访问、合成器、窗口、
  任务栏、开始菜单与应用套件。
- **内核态服务 + 模块化服务**：核心服务驻留在内核空间，驱动与子系统通过统一
  注册表管理，兼顾宏内核的直接调用与微内核的模块边界。

| # | 子系统 | 职责 | 实现 |
|---|---|---|---|
| 0 | Kernel Core | 子系统与驱动总线、能力位、自检 | `kernel/core/*.c(pp)` |
| 1 | Video | 显卡探测、framebuffer、模式设置 | `video.rs` / `gfx.rs` |
| 2 | Input | PS/2、USB HID、UEFI 指针、键盘 | `input.rs` / `ps2.rs` / `usbhid.rs` |
| 3 | Storage | 卷、FAT16/FAT32、文件与目录 | `fs.rs` / `diskfmt.rs` |
| 4 | Network | e1000/82574L、RTL8139、virtio-net、DHCP/DNS/TCP/HTTP | `net.rs` / `netstack.rs` |
| 5 | Audio | AC'97 DMA 与 PC Speaker | `audio.rs` |
| 6 | UI | 合成器、窗口、顶部任务栏、开始菜单、桌面图标 | `ui.rs` / `apps/` |
| 7 | Ring3 | GDT/TSS/IDT、双页表、用户程序 | `ring3.rs` |
| 8 | Updates | `.updpkg`、GitHub Releases、离线回退 | `updater.rs` / `http.rs` |

- `src/ckernel.rs` 提供 C/C++ 内核 FFI；`src/kernel.rs` 只负责探测上报与 UI 快照。
- 终端 `kernel` 命令列出 9 个子系统与 C++ 驱动总线（builtin 6 / module 10，
  16/16 已绑定）；关于本机与安全中心显示“混合内核 · N/9 · 驱动 B/D”。
- 混合内核不排斥用户态：Ring3 程序仍可通过 `int 0x80` 请求内核服务。

### 混合驱动架构

C++ 驱动总线（`kernel/core/driver_bus.cpp`）区分两类驱动：

- **builtin**：随内核驻留的 GOP 帧缓冲、PS/2 键盘/鼠标、FAT VFS、UEFI Block、AC'97。
- **module**：按总线匹配后绑定的 virtio-gpu、VMware SVGA、Bochs VBE、e1000/e1000e、
  RTL8139、virtio-net、USB HID、UEFI SNP、UEFI Absolute Pointer。

## Ring3 与双页表

- **用户态（CPL3）**：自定义 GDT 在固件 GDT 基础上追加用户代码/数据段、内核数据段与
  64 位 TSS；TSS.RSP0 指向独立内核栈，`int 0x80` 从用户态进入内核时自动切换栈。
- **双页表**：
  - 内核页表：identity 映射 0–4 GB（2 MB 大页，U/S=0），供内核/UEFI 使用。
  - 用户页表：复制内核映射，并在 PML4[128] 建立用户代码与用户栈
    （`0x0000_4000_0000_0000` / `+0x1000`，各级 U/S=1）。
  - 进入用户程序时切换 CR3；退出/异常时恢复内核 CR3。
- **系统调用**：`int 0x80`，调用号 1=输出、2=退出、3=运行时间；
  DPL=3 的中断门，参数与结果通过寄存器传递。
- **异常隔离**：IDT 覆盖 #PF/#GP/#UD/#DF；用户态访问内核地址（如 `0x1000`）
  触发 #PF，处理程序记录 CR2 后安全返回内核，系统继续运行。
- **验证入口**：终端 `ring3` 运行正常 syscall/exit 程序，`ring3f` 运行页错误隔离测试；
  关于本机显示“Ring0 内核 · Ring3 用户态 · 双页表”。

## 系统架构

启动流程：

1. UEFI 固件加载 `\EFI\BOOT\BOOTX64.EFI`；BIOS 固件通过 El Torito 加载 `bios.img`
   进入兼容引导画面（旋转加载圈 + 双引导提示）。
2. 探测显卡并建立合成器；挂载卷，判定启动卷与安装介质。
3. 若从 ISO 启动：询问安装 → 复制系统文件 → 创建 FAT16 引导分区。
4. 否则：LOGO 缩放动画 → 预生成图标 → 探测 ACPI/网络/输入/音频 → 桌面。

核心模块：

| 文件 | 职责 |
|---|---|
| `src/main.rs` | EFI 入口、启动流程、内存函数 |
| `src/efi.rs` | UEFI 协议绑定、ConIn、CRC32、内存映射 |
| `src/ckernel.rs` / `kernel/` | C/C++ 混合内核核心、C++ 驱动总线与 FFI |
| `src/kernel.rs` | Rust 侧混合内核前端、探测上报、系统调用入口 |
| `src/video.rs` | virtio-gpu / SVGA / VBE / GOP |
| `src/gfx.rs` | 32bpp 合成、UTF-8 混排、圆角/渐变/位图 |
| `src/cjk.rs` | GB2312 点阵字库（6786 字） |
| `src/fs.rs` / `src/diskfmt.rs` | 文件系统、FAT16 安装、BMP 截屏、更新包 |
| `src/ui.rs` | 窗口栈、任务栏、开始菜单、桌面图标、合成与截屏 |
| `src/apps/mod.rs` | 终端、文件、设置、商店、天气、关于 |
| `src/apps/suite.rs` | 应用套件与安全中心 |
| `src/ps2.rs` / `src/usbhid.rs` / `src/input.rs` | 输入驱动 |
| `src/audio.rs` | AC'97 |
| `src/anim.rs` | LOGO 缩放淡入动画 |
| `src/netstack.rs` | DHCP、DNS、TCP、HTTP 协议栈 |
| `src/http.rs` / `src/updater.rs` | 固件 HTTPS、在线更新 |

## 文件系统与数据

```
用户卷\
├── home\user\
│   ├── .apps\<appid>.kv 与 _permissions
│   ├── Documents\  Pictures\  Screenshots\
│   ├── Notes\  Music\  Videos\
│   ├── Contacts\  Messages\  Mail\
│   └── updates\
└── updates\latest.json
系统卷\
├── EFI\BOOT\BOOTX64.EFI
└── HYWOS\
    ├── VERSION  INSTALL.TXT
    ├── SYSTEM\WELCOME.TXT
    └── SECURITY\VIRUS.DB（可选）
```

- 权限：设置中按应用控制存储、通知、网络；关闭存储后应用读写接口真实拒绝。
- 更新：`.updpkg`（CONLYUPD + CRC32）替换 `\EFI\BOOT\BOOTX64.EFI` 并写入版本，
  用户数据保留；设置内可在线检查 GitHub Releases 或使用本地更新包。

## 构建

依赖 Rust 稳定版、`x86_64-unknown-uefi` target，以及可生成 x86_64 COFF 的 C/C++
交叉编译器（如 clang）与 `nasm`、`xorriso`、`mtools`、`dosfstools`。

```sh
rustup target add x86_64-unknown-uefi
cargo build --release --target x86_64-unknown-uefi
sh tools/mkiso.sh
```

`build.rs` 会把 `kernel/` 下的 C/C++ 源码交叉编译为 x86_64 COFF 对象并链接进
UEFI 映像。构建机为 aarch64 时会自动使用 `clang --target=x86_64-pc-windows-msvc`。

产物：

- `build/hywos.iso`：UEFI + BIOS 双引导安装 ISO。
- `build/esp/EFI/BOOT/BOOTX64.EFI`：可直接放入 ESP 的内核。
- `build/bios.img`：BIOS El Torito 引导扇区。
- `build/hywos-<版本>.updpkg`：内核更新包。

## 在 QEMU 中运行

UEFI（OVMF 分离固件）：

```sh
qemu-system-x86_64 -M q35 -m 512 -smp 2 -vga std -usb \
  -drive if=pflash,format=raw,readonly=on,file=code.fd \
  -drive if=pflash,format=raw,file=vars.fd \
  -cdrom build/hywos.iso -boot d \
  -drive file=build/install.img,format=raw,if=virtio
```

BIOS（SeaBIOS，无需 OVMF）：

```sh
qemu-system-x86_64 -M pc -m 256 -vga std \
  -cdrom build/hywos.iso -boot d
```

安装完成后移除 ISO，仅保留安装盘即可从本地磁盘启动。BIOS 路径提供兼容引导
画面与加载圈；完整图形桌面由 UEFI 启动提供。

## 应用开发

- 应用标识：`src/icon.rs` 的 `AppId`；名称、ID、颜色与图标同文件维护。
- 轻量应用：在 `src/apps/suite.rs` 增加 `draw/click/key` 分支与状态字段。
- 复杂应用：在 `src/apps/mod.rs` 注册独立 `AppState` 与绘制函数。
- 持久化：使用 `fs::read_file/write_file/list_dir`，数据放在用户卷。
- 窗口尺寸：`apps::app_size`；任务栏固定项由 `ui.rs::TB_APPS` 驱动。

## 快捷键

| 按键 | 作用 |
|---|---|
| `Super` / `0` | 打开或关闭开始菜单 |
| `W` | 打开终端（桌面状态） |
| `↑` / `↓` | 选择桌面图标，`Enter` 打开 |
| `Tab` | 在窗口之间切换 |
| `F10` / `Menu` | 桌面上下文菜单（数字 `1`–`5` 选择） |
| `F11` | 最大化 / 还原当前窗口 |
| `F2` | 截屏 |
| `F4` | 关闭当前窗口 |
| `Esc` | 关闭菜单 / 开始菜单 |

鼠标：拖动标题栏移动窗口；右上角按钮依次为最小化、最大化、关闭；
点击任务栏图标切换应用，右下角竖条显示桌面；桌面右键打开菜单，双击图标启动应用。

设置内：`←` / `→` 切页，`C` 检查更新，`D` 下载安装。
安装程序：`Enter` 安装，`N` / `Esc` 取消。
开始菜单：直接键入过滤中英文名称或应用 ID，`Enter` 启动第一项。

## 界面截图

| 启动动画（LOGO 淡入） | 安装程序 |
|---|---|
| ![启动](docs/screenshots/boot-large.png) | ![安装](docs/screenshots/installer.png) |

| 安装进度 | 安装完成 |
|---|---|
| ![进度](docs/screenshots/progress.png) | ![完成](docs/screenshots/install-done.png) |

| 桌面（Windows 风格） | 开始菜单 |
|---|---|
| ![桌面](docs/screenshots/desktop.png) | ![开始菜单](docs/screenshots/launchpad.png) |

| 设置 · 权限 | 设置 · 软件更新 |
|---|---|
| ![设置](docs/screenshots/settings.png) | ![更新](docs/screenshots/update.png) |

| 应用商店 | 天气 |
|---|---|
| ![商店](docs/screenshots/store.png) | ![天气](docs/screenshots/weather.png) |

| 关于本机 | 终端 |
|---|---|
| ![关于](docs/screenshots/about.png) | ![终端](docs/screenshots/terminal.png) |

| 终端 · net | 终端 · ping |
|---|---|
| ![net](docs/screenshots/terminal-net.png) | ![ping](docs/screenshots/terminal-ping.png) |

| 终端 · Ring3 | 终端 · 页表隔离 |
|---|---|
| ![ring3](docs/screenshots/terminal-ring3.png) | ![ring3f](docs/screenshots/terminal-ring3f.png) |

| 终端 · 混合内核与驱动总线 |
|---|
| ![kernel](docs/screenshots/terminal-kernel.png) |

| 文件 | 安全中心 |
|---|---|
| ![文件](docs/screenshots/files.png) | ![安全](docs/screenshots/app-security.png) |

| 浏览器 | 邮件 |
|---|---|
| ![浏览器](docs/screenshots/app-browser.png) | ![邮件](docs/screenshots/app-mail.png) |

| 照片 | 视频 |
|---|---|
| ![照片](docs/screenshots/app-photos.png) | ![视频](docs/screenshots/app-videos.png) |

| 音乐 | 日历 |
|---|---|
| ![音乐](docs/screenshots/app-music.png) | ![日历](docs/screenshots/app-calendar.png) |

| 计算器 | 备忘录 |
|---|---|
| ![计算器](docs/screenshots/app-calculator.png) | ![备忘录](docs/screenshots/app-notes.png) |

| 地图 | 时钟 |
|---|---|
| ![地图](docs/screenshots/app-maps.png) | ![时钟](docs/screenshots/app-clock.png) |

| 相机 | 通讯录 |
|---|---|
| ![相机](docs/screenshots/app-camera.png) | ![通讯录](docs/screenshots/app-contacts.png) |

| 信息 |
|---|
| ![信息](docs/screenshots/app-messages.png) |

截图由 QEMU 运行 HYWOS 生成，通过 VNC 抓取 framebuffer 原始画面，未做修图。

## 现实边界

- 网络：自研栈支持 DHCP、DNS、TCP 与明文 HTTP；HTTPS/TLS 仍由固件 HTTP 提供。
  暂不含 IPv6、TCP 重传与拥塞控制。
- 邮件、信息：本地记录与撰写，不连接 SMTP/短信网关。
- 视频：播放 BMP 序列，不含 H.264/H.265 解码。
- 相机：采集当前合成画面，不含 USB 摄像头驱动。
- 病毒库：需用户提供 `VIRUS.DB`；仓库不内置样本签名。
- 中文输入：显示完整，键盘输入为 ASCII；中文输入法不在范围内。
- 用户态：内置固定用户程序用于验证 Ring3/syscall/隔离，尚未实现 ELF 加载器与独立用户进程管理。
- 分辨率：软件合成上限 1920×1080。
- BIOS 引导：提供 El Torito 兼容引导画面与旋转加载圈；完整图形桌面依赖 UEFI GOP。
- 混合内核：C/C++ 负责内核核心、子系统注册表与驱动总线；具体硬件寄存器访问仍在 Rust，
  并非把全部驱动寄存器操作迁入 C/C++。

## 开源协议

MIT License，见 [LICENSE](LICENSE)。
