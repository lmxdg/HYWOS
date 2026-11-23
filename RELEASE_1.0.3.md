# HYWOS 1.0.3 — 星河 · 输入与图形修复版

本次更新聚焦输入稳定性、virtio-gpu 现代驱动与全新启动动画。

## 修复

- **PS/2 鼠标点击后回到初始位置 / 应用打不开**：
  旧实现仅以初始化应答判断 PS/2 鼠标是否存在，在只有 USB 指针的环境下误判，
  导致走 PS/2 分支但收不到数据，点击被吞掉。现在输入优先级为
  **绝对指针（Absolute Pointer）→ SimplePointer → PS/2**，并且 PS/2 鼠标
  初始化严格检查 ACK 与设备 ID，失败即回退。
- **PS/2 鼠标乱飞**：
  鼠标三字节包增加溢出位校验（bit6/bit7）与 100 单位异常跳变过滤；
  SimplePointer 分支限制单帧位移（±45）并丢弃 >150 的同步跳变；
  PS/2 分支同样限幅。光标不再因未同步的大位移跳到屏幕角落。
- **virtio-gpu 不被支持**：新增 **modern virtio-pci** 支持：
  解析 PCI vendor capability，找到 common/notify/isr/device 配置区，
  协商 `VIRTIO_F_VERSION_1`，通过 common cfg 设置队列地址与使能，
  通过 notify 区 MMIO 触发队列。legacy 与 modern 两条路径共用同一套
  2D 命令（GET_DISPLAY_INFO / RESOURCE_CREATE_2D / ATTACH_BACKING /
  SET_SCANOUT / TRANSFER_TO_HOST_2D / RESOURCE_FLUSH）。
  测试环境升级为 **QEMU 11.1.3**。当前 QEMU 11 + OVMF 把 64 位 BAR 分配在
  `0xC000000000`，应用侧队列通知未生效（命令超时），modern 协议代码已完整，
  仍需通过 UEFI PciIo.Map 建立映射后继续验证。

## 新增

- **现代键鼠驱动**：新增 `src/input.rs`，使用 UEFI
  `SimpleTextInputEx`（扩展按键）与 `AbsolutePointer`（绝对坐标），
  优先于旧版 ConIn / SimplePointer，并与 PS/2 组成回退链。
  `关于本机` 会显示当前实际使用的输入栈。
- **全新启动动画**：168 个粒子从四周螺旋汇聚，合成一个发光的 **O**；
  随后 O 收缩并移动到标题位置，H、Y、W、S 依次淡入，
  最终成为 HYWOS 中的那个 O，最后浮现中文副标题。
- 版本号统一升级至 1.0.3。

## 下载

| 文件 | 说明 |
|---|---|
| `hywos.iso` | UEFI x86_64 可启动镜像 |
| `BOOTX64.EFI` | 独立 UEFI 应用 |
| `hywos-1.0.1.updpkg` | CONLYUPD 测试更新包 |

## 测试环境

- QEMU 11.1.3（源码构建，x86_64-softmmu）
- OVMF 202308
- `-vga std` / `-device AC97` / `-device usb-tablet`（xHCI）
- `-device virtio-gpu-pci`（modern 协议已实现，映射受限）
- `-device usb-tablet` 提供绝对指针

已知限制沿用 1.0.2：无 TCP/IP 协议栈、单核协作式调度、FAT32、
无中文输入法、无可写卷时使用内存文件系统。