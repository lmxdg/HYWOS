# HYWOS

<p align="center">
  <img src="assets/logo-256.png" width="140" alt="HYWOS logo">
</p>

<p align="center">
  <b>当前版本 1.1.0 «星河»</b> · MIT · x86_64 UEFI
</p>

HYWOS 是一个用 Rust 从零编写的 64 位 UEFI 操作系统。它不链接任何第三方 crate，
以 `no_std` 方式直接运行在固件之上，自带显卡、输入、音频驱动、桌面环境、
应用套件与安全中心，可制作启动 U 盘或 ISO 安装到本地磁盘。

![桌面](docs/screenshots/desktop.png)

## 目录

- [设计目标](#设计目标)
- [应用套件](#应用套件)
- [安全中心](#安全中心)
- [显示](#显示)
- [输入](#输入)
- [音频](#音频)
- [硬件与驱动](#硬件与驱动)
- [系统架构](#系统架构)
- [文件系统与数据](#文件系统与数据)
- [构建](#构建)
- [在-QEMU-中运行](#在-qemu-中运行)
- [应用开发](#应用开发)
- [快捷键](#快捷键)
- [界面截图](#界面截图)
- [版本历史](#版本历史)
- [现实边界](#现实边界)
- [开源协议](#开源协议)

## 设计目标

- **零依赖**：不引入任何 crate，直接手写 UEFI 协议绑定、图形合成、文件系统与驱动。
- **真实可用**：桌面上的每个应用都有实际功能与持久化数据，不使用占位页面。
- **本地优先**：数据写入用户卷；网络能力由固件 HTTP 提供，离线时功能完整。
- **可安装、可更新**：ISO 安装到本地磁盘，`.updpkg` 更新内核并保留用户数据。

## 应用套件

| 应用 | 能力 | 数据位置 |
|---|---|---|
| 终端 | 27 条命令：文件、系统、网络、截屏、更新 | 当前目录 |
| 文件 | 浏览用户卷，双击打开文本/更新包 | 用户卷 |
| 设置 | 显示、权限、软件更新、关于本机 | `\home\user\.apps` |
| 应用商店 | 安装状态持久化 | `\home\user\.apps` |
| 关于本机 | 显卡/输入/音频/CPU/内存真实信息 | 只读 |
| 浏览器 | 本地页面、UEFI HTTP 打开网址 | `\HYWOS\SYSTEM` |
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
| 天气 | 5 城市离线 5 日预报（启动台） | 内置 |

Dock 为 **19 个应用 + 启动台**，启动台展示全部 20 个应用。
应用首次打开时会自动创建缺失目录与示例记录。

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
- 软件合成器支持到 **1920×1080**；1024×768 与 1280×800 均完整显示状态栏与 Dock。
- 启动动画：LOGO 淡入 + `HYWOS` 文字，无副标题，约 2.4 秒。

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
| 网络 | SNP（MAC/链路），HTTP 由固件 `EFI_HTTP_PROTOCOL` 提供 |
| 固件 | ACPI MADT、CPUID、RTC、SMBIOS 字符串 |

## 系统架构

启动流程：

1. 固件加载 `\EFI\BOOT\BOOTX64.EFI`。
2. 探测显卡并建立合成器；挂载卷，判定启动卷与安装介质。
3. 若从 ISO 启动：询问安装 → 复制系统文件 → 创建 FAT16 引导分区。
4. 否则：LOGO 淡入动画 → 预生成图标 → 探测 ACPI/网络/输入/音频 → 桌面。

核心模块：

| 文件 | 职责 |
|---|---|
| `src/main.rs` | EFI 入口、启动流程、内存函数 |
| `src/efi.rs` | UEFI 协议绑定、ConIn、CRC32、内存映射 |
| `src/video.rs` | virtio-gpu / SVGA / VBE / GOP |
| `src/gfx.rs` | 32bpp 合成、UTF-8 混排、圆角/渐变/位图 |
| `src/cjk.rs` | GB2312 点阵字库（6786 字） |
| `src/fs.rs` / `src/diskfmt.rs` | 文件系统、FAT16 安装、BMP 截屏、更新包 |
| `src/ui.rs` | 窗口栈、Dock、启动台、合成与截屏 |
| `src/apps/mod.rs` | 终端、文件、设置、商店、天气、关于 |
| `src/apps/suite.rs` | 应用套件与安全中心 |
| `src/ps2.rs` / `src/usbhid.rs` / `src/input.rs` | 输入驱动 |
| `src/audio.rs` | AC'97 |
| `src/anim.rs` | LOGO 淡入动画 |
| `src/http.rs` / `src/updater.rs` | 固件 HTTP、在线更新 |

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

依赖 Rust 稳定版与 `x86_64-unknown-uefi` target：

```sh
rustup target add x86_64-unknown-uefi
cargo build --release
sh tools/mkiso.sh
```

产物：

- `build/hywos.iso`：可启动安装 ISO。
- `build/esp/EFI/BOOT/BOOTX64.EFI`：可直接放入 ESP 的内核。
- `build/hywos-<版本>.updpkg`：内核更新包。

## 在 QEMU 中运行

```sh
qemu-system-x86_64 -M q35 -m 512 -smp 4 -vga std -usb \
  -bios /usr/share/ovmf/OVMF.fd \
  -cdrom build/hywos.iso \
  -drive file=build/install.img,format=raw,if=virtio
```

安装完成后移除 ISO，仅保留安装盘即可从硬盘启动。

## 应用开发

- 应用标识：`src/icon.rs` 的 `AppId`；名称、ID、颜色与图标同文件维护。
- 轻量应用：在 `src/apps/suite.rs` 增加 `draw/click/key` 分支与状态字段。
- 复杂应用：在 `src/apps/mod.rs` 注册独立 `AppState` 与绘制函数。
- 持久化：使用 `fs::read_file/write_file/list_dir`，数据放在用户卷。
- 窗口尺寸：`apps::app_size`；Dock 由 `ui.rs::DOCK_APPS` 驱动。

## 快捷键

| 按键 | 作用 |
|---|---|
| `F1` | 启动台 |
| `F2` | 天气 |
| `F3` | 设置 |
| `F4` | 应用商店 |
| `F5` | 关于本机 |
| `F6` / `F8` | 文件 |
| `F7` | 终端 |
| `F12` | 截屏 |
| `Esc` | 关闭当前窗口 / 启动台 |

设置内：`←` / `→` 切页，`C` 检查更新，`D` 下载安装。
安装程序：`Enter` 安装，`N` / `Esc` 取消。
启动台：直接键入过滤中英文名称或应用 ID，`Enter` 启动第一项。

## 界面截图

| 启动动画（LOGO 淡入） | 安装程序 |
|---|---|
| ![启动](docs/screenshots/boot-large.png) | ![安装](docs/screenshots/installer.png) |

| 安装进度 | 安装完成 |
|---|---|
| ![进度](docs/screenshots/progress.png) | ![完成](docs/screenshots/install-done.png) |

| 桌面 | 启动台 |
|---|---|
| ![桌面](docs/screenshots/desktop.png) | ![启动台](docs/screenshots/launchpad.png) |

| 设置 · 权限 | 设置 · 软件更新 |
|---|---|
| ![设置](docs/screenshots/settings.png) | ![更新](docs/screenshots/update.png) |

| 应用商店 | 天气 |
|---|---|
| ![商店](docs/screenshots/store.png) | ![天气](docs/screenshots/weather.png) |

| 关于本机 | 终端 |
|---|---|
| ![关于](docs/screenshots/about.png) | ![终端](docs/screenshots/terminal.png) |

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

截图由 QEMU 11.1.3 运行 HYWOS 1.1.0 生成，通过 VNC 抓取 framebuffer 原始画面，未做修图。

## 版本历史

| 版本 | 主题 | 发布 |
|---|---|---|
| 1.1.0 | 全应用可用、安全中心、LOGO 淡入 | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.1.0) |
| 1.0.8 | 高分辨率显示、PS/2 重写、USB HID | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.8) |
| 1.0.7 | 安装检测、安装流程、在线更新 | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.7) |
| 1.0.6 | 安装进度条 | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.6) |
| 1.0.5 | 硬盘检测、4Kn | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.5) |
| 1.0.4 | ISO 安装介质、FAT16 引导 | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.4) |
| 1.0.3 | 粒子动画、现代输入 | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.3) |
| 1.0.2 | 显卡/PS2/声卡驱动 | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.2) |
| 1.0.1 | 中文界面与 UI 重绘 | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.1) |
| 1.0.0 | 首个版本 | [Release](https://github.com/lmxdg/HYWOS/releases/tag/v1.0.0) |

## 现实边界

- 网络：内核不含 TCP/IP 栈；浏览器与在线更新依赖固件 `EFI_HTTP_PROTOCOL`。
  固件缺少 TLS 证书时，仅本地页面与离线更新可用。
- 邮件、信息：本地记录与撰写，不连接 SMTP/短信网关。
- 视频：播放 BMP 序列，不含 H.264/H.265 解码。
- 相机：采集当前合成画面，不含 USB 摄像头驱动。
- 病毒库：需用户提供 `VIRUS.DB`；仓库不内置样本签名。
- 中文输入：显示完整，键盘输入为 ASCII；中文输入法不在范围内。
- 分辨率：软件合成上限 1920×1080。

## 开源协议

MIT License，见 [LICENSE](LICENSE)。
