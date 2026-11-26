# HYWOS 1.1.1 «星河 · 网卡与网络版»

## 新增：自研网卡驱动

- **Intel e1000 / 82574L**（8086:100E / 100F / 10D3 / 153A）
  - PCI BAR0 MMIO、总线主控、复位与链路初始化。
  - RX 32 描述符 / TX 8 描述符 DMA 环，2048 字节缓冲。
  - 从 RAL/RAH 读取 MAC，STATUS 读取链路状态。
- **Realtek RTL8139**（10EC:8139）
  - I/O BAR、软复位、MAC 读取、4 路 TX、8KB 环接收与 CAPR 更新。
- **virtio-net（legacy，1AF4:1000）**
  - 特性协商（MAC/链路），desc/avail/used 三页队列布局，
    根据队列大小计算 used ring 偏移，RX 32 缓冲、TX 描述符与通知。
- 驱动按 virtio-net → e1000 → RTL8139 顺序探测；全部失败时保留固件 SNP。

## 新增：最小网络栈

- ARP：请求/应答，记录网关 MAC。
- IPv4/ICMP：构造并校验 IP 头与 ICMP Echo，`ping <IP>` 等待回复。
- 默认静态配置：`10.0.2.15/24`，网关 `10.0.2.2`。
- 终端新增：
  - `net`：驱动型号、MAC、链路、IP/网关、收发包计数。
  - `ping <IP>`：ARP + ICMP 连通性测试。
- 关于本机新增“网卡”行；安全中心新增“网络”卡片（驱动与链路状态）。

## 版本与边界

- 版本号升级到 **1.1.1**。
- 浏览器与在线更新仍使用固件 `EFI_HTTP_PROTOCOL`（TLS）；内核 TCP/UDP 传输层不在本版本范围。

## 实测

- QEMU 11.1.3 + OVMF，`-netdev socket,udp` 对接测试对端：
  - Intel e1000（82540EM）与 e1000e（82574L）完成初始化，
    `ping 10.0.2.2` 收到 ARP 与 ICMP 回复。
  - virtio-net（legacy）在补齐 10 字节 virtio-net 头后完成 ARP/ICMP 与 ping。
  - RTL8139 驱动完成初始化、MAC 读取与发送；当前测试环境未完成收包验证。
- 无网卡时自动回退固件 SNP，界面显示“未绑定自研驱动”。
