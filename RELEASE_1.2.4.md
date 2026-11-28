# HYWOS 1.2.4 «AI 助手版»

## 任务栏控制中心

- 修复控制中心按钮点击无反应：
  - 打开控制中心时自动关闭开始菜单与右键菜单，避免点击被其他弹层抢占；
  - 点击面板外关闭，面板内按钮正确接收点击。
- 任务栏右侧新增 **控制中心图标**（滑块图标），位于时间/电池左侧。
- 面板继续提供亮度、音量、重启、关机与网络状态。

## AI 助手

- 开始菜单新增 **AI 助手** 应用（紫色星标图标）。
- OpenAI 兼容接口：
  - `C` 进入配置，`Tab` 切换 Endpoint / API Key / Model，`Enter` 保存；
  - 默认 Endpoint：`https://api.openai.com/v1/chat/completions`；
  - 默认 Model：`gpt-4o-mini`；
  - 配置保存到 `\home\user\.apps\ai.kv`。
- 对话：
  - 输入消息，`Enter` 发送，`Esc` 清空；
  - 通过 UEFI 固件 HTTP(S) POST 请求（不引入第三方 TLS 库）。
- 提示词与 skills 注入：
  - 每次请求自动附带系统信息与可用操作列表；
  - 支持动作：`[[OPEN:terminal|settings|files|office|browser]]`、
    `[[BRIGHTNESS:n]]`、`[[VOLUME:n]]`、`[[WALLPAPER:n]]`、
    `[[SCALE:100|125|150]]`、`[[SHUTDOWN]]`、`[[REBOOT]]`。
- 系统操作执行：解析 AI 回复末尾的动作标记并调用内核/桌面接口。

## 说明

- AI 需要用户自行配置有效的 OpenAI 兼容 API Key；未配置时会提示。
- 本轮未使用真实 API Key 做端到端网络验证，仅验证界面、配置与请求构造。
- UHCI/xHCI 完整驱动与 SMP AP 实际启动仍不在本轮范围内。

## 资产

- `hywos.iso`：UEFI + BIOS 双引导安装镜像。
- `BOOTX64.EFI`：可直接放入 ESP 的内核。
- `bios.img`：BIOS El Torito 引导扇区。
- `hywos-1.2.4.updpkg`：在线/本地更新包。