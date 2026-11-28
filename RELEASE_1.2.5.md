# HYWOS 1.2.5 «输入修复版»

## 修复：退格 / 删除键误关闭应用

- 根因：全局关闭窗口条件 `scan == 0x0e` 同时匹配了退格键（Backspace）与 F4，
  导致在 AI 助手、HYWOffice 等应用中按退格/删除时直接关闭窗口。
- 修复：关闭条件改为 `scan == 0x3e || (scan == 0x0e && ch == 0)`：
  - 仅 F4（功能键，`ch == 0`）触发关闭；
  - 退格（`ch == 8`）不再关闭，交还给当前应用处理；
  - 删除键不再触发关闭。
- 应用内文本编辑恢复正常：AI 输入框、HYWOffice TXT 编辑、搜索框等可正常退格。

## 版本

- 版本号统一升级为 1.2.5。

## 资产

- `hywos.iso`：UEFI + BIOS 双引导安装镜像。
- `BOOTX64.EFI`：可直接放入 ESP 的内核。
- `bios.img`：BIOS El Torito 引导扇区。
- `hywos-1.2.5.updpkg`：在线/本地更新包。