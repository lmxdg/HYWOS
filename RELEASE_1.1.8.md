# HYWOS 1.1.8 «混合内核 · 办公与壁纸版»

本次更新新增 HYWOffice 办公文档预览与 5 张内置桌面壁纸。

## HYWOffice

- 新增开始菜单应用 **HYWOffice**，支持打开：
  - `txt` 纯文本；
  - `docx` / `pptx`（ZIP 容器 + DEFLATE 解压 + XML 文本提取）；
  - `doc` / `ppt`（OLE 二进制兼容文本提取）。
- 文件列表自动扫描 `\home\user\Documents`（为空时回退 `\home\user`），
  只列出上述扩展名。
- 预览为只读文本视图，支持 `↑` / `↓` 选择、`Enter` 打开、`PageUp` / `PageDown` 滚动。
- 不计第三方 crate，自带 ZIP 目录解析、inflate（stored/fixed/dynamic Huffman）
  与 XML `<w:t>` / `<a:t>` 文本提取。

### 格式边界

- docx/pptx 仅提取正文文本，不保留字体、颜色、表格、图片与版式。
- doc/ppt 为二进制兼容提取，可能包含少量非正文串。
- 加密文档、扫描件与纯图片文档无法提取文本。

## 内置壁纸

- 新增 5 张内置壁纸：**深空、极光、山脉、海洋、日落**。
- 切换入口：设置 → 显示 → 桌面壁纸，单击缩略图或按 `1`–`5`。
- 当前壁纸持久化到 `\home\user\.apps\wallpaper.kv`，重启后保持。
- 壁纸由软件合成器实时绘制，不依赖外部图片资源。

## 版本与验证

- 版本号统一升级为 1.1.8：终端、关于本机、安装程序、锁屏、更新页同步。
- 构建：`cargo build --release --target x86_64-unknown-uefi`。
- 验证：QEMU 中打开 HYWOffice 预览 txt/docx/pptx 文本，切换 5 张壁纸并重启确认持久化。

## 资产

- `hywos.iso`：UEFI + BIOS 双引导安装镜像。
- `BOOTX64.EFI`：可直接放入 ESP 的内核。
- `bios.img`：BIOS El Torito 引导扇区。
- `hywos-1.1.8.updpkg`：在线/本地更新包。