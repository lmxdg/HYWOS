# HYWOS 1.1.3 «星河 · 宏内核版»

## 宏内核架构

- 明确采用 **Monolithic Kernel**：显卡、输入、存储、网络、音频、图形/桌面、
  Ring3 与更新全部运行在 ring0 单地址空间，模块间通过内核 API 直接调用。
- 新增 `src/kernel.rs`：
  - 8 个子系统状态表：Video / Input / Storage / Network / Audio / UI /
    Ring3 / Updates，记录“就绪 / 降级 / 未就绪”与实现说明。
  - 统一内核 API：`cores`、`mem_stats`、`uptime`、`video_name`、`input_desc`、
    `audio_name`、`net_info`、`ping`、`windows`。
  - 统一系统调用入口 `kernel::syscall`：用户输出、退出、运行时间；
    Ring3 不再直接访问 `hw/net`，而是经内核系统调用层转发。
- 终端新增 `kernel` 命令，列出 8 个子系统状态与说明。
- 关于本机“架构”显示 `x86_64 · 宏内核 · N/8 子系统就绪`；
  安全中心底部显示同一状态行；保留 Ring3 与双页表。

## 集成改动

- 关于页的显卡/输入/音频/网卡信息改由 `kernel` API 提供。
- 安全中心的 CPU/内存/运行时间/网卡信息改由 `kernel` API 提供。
- 终端 `net`/`ping` 改走 `kernel::net_info` / `kernel::ping`。
- Ring3 `int 0x80` 统一由 `kernel::syscall` 处理输出与退出。

## 版本

- 版本号升级到 **1.1.3**（Cargo、横幅、状态栏、终端、关于、设置、安装器、构建脚本）。

## 实测

- QEMU 11.1.3 + OVMF：终端 `kernel` 显示 8/8 子系统就绪；
  关于本机与安全中心状态一致。
- `ring3` / `ring3f` 在新内核系统调用路径下仍正常：输出、退出、
  #PF 隔离与返回内核均无回归。

## 边界

- 宏内核为单地址空间、无用户态服务隔离；Ring3 仅用于运行受控用户程序与
  验证页表隔离，ELF 加载器与用户进程调度仍未实现。
