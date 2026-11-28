# HYWOS 1.1.6 «混合内核 + 双引导版»

本次更新把内核核心改写为 C/C++ 混合内核，保留 Rust 桌面，并加入 UEFI + BIOS
双引导、顶部任务栏与启动加载动画。

## 混合内核

- **C/C++ 内核核心**（`kernel/`）：子系统注册表、状态机、能力位、运行计数、
  自检与驱动总线。导出函数统一使用 `ms_abi`，Rust 以 `extern "efiapi"` 调用。
- **Rust 桌面**：显卡/输入/存储/网络/音频寄存器访问、合成器、窗口、任务栏、
  开始菜单与应用套件继续由 Rust 实现，交互逻辑保持不变。
- **9 个子系统**：Kernel Core、Video、Input、Storage、Network、Audio、UI、
  Ring3、Updates；状态由 C/C++ 内核维护，Rust 探测后上报。
- 终端 `kernel` 命令显示混合内核状态与驱动总线，并逐项列出驱动。

## 混合驱动架构

C++ 驱动总线（`kernel/core/driver_bus.cpp`）统一注册、匹配与绑定两类驱动：

- **builtin（6）**：GOP 帧缓冲、PS/2 键盘、PS/2 鼠标、FAT VFS、UEFI Block、AC'97。
- **module（10）**：virtio-gpu、VMware SVGA、Bochs VBE、e1000、e1000e、RTL8139、
  virtio-net、USB HID、UEFI SNP、UEFI Absolute Pointer。
- 启动时完成 16/16 绑定，终端显示 `builtin 6 module 10 bound 16/16`。

## UEFI + BIOS 双引导

- ISO 同时包含 El Torito 的 BIOS 与 UEFI 引导条目，`-eltorito-alt-boot` 保证两项共存。
- BIOS 引导扇区（`boot/bios/boot.asm`，NASM）显示 HYWOS 1.1.6 兼容引导画面与
  旋转加载圈；完整图形桌面由 UEFI 启动提供。
- UEFI 继续使用 `\EFI\BOOT\BOOTX64.EFI`。

## 界面

- **任务栏移到顶部**：开始按钮、搜索框、固定应用与系统托盘全部位于顶部任务栏。
- **开始按钮改为系统 LOGO**：直接绘制 `LOGO_RGBA`，不再使用四格方块。
- **开始菜单**：从顶部任务栏下方展开，底部显示 `HYWOS 1.1.6`。
- **桌面图标**：自动下移到任务栏下方，避免遮挡。
- **窗口**：初始位置、最大化、拖动边界均按顶部任务栏重新计算。
- **启动动画**：保留 LOGO 缩放淡入与 `HYWOS` 字标，底部加载点改为旋转加载圈。

## 输入修复

- 相对指针（UEFI SimplePointer、USB HID、PS/2）加入约 2.4 倍增益，小位移保留
  精细控制；PS/2 单帧位移上限提高。
- 主循环轮询间隔由 12 ms 降至 8 ms，指针移动更跟手。

## 构建与验证

- 构建：`cargo build --release --target x86_64-unknown-uefi`。
- C/C++ 内核由 `build.rs` 交叉编译为 x86_64 COFF 对象并链接进 UEFI 映像；
  aarch64 构建机自动使用 `clang --target=x86_64-pc-windows-msvc`。
- QEMU 验证：
  - UEFI：OVMF（q35、e1000、PS/2）完成安装、启动动画与桌面；
  - BIOS：SeaBIOS 从混合 ISO 进入兼容引导画面与旋转加载圈；
  - 桌面、开始菜单、终端 `kernel` 输出均已截图确认。

## 资产

- `hywos.iso`：UEFI + BIOS 双引导安装镜像。
- `BOOTX64.EFI`：可直接放入 ESP 的内核。
- `bios.img`：BIOS El Torito 引导扇区。
- `hywos-1.1.6.updpkg`：在线/本地更新包。