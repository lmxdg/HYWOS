# HYWOS

HYWOS 是一个面向 x86_64 的桌面操作系统，采用 **C/C++ 混合内核 + Rust 桌面**，
支持 **UEFI 与 BIOS 双引导**。内核核心、子系统注册表与混合驱动总线由 C/C++ 实现，
图形合成器、窗口、任务栏与应用由 Rust 编写。

- 开发方：**MEMZ-LMXDG**
- 许可：MIT
- 架构：x86_64
- 引导：UEFI（BOOTX64.EFI）+ BIOS（El Torito）

## 特性

- **混合内核**：内核核心、子系统注册表、能力位、自检与驱动总线由 C/C++ 实现，
  Rust 负责硬件寄存器访问、桌面与应用。
- **混合驱动**：内置驱动（GOP、PS/2、FAT VFS、AC'97 等）与模块驱动
  （virtio-gpu、SVGA、e1000、RTL8139、virtio-net、USB HID、UEFI SNP 等）
  由 C++ 驱动总线统一注册、匹配与绑定。
- **Rust 桌面**：顶部任务栏、开始菜单、窗口管理、桌面图标、壁纸、
  任务栏控制中心（亮度/音量/电源）、设置中心。
- **应用套件**：终端、文件、设置、应用商店、关于本机、浏览器、邮件、照片、
  音乐、日历、计算器、备忘录、地图、时钟、相机、视频、通讯录、天气、信息、
  安全中心、HYWOffice、AI 助手。
- **HYWOffice**：打开 txt/doc/docx/ppt/pptx 文本预览，支持 TXT 编辑。
- **AI 助手**：OpenAI 兼容接口配置、对话、提示词/skills 注入与系统操作。
- **系统能力**：用户密码与启动锁屏、音量、亮度、软关机/软重启、
  显示缩放与分辨率切换、网络诊断与测速。
- **安装程序**：隐私政策与用户许可协议、磁盘选择、FAT16 引导分区创建。
- **更新**：`.updpkg` 内核更新包、GitHub Releases 在线检测与离线回退。

## 架构

```
UEFI/BIOS
  └─ BOOTX64.EFI / bios.img
       ├─ C/C++ 混合内核核心（kernel/）
       │    ├─ 子系统注册表与状态机
       │    ├─ 能力位、自检、运行计数
       │    └─ C++ 驱动总线（builtin + module）
       └─ Rust 运行时与桌面（src/）
            ├─ 视频/输入/存储/网络/音频
            ├─ 合成器、窗口、任务栏、控制中心
            └─ 应用套件（apps/）
```

- 内核导出函数使用 `ms_abi`，Rust 以 `extern "efiapi"` 调用。
- 子系统：Kernel Core、Video、Input、Storage、Network、Audio、UI、Ring3、Updates。
- 用户态：Ring3 程序通过 `int 0x80` 请求内核服务。

## 目录

```
kernel/          C/C++ 混合内核核心与驱动总线
boot/bios/       BIOS El Torito 引导扇区
src/             Rust 内核前端、桌面与应用
  apps/          应用实现（含 office.rs、ai.rs）
  gfx.rs         图形合成（含逻辑帧缓冲缩放）
  ui.rs          桌面、任务栏、控制中心
  fs.rs          FAT16/FAT32 与 VFS
  net.rs         网卡与网络栈
docs/screenshots 截图
tools/           ISO 构建与辅助脚本
build.rs         C/C++ 交叉编译与链接
```

## 构建

依赖：

- Rust 稳定版 + `x86_64-unknown-uefi` target
- C/C++ 交叉编译器（clang，或 x86_64 交叉 gcc）
- `nasm`、`xorriso`、`mtools`、`dosfstools`

```sh
rustup target add x86_64-unknown-uefi
cargo build --release --target x86_64-unknown-uefi
sh tools/mkiso.sh
```

`build.rs` 将 `kernel/` 下的 C/C++ 交叉编译为 x86_64 COFF 对象并链接进 UEFI 映像。
aarch64 构建机自动使用 `clang --target=x86_64-pc-windows-msvc`。

产物：

- `build/hywos.iso`：UEFI + BIOS 双引导安装镜像
- `build/esp/EFI/BOOT/BOOTX64.EFI`：可直接放入 ESP 的内核
- `build/bios.img`：BIOS El Torito 引导扇区
- `build/hywos-<版本>.updpkg`：更新包

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
画面与明确提示；完整图形桌面由 UEFI 启动提供。

## 快捷键

- `F2` / Print Screen：截屏
- `F4`：关闭当前窗口
- `F11`：切换最大化
- `Super` / `0`：开始菜单
- `W`：终端
- 设置 → 系统：音量、亮度、缩放、分辨率、重启、关机
- 任务栏右侧：控制中心（亮度/音量/电源）

## 边界

- docx/pptx 仅提取正文文本，不保留排版、图片与表格。
- doc/ppt 为二进制兼容文本提取。
- AI 需要用户自行配置 OpenAI 兼容 API Key。
- UHCI/xHCI 完整驱动与 SMP AP 实际启动尚未实现。
- 分辨率与缩放运行时可切换；窗口状态在切换时会重置。

## 许可

MIT License。开发者：MEMZ-LMXDG。