# HYWOS 1.0.1 «星河 · 中文版»

<p align="center">
  <img src="assets/logo-256.png" width="140" alt="HYWOS logo">
</p>

HYWOS 是一个用 Rust 编写的 64 位 UEFI 操作系统，不依赖任何第三方 crate，
以 `no_std` 方式直接运行在 UEFI 固件之上。1.0.1 起内置完整中文字库，
界面全面中文化与视觉重绘。

![开机动画](docs/screenshots/boot-1.png)

## 目录

- [1.0.1 新变化](#101-新变化)
- [特性总览](#特性总览)
- [界面截图](#界面截图)
- [快捷键](#快捷键)
- [中文支持](#中文支持)
- [终端](#终端)
- [文件系统与应用存储](#文件系统与应用存储)
- [权限模型](#权限模型)
- [截屏](#截屏)
- [软件更新 .updpkg](#软件更新-updpkg)
- [构建](#构建)
- [在 QEMU 中运行](#在-qemu-中运行)
- [源码结构](#源码结构)
- [设计说明](#设计说明)
- [现实边界](#现实边界)
- [开源协议](#开源协议)

## 1.0.1 新变化

- **完整中文支持**：6786 个 GB2312 汉字（一级 + 二级）+ 中文标点，
  16×16 四阶灰度点阵，UTF-8 解码，ASCII 与中文混排。
- **全部软件 UI 重绘**：新壁纸、新状态栏、新 Dock、新窗口外框，
  终端/文件/设置/天气/商店/关于全部重新排版并中文化。
- **关于本机展示真实 LOGO**：与 `assets/logo.png` 一致的 128×128 位图。
- **开机动画中文化**：副标题「星河 · 中文界面 · 64 位」。
- 图标改用新的双色渐变与更圆润的外形。
- 天气单位改为 `C / F`，修复字库缺字与关于页重叠。

## 特性总览

| 能力 | 实现 |
|---|---|
| 启动 | UEFI x86_64，PE32+ `BOOTX64.EFI`，El Torito ISO |
| 语言 | Rust 1.100，`no_std`，零外部依赖，手写 EFI 协议绑定 |
| 中文 | 6786 字 GB2312 点阵，UTF-8 混排渲染，中文界面 |
| 图形 | GOP framebuffer，双缓冲合成器，圆角/渐变/抗锯齿/阴影 |
| 开机动画 | 星河主题：星点螺旋汇聚、双层光环、徽标淡入、中文副标 |
| 图标 | 256×256 画布 4× 超采样后降采样到 64px 缓存，Dock 缩小显示 |
| 桌面 | 中文状态栏、19 槽浮动 Dock、窗口栈、鼠标光标 |
| Launchpad | 6 列网格，渐暗与错峰缩放，搜索框，中文名称 |
| 应用 | 终端、文件、设置、天气、应用商店、关于本机 + 13 个套件图标 |
| Weather | 520×430，离线 5 日预报，5 城市，C/F，太阳浮动与云层漂移 |
| 终端 | 27 条 Linux 风格命令，中文输出，操作真实文件系统 |
| 文件系统 | UEFI SimpleFileSystem；无可写卷时启用内存文件系统回退 |
| 应用存储 | `appstore_get/set/del`，每应用独立 KV，值上限 120 字符 |
| 权限 | 存储 / 通知 / 网络三项，关闭存储时真实拒绝 |
| 截屏 | F12 / Print Screen，24 位 BMP，编号递增 |
| 更新 | CONLYUPD 包，CRC32 校验，LBA 64 内核区写入 |
| 多核 | ACPI MADT + CPUID 拓扑，QEMU 中实测 `4 核` |
| 网络 | UEFI SimpleNetwork 探测，读取真实 MAC 与链路状态 |
| 鼠标 | UEFI SimplePointer（固件 USB HID 链路） |
| 存储 | UEFI SimpleFileSystem（U 盘由固件枚举后即可读写） |

## 界面截图

| 启动动画 |
|---|
| ![启动动画](docs/screenshots/boot-1.png) |

| 桌面 | Launchpad |
|---|---|
| ![桌面](docs/screenshots/desktop.png) | ![Launchpad](docs/screenshots/launchpad.png) |

| 天气 | 设置 · 权限 |
|---|---|
| ![天气](docs/screenshots/weather.png) | ![设置](docs/screenshots/settings.png) |

| 应用商店 | 关于本机 |
|---|---|
| ![商店](docs/screenshots/store.png) | ![关于](docs/screenshots/about.png) |

| 终端 | 文件 |
|---|---|
| ![终端](docs/screenshots/terminal.png) | ![文件](docs/screenshots/files.png) |

以上截图全部由 QEMU 运行 `hywos.iso` 后，通过 VNC 抓取的 framebuffer
原始画面，未做任何修图。

## 快捷键

| 按键 | 作用 |
|---|---|
| `F1` | 启动台 |
| `F2` | 天气 |
| `F3` | 设置（权限页） |
| `F4` | 应用商店 |
| `F5` | 关于本机 |
| `F6` / `F8` | 文件 |
| `F7` | 终端 |
| `F12` | 截屏 |
| `Esc` | 关闭启动台或当前窗口 |

启动台打开时直接键入即可过滤：匹配应用中文名称与英文 ID，不区分大小写；
`Backspace` 删除，`Enter` 启动第一项，`Esc` 关闭。

## 中文支持

- `tools/gen_cjk.py` 从文泉驿微米黑生成 16×16 4bpp 点阵，覆盖
  GB2312 一级（3755）与二级（3008）汉字，外加 `，。！？：；、（）《》“”`
  等标点，共 6786 字。
- 码点表按升序存储，渲染时二分查找；缺字绘制空心方框而不是乱码。
- `gfx::text` 自动进行 UTF-8 解码：ASCII 取 8×16 字体，中文取 16×16 点阵，
  两者可在同一行混排，宽度按实际字符计算。
- 界面、窗口标题、应用名、权限页、终端提示与帮助均已中文化。
- 键盘当前只处理 ASCII 输入；中文输入法不在 1.0.1 范围内。

## 终端

`user@hywos:~$` 提供以下命令（帮助信息为中文）：

```
help  clear  ls  cd  pwd  cat  echo  mkdir  rm  cp  mv
uname ps  free date df  mount whoami uptime weather
screenshot  appstore  exit
```

`ls/cd/cat/mkdir/rm/cp/mv` 直接作用在 UEFI 文件系统上，路径分隔符为 `\`。
`date` 读取固件 RTC；当固件未提供有效日期时状态栏显示 `--:--`。

## 文件系统与应用存储

HYWOS 通过 UEFI `SimpleFileSystem` 枚举所有卷：

- 含 `\EFI\BOOT\BOOTX64.EFI` 的卷被标记为系统卷；
- 其余可写卷作为用户卷，自动建立：

```
\home\user\
├── .apps\           应用存储与权限
│   ├── <appid>.kv
│   └── _permissions
├── Documents\
├── Pictures\
└── Screenshots\     shot-0001.bmp, shot-0002.bmp, ...
```

应用存储 API（App API v3）为每个应用维护独立 KV 文件：

```
appstore_set("weather", "unit", "f")   -> 写 \home\user\.apps\weather.kv
appstore_get("weather", "unit", buf)   -> 读取
appstore_del("weather", "unit")        -> 删除
```

值上限 120 字符。如果固件没有提供可写卷（例如只读 ISO 启动），
系统会打印 `no writable volume, RAM fallback` 并启用内存文件系统，
全部功能仍可使用但重启后数据丢失。

## 权限模型

`设置 → 权限` 为每个应用提供三项开关：存储、通知、网络。
关闭存储后 `appstore_set/get/del` 真实返回拒绝。记录写入
`\home\user\.apps\_permissions`：

```
# appid storage notify network
weather 1 1 1
store 0 1 1
```

## 截屏

`F12` 或 `Print Screen` 抓取当前合成画面，编码为 24 位 BMP：

- 逐行自底向上，BGR，行按 4 字节对齐；
- 路径 `\home\user\Screenshots\shot-NNNN.bmp`，编号自动递增；
- 完成后状态栏显示 `Saved: shot-NNNN.bmp`；
- `设置 → 显示 → 立即截屏` 也提供按钮。

## 软件更新 .updpkg

内核原生识别 `CONLYUPD` 格式：

| 偏移 | 长度 | 内容 |
|---|---|---|
| 0 | 8 | `CONLYUPD` |
| 8 | 4 | 版本号 |
| 12 | 4 | payload 长度（≤32KB） |
| 16 | 4 | 目标 LBA（默认 64） |
| 20 | 4 | payload 的 CRC32 |
| 24 | 8 | 保留 |
| 32 | N | payload |

在文件管理器中双击 `.updpkg` 会打开 `设置 → 软件更新`。
点击「应用更新」后：校验头部与长度 → CRC32 校验 → 通过 Block I/O
写入目标 LBA → 不触碰用户卷。`tools/make_updpkg.py` 可生成测试包。

## 构建

依赖：Rust stable + `x86_64-unknown-uefi` target、`xorriso`、`mtools`、
`dosfstools`；运行测试需要 `qemu-system-x86_64` 与 OVMF。

```bash
rustup target add x86_64-unknown-uefi
cargo build --release --target x86_64-unknown-uefi
sh tools/mkiso.sh
```

生成中文字体与 LOGO 数据（修改文案或字库后执行）：

```bash
python3 tools/gen_cjk.py
python3 tools/gen_logo_data.py
```

产物：

```
build/hywos.iso            可启动 ISO（约 26MB）
build/efiboot.img          El Torito EFI 引导映像
build/usb.img              64MB FAT32 用户卷（MBR 分区）
build/kdisk.img            64MB 裸盘（更新测试）
build/hywos-1.0.1.updpkg   测试更新包
```

## 在 QEMU 中运行

```bash
qemu-system-x86_64 \
  -M q35 -m 512 -smp 4 \
  -bios /usr/share/edk2/ovmf/OVMF.fd \
  -drive file=build/hywos.iso,media=cdrom,readonly=on \
  -drive file=build/usb.img,if=none,id=usbdisk,format=raw \
  -device usb-storage,drive=usbdisk \
  -device usb-tablet \
  -drive file=build/kdisk.img,if=none,id=kdisk,format=raw \
  -device virtio-blk-pci,drive=kdisk \
  -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
  -vga none -device ramfb \
  -serial file:build/serial.log \
  -monitor unix:/tmp/qmon.sock,server,nowait
```

- `ramfb` 让 framebuffer 落在 RAM，避免 TCG 下 VGA MMIO 写入过慢；
- `usb-tablet` 为 SimplePointer 提供绝对坐标，固件转换成相对移动；
- 部分 OVMF 构建不会枚举 USB 存储，此时用户卷回退到内存文件系统。

`tools/vncsnap.py` 抓取 framebuffer，`tools/qmon.py` 与 `tools/vncclick.py`
用于自动化按键与点击，`tools/gen_*.py` 生成字体、LOGO 与更新包。

## 源码结构

```
src/
├── main.rs      EFI 入口、启动序列、panic 处理、memcpy/memset
├── efi.rs       手写 EFI 协议绑定
├── gfx.rs       framebuffer 合成器、UTF-8 混排文本、圆角/渐变/抗锯齿
├── font.rs      ASCII 4bpp 位图字体
├── cjk.rs       GB2312 6786 字 16×16 点阵
├── logo.rs      关于页使用的 128×128 RGBA LOGO
├── icon.rs      程序化图标 + 4× 超采样 + 64px 缓存
├── anim.rs      星河开机动画
├── hw.rs        ACPI MADT、CPUID、SNP、RTC、SimplePointer
├── fs.rs        文件系统、RAM 回退、KV、权限、BMP、updpkg、Block I/O
├── ui.rs        桌面、状态栏、Dock、启动台、窗口栈、双缓冲
├── apps/mod.rs  终端 / 文件 / 设置 / 天气 / 商店 / 关于
tools/            构建、字体、LOGO、更新包、VNC 抓屏
assets/           logo.svg / logo.png / logo-256.png
docs/screenshots  QEMU 实拍截图（含启动动画）
```

## 设计说明

**中文字体**：16×16 字号渲染，四阶灰度，码点升序 + 二分查找，
缺字回退为空心方框，避免乱码。

**图标抗锯齿**：256×256 RGBA 画布绘制后 4×4 盒式平均降采样到 64×64；
Dock 图标 34/42/46px 由 64px 缩小，边缘干净。

**Launchpad**：渐暗 208，按 `row*2+col` 错峰缩放，名称延迟淡入，
6 列固定，搜索框与提示中文。

**天气**：520×430，5 城市 × 5 日离线数据，行点击切换、选中高亮，
太阳正弦浮动、云层漂移，单位 `C/F` 通过应用存储持久化。

**多核**：优先 ACPI XSDT/RSDT 的 MADT，失败回退 CPUID leaf 1；
QEMU `-smp 4` 下状态栏显示 `4 核`。调度为多 runqueue 模型，无 AP trampoline。

**双缓冲**：绘制写入后台缓冲后一次性拷贝到 GOP framebuffer，
避免撕裂并让截屏始终拿到完整帧。

## 现实边界

真实实现：UEFI 启动、GOP 图形、SimplePointer、SimpleFileSystem 读写、
中文字库与混排、图标超采样、动画、ACPI/CPUID、SNP MAC、KV、权限拒绝、
BMP 截屏、CONLYUPD 解析与 CRC32、Block I/O 写 LBA、终端文件操作。

演示层或受限：无自有 TCP/IP 协议栈（仅展示 SNP 真实链路状态）；
多核为拓扑真实 + 单核协作式调度；USB 鼠标/存储依赖固件枚举；
文件系统为 FAT32；无中文输入法；固件无可写卷时使用内存文件系统。

## 开源协议

MIT License，见 [LICENSE](LICENSE)。