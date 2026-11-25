# HYWOS 1.0.7 «星河 · 安装与在线更新版»

## 修复：安装后仍进入安装程序

- 安装介质检测改为**按当前启动卷判断**：只有从 ISO 的 El Torito 卷启动、
  且该卷带有 `\HYWOS\CD` 或 `\CDMARK.TXT` 时才进入安装程序。
- 安装成功后写入 `\HYWOS\INSTALL.TXT`。从本地硬盘启动时优先识别该标记，
  即使 ISO 仍连接、固件仍缓存 2048 字节块设备，也不会再次进入安装页面。
- 移除「任何存在 2048 字节块设备就判定为安装介质」的误判逻辑。
- **修复 UEFI 文件写入**：FatPkg 只接受 `READ`、`READ|WRITE`、
  `READ|WRITE|CREATE` 三种打开模式，旧代码使用 `WRITE` 导致卷被误判为只读；
  现在安装盘与用户卷均可正常创建、删除和替换文件。

## 安装流程重写

- ISO 启动后**先询问用户是否安装**，按 Enter 确认，按 N / Esc 取消。
- 确认后**先复制全部系统文件到本地磁盘**：
  - 内核 `BOOTX64.EFI` → `\EFI\BOOT\BOOTX64.EFI`
  - 系统资源 `WELCOME.TXT` → `\HYWOS\SYSTEM\WELCOME.TXT`
  - 版本信息 → `\HYWOS\VERSION`
  - 安装完成标记 → `\HYWOS\INSTALL.TXT`
- 文件与目录全部写完后，**最后才创建 FAT16 引导分区（BPB）和 MBR 引导记录**，
  不再把安装程序本身当作系统文件复制进硬盘。
- 安装进度条按「准备磁盘 → 复制全部系统文件 → 写入目录/FAT → 创建引导分区 →
  写入引导记录 → 完成」分阶段显示。

## 新增：设置中的在线更新

- `设置 → 软件更新` 新增「检查更新」：
  - 通过 UEFI HTTP 协议请求 GitHub Releases 列表
    `https://api.github.com/repos/lmxdg/HYWOS/releases`；
  - 解析 `tag_name`，与本地版本 1.0.7 比较；
  - 新版本可用时显示版本号与下载源，点击「下载并安装」拉取 `.updpkg`；
  - 固件不提供 HTTP/TLS 或网络未配置时，读取 `\updates\latest.json`，
    并扫描 `\updates` 目录中的 `.updpkg`（含 8.3 短名）作为离线回退；
  - 无指针设备时，设置窗口可用 `←` / `→` 切换页面，软件更新页按 `C` 检查、
    `D` 下载并安装；
  - 下载完成后写入用户卷 `\updates\hywos-<版本>.updpkg` 并应用更新，
    CRC32 校验失败会拒绝更新。
- 更新包携带完整内核，应用时替换 `\EFI\BOOT\BOOTX64.EFI` 并写入 `\HYWOS\VERSION`；
  不支持文件替换的设备回退到 LBA 64。用户文件、应用数据与设置始终保留。

## 版本号

- 版本统一升级到 **1.0.7**（Cargo.toml、安装程序、状态栏、关于本机、终端、
  ISO 构建脚本与更新包生成器）。