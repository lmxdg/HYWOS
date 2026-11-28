# HYWOS 1.2.1 «安装与硬件版»

## 安装协议与磁盘选择

- 安装开始前显示 **隐私协议** 与 **用户协议**。
  - `Enter` / `Y` 同意并继续，`N` / `Esc` 取消。
- 同意协议后列出可安装磁盘（容量、可安装分区），支持 `↑` / `↓` 选择、
  `Enter` 开始安装、`N` / `Esc` 取消。
- 安装仍会清空所选磁盘的分区表与数据。

## 开发标识

- 版本与关于本机显示开发者：**MEMZ-LMXDG**。
- 安装协议中同步标注开发者。

## 硬件探测与诊断

- 新增 PCI 配置空间扫描：统计 PCI 设备、xHCI、UHCI、EHCI、PCIe 端口、
  NVMe、SATA、VGA。
- 终端新增 `sysdiag` 命令，显示：
  - ACPI MADT / FADT 状态；
  - CPU 逻辑核心数与启用数；
  - PCI/xHCI/UHCI/EHCI/PCIe/NVMe/SATA/VGA 统计；
  - 网络连接状态。
- 说明：本轮为**探测与诊断框架**，UHCI/xHCI 主机控制器数据传输、SMP AP 实际启动
  尚未实现。

## 网络

- 保留 e1000/82574L、RTL8139、virtio-net 与 DHCP/DNS/TCP/HTTP 栈。
- `sysdiag` 增加网络连接状态；现有 `net`、`ping`、`dhcp` 命令继续可用。

## 尚未完成

- **AI 助手**（本轮未选择，留待后续）。
- **显示缩放与分辨率运行时切换**（GOP SetMode 与全局缩放尚未接入）。
- UHCI/xHCI 完整驱动与 SMP AP 启动（当前仅 PCI 探测与 ACPI/MADT 拓扑）。
- 网络吞吐测试（当前为状态诊断）。

## 资产

- `hywos.iso`：UEFI + BIOS 双引导安装镜像。
- `BOOTX64.EFI`：可直接放入 ESP 的内核。
- `bios.img`：BIOS El Torito 引导扇区。
- `hywos-1.2.1.updpkg`：在线/本地更新包。