# HYWOS 1.0.2 — 星河 · 驱动版

本次更新加入寄存器级硬件驱动，并保留 1.0.1 的完整中文界面。

## 新增驱动

### 显卡

- **标准 VGA / Bochs VBE**：PCI `1234:1111`，通过 VBE I/O 0x1CE/0x1CF
  设置 1024×768×32，使用 PCI BAR0 的 LFB。已用 QEMU `-vga std` 实测，
  截图分辨率 1024×768。
- **virtio-gpu**：legacy virtio-pci（I/O BAR0），control virtqueue，
  实现 GET_DISPLAY_INFO、RESOURCE_CREATE_2D、ATTACH_BACKING、
  SET_SCANOUT、TRANSFER_TO_HOST_2D、RESOURCE_FLUSH 六条 2D 命令。
  代码完整；本次 QEMU 8.2 构建缺少 `virtio-gpu-device`，未能实测。
- **VMware SVGA II**：PCI `15AD:0405`，SVGA II 寄存器与 LFB，
  带 SYNC 刷新。当前 QEMU 固件只暴露 index 寄存器，SetMode 失败自动回退 GOP。
- 驱动探测顺序：virtio-gpu → SVGA → VBE → UEFI GOP。

### 输入

- **PS/2 键盘与鼠标**：直接操作 8042，初始化时禁用/启用扫描，
  轮询键盘 Set-1 扫描码（字母、数字、Shift、方向键、F1–F12、Enter、
  Backspace、Esc）与鼠标三字节包（AUX 位区分）。已实测：
  QEMU sendkey F2 能打开天气窗口。

### 音频

- **Intel AC'97**：PCI `8086:2415`，NAM/NABM 初始化、主音量与 PCM 音量、
  48kHz 采样率、Buffer Descriptor List + DMA，启动时播放三音和弦。
- **PC Speaker** 回退：PIT 通道 2。
- 已用 `-audiodev wav` 实测：生成的 WAV 中检测到启动和弦，
  峰值 14116/32767。

## 界面

- `关于本机` 新增显卡、输入、音频驱动信息，例如：
  `显卡 VBE 2.0 standard VGA` / `输入 PS/2 键盘与鼠标（8042）` / `音频 Intel AC'97`。
- 版本号统一升级到 1.0.2，状态栏与所有截图同步。

## 验证矩阵

| 项目 | 结果 |
|---|---|
| VBE 1024×768 | 通过（截图） |
| PS/2 键盘 | 通过（F2 打开天气） |
| AC'97 播放 | 通过（WAV 波形） |
| UEFI GOP 回退 | 通过 |
| virtio-gpu | 代码完成，环境缺设备 |
| VMware SVGA II | 代码完成，环境寄存器受限 |

## 产物

| 文件 | 说明 |
|---|---|
| `hywos.iso` | 可启动 ISO，UEFI x86_64 |
| `BOOTX64.EFI` | 独立 UEFI 应用 |
| `hywos-1.0.1.updpkg` | CONLYUPD 测试更新包 |

## 已知限制

- 无自有 TCP/IP 协议栈；
- 多核为拓扑真实 + 单核协作式调度；
- FAT32 文件系统；
- 无中文输入法；
- 固件无可写卷时使用内存文件系统。