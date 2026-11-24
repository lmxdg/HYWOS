# HYWOS 1.0.5 — 星河 · 安装修复版

本次更新修复安装器识别硬盘的问题，并把安装确认改为回车。

## 修复

- **读取不到硬盘**：
  - 枚举 Block I/O 时先对每个设备执行 `Reset`，修复部分固件/设备在首次访问前
    `MediaPresent=0` 导致的漏检；
  - 可安装容量门槛由 50MB 降到 20MB（`last_block >= 40000`），小盘可用；
  - 优先选择整盘（`LogicalPartition=0`），避免误选分区；
  - 未找到磁盘时在界面列出检测到的块设备：容量、块大小、只读、分区标志；
  - 支持 IDE / SATA / virtio-blk / NVMe，兼容 512 字节与 4096 字节（4Kn）逻辑块设备；
  - 安装前重复 ConnectController 并等待，设备延迟就绪也能识别；
  - 诊断行显示「就绪」状态。
- **确认方式**：安装确认页直接按 **Enter** 开始安装，按 `N` / `Esc` 取消。

## 安装流程

1. 添加一块 ≥20MB 的可写磁盘；
2. 用 `hywos.iso` 启动，进入命令行安装程序；
3. 按 Enter 确认，安装器写入 MBR + FAT16 ESP，并复制 `BOOTX64.EFI`；
4. 完成后移除 ISO，从硬盘启动。

## 验证

QEMU 11.1.3 + OVMF 202308：
- 64MB virtio-blk（512B 块）：识别 63MB，按 Enter 安装，从硬盘启动进入 HYWOS 1.0.5；
- 128MB NVMe（4Kn，4096B 块）：识别 127MB，MBR 分区起始块 256，
  FAT16 ESP 写入正确，安装后从 NVMe 启动进入 HYWOS 1.0.5。

## 产物

| 文件 | 说明 |
|---|---|
| `hywos.iso` | 安装介质 |
| `BOOTX64.EFI` | UEFI 应用 |
| `hywos-1.0.1.updpkg` | 测试更新包 |

MIT License。
