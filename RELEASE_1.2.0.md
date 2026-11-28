# HYWOS 1.2.0 «桌面全能版»

## BIOS 引导修复

- BIOS 兼容引导不再无限停在 `Loading` 转圈。
- 加载动画运行有限次数后停止，显示：
  - `BIOS compatibility boot finished.`
  - `Full desktop requires UEFI boot.`
  - `Press R to reboot, C for help.`
- 按 `R` 通过 8042 复位，按 `C` 显示引导器信息。

## 系统设置页

设置新增“系统”页：

- **音量**：AC'97 主音量 0–100%，`-` / `+` 按钮调节，立即生效。
- **亮度**：软件合成亮度 0–100%，立即生效。
- **显示缩放**：100% / 125% / 150% 选择并保存。
- **分辨率**：1024×768 / 1280×800 / 1920×1080 选择并保存。
- **电源**：重启、关机按钮。
- 偏好保存到 `\home\user\.apps\display.kv`。

## 软关机 / 软重启

- 优先调用 UEFI RuntimeServices `ResetSystem`（关机/冷重启）。
- 关机失败时回退 ACPI FADT PM1a_CNT（SLP_TYP=5 + SLP_EN）。
- 重启失败时回退 8042 复位。

## HYWOffice

- 打开文档时显示 HYWOffice 启动画面（LOGO、名称、旋转加载圈）。
- TXT 文件支持编辑：
  - `E` 进入编辑；
  - 字符输入、Backspace、Enter 换行；
  - `Ctrl+S` 保存到原文件；
  - `Esc` 退出编辑。
- doc/docx/ppt/pptx 仍为只读文本预览。

## 说明

- 分辨率与缩放选项已保存；实际模式切换与全局缩放应用将在后续版本接入。
- ACPI/PCIe/SMP/UHCI/xHCI 深度驱动、AI 助手与网络增强仍待后续版本实现。

## 资产

- `hywos.iso`：UEFI + BIOS 双引导安装镜像。
- `BOOTX64.EFI`：可直接放入 ESP 的内核。
- `bios.img`：BIOS El Torito 引导扇区。
- `hywos-1.2.0.updpkg`：在线/本地更新包。