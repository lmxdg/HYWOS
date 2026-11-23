# HYWOS 1.0.0 — Harmony of Stars

首个公开版本。HYWOS 是一个用 Rust `no_std` 编写的 64 位 UEFI 操作系统，
直接运行在固件之上，自带图形桌面、应用框架、文件系统与终端。

## 亮点

- **真 UEFI 启动**：PE32+ `BOOTX64.EFI`，El Torito ISO，QEMU + OVMF 实测。
- **星河开机动画**：星点螺旋汇聚、双层光晕、HYWOS 徽标淡入，约 90 帧。
- **64px 图标缓存**：256×256 画布 4× 超采样后降采样，Dock 48/42/34px 自适应，
  48–56px 图标由大图缩小，边缘更干净。
- **Launchpad**：6 列网格，260ms 渐暗，行列错峰缩放，名称延迟浮现，
  底部搜索框支持即时过滤、退格、回车启动与 Esc 关闭。
- **原生 Weather**：520×430，离线 5 日预报，5 城市，℃/℉ 持久化，
  太阳浮动与云层漂移，选中日期高亮；只出现在 Launchpad，Dock 保持 19 槽。
- **App Store**：6 个可安装应用卡片，安装状态写入应用 KV。
- **应用存储 API v3**：`appstore_get/set/del`，每个应用独立
  `.apps/<appid>.kv`，值上限 120 字符。
- **权限页**：Storage / Notify / Network，按应用持久化；
  关闭 Storage 后存储 API 真实拒绝。
- **截屏**：F12 或 Print Screen，24 位 BMP，自动编号，状态栏提示。
- **.updpkg 更新**：CONLYUPD 头 + CRC32 校验，只写目标 LBA（默认 64），
  不触碰用户分区；Files 双击直接跳到 Software Update。
- **终端**：27 条 Linux 风格命令，直接操作文件系统。
- **多核**：ACPI MADT + CPUID 拓扑，QEMU `-smp 4` 显示 `SMP 4/4 cores`。
- **双缓冲合成器**：圆角、渐变、抗锯齿、阴影、位图字体，全部自行实现。
- **零依赖**：不引入任何 crate，手写 EFI 协议绑定。

## 产物

| 文件 | 说明 |
|---|---|
| `hywos.iso` | 可启动 ISO（约 25MB） |
| `BOOTX64.EFI` | UEFI 应用（约 117KB） |
| `usb.img` | 64MB FAT32 用户卷（MBR 分区） |
| `kdisk.img` | 64MB 裸盘，用于 LBA 更新测试 |
| `hywos-1.0.1.updpkg` | 测试更新包（544 字节） |

## 运行

```bash
qemu-system-x86_64 -M q35 -m 512 -smp 4 \
  -bios /usr/share/edk2/ovmf/OVMF.fd \
  -drive file=hywos.iso,media=cdrom,readonly=on \
  -drive file=usb.img,if=none,id=usbdisk,format=raw \
  -device usb-storage,drive=usbdisk -device usb-tablet \
  -drive file=kdisk.img,if=none,id=kdisk,format=raw \
  -device virtio-blk-pci,drive=kdisk \
  -netdev user,id=n0 -device virtio-net-pci,netdev=n0 \
  -vga none -device ramfb
```

快捷键：`F1` Launchpad，`F2` Weather，`F3` Permissions，`F4` Store，
`F5` About，`F6/F8` Files，`F7` Terminal，`F12` 截屏，`Esc` 关闭窗口。

## 已知限制

- 无自有 TCP/IP 协议栈，网络仅展示固件 SNP 的真实 MAC/链路状态；
- 多核为拓扑真实 + 单核协作式调度，无 AP trampoline；
- USB 鼠标/存储依赖固件枚举（UEFI 应用标准做法）；
- 文件系统为 FAT32，非 ext4；
- 若固件未提供可写卷，自动启用内存文件系统，重启后数据丢失。

## 协议

MIT License。