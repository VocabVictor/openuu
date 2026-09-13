# 会话窗口（远控 / 文件传输 / 终端 / 端口转发 / 摄像头）新版样式方案

范围：只动会话窗口。主窗口/设置页归 52；共用件（tabbar_widget、msgbox_parts、dialog/*、ui_tokens）改动前先与 52 打招呼。
风格与 token：全部沿用 design-review.md 第 2 节与 ui_tokens.dart（UiSpace/UiColor/UiType），不新造数值；会话窗口需要的少量新增 token 加到 ui_tokens.dart 的独立 `UiSession` 段。

## 1. 盘点结论（需要重做的元素）

| 元素 | 现状（文件） | 问题 |
| --- | --- | --- |
| 会话 tab 条 | tabbar_widget/*（DesktopTab，高 28、图标 18、关闭 CircleBorder、无圆角/无 token，颜色在 TabbarTheme） | 上游原样；5 个会话窗口各自复制 icon/菜单主题（remote/view_camera 各一份 `_MenuTheme`，terminal 用 `CustomPopupMenuTheme`）；窗口操作按钮 27px 命中、关闭 hover 硬编码红 |
| 悬浮工具栏 | remote_toolbar/*（`_ToolbarTheme`：按钮 32、圆角 8、白色 SVG 铺纯蓝 `MyTheme.button`、`Material elevation 3`、菜单 Material3 MenuBar；拖拽手柄 20px） | 上游"蓝色药丸"风格，与纤细描边风格冲突；菜单项高/内边距与 design-review 的 menu token 不一致；两套菜单系统（MenuBar 与 mod_popup_menu）并存 |
| 连接状态 | 全部是对话框：`showLoading('Connecting...')`（overlay.dart，240 宽 + 圆形进度）、`QualityMonitor`（右上半透明黑块，200 宽）、只读横幅（body.dart 硬编码 #e8f0fa） | 无行内状态条；连接中/重连中把整个画布挡成一个居中小框 |
| 密码 / 2FA / 等待接受 | connect_dialogs.dart `_connectDialog`、two_factor_dialogs.dart、elevation_dialogs.dart `showWaitAcceptDialog`——都是 `CustomAlertDialog` + `dialogButton`（带 ✓/✕ 图标）、50px 大图标、21px 标题 | 未重做（52 改的是设置页的"设置密码"对话框，会话侧密码框仍是上游） |
| 断线重连 / 中继提示 | msgbox.dart（"Connection Error" + `_CountDownButton` Reconnect）、ffi_model_reconnect.dart `showRelayHintDialog`（硬编码 `Colors.green[700]` 按钮） | 上游；按钮带图标、颜色不在 token 内 |
| 文件传输 | file_transfer_layout.dart / page_transfer.dart 已是自研样式（但自带一套 #3979ff/#dce1e8 硬编码，字体 YaHei，两个按钮都叫"发送"） | 已重做但未走 token；头部工具（view_head_tools.dart，MenuButton 圆角 8、地址栏 cardColor）仍上游 |
| 终端 | terminal_sessions_page.dart 已自研（渐变标题栏、卡片 #dce0e5），terminal_page 本体上游 | 已重做但未走 token |
| 端口转发 | port_forward_page/tunnels.dart：绿色 #007F00 提示条、60px 行、ElevatedButton | 上游 |

## 2. 目标样式（对照 token）

- **Tab 条**：高 36（`sidebarItemHeight`），底部 1px `#E5E6EB`；tab 项左右内边距 12、图标 16、字 13/500、选中 `UiColor.text` + 2px 主色下划线（与设置页 tab 同款），未选中 `UiColor.muted`；关闭按钮 16px 命中 24、hover 底 `rgba(0,0,0,.06)`；窗口操作簇按钮命中 36×36、图标 14，关闭 hover 用 `#F53F3F` 12% 底不再反白；tab 溢出下拉走 menu token（项高 32、圆角 8、描边 #E5E6EB + 阴影）。TabbarTheme 加 height/radius/indicator 字段并由 token 喂值，主窗口不受影响（DesktopTabType 区分）。
- **工具栏**：由"蓝色药丸"改为**白底描边条**：容器白（暗色 `#22262c`）、1px `#E5E6EB`、圆角 8、阴影 `0 4px 16px rgba(0,0,0,.10)`；按钮 28×28 命中、图标 16、图标色 `UiColor.textSecondary`，hover 底 `#F7F8FA`，激活态（录制/通话/固定）主色 8% 底 + 主色图标，关闭按钮 hover 底 `#FFF0EF` 图标 `#F53F3F`；按钮间距 4，两端内边距 8；拖拽手柄 20 高、`drag_indicator` 14px、`UiColor.faint`。菜单统一到 design-review 的 menu token（项高 32、左右 12、圆角 8、当前值 ✓ 14px），MenuBar 与 mod_popup_menu 两套先只改样式不合并（合并留 P2）。
- **连接状态**：新增 `SessionStatusBar`（画布顶部内嵌 28 高条，白底描边，状态点 6px + 12px 文案："连接中… / 等待对方接受 / 已连接·直连 / 已连接·中继 / 已断开，3 秒后重连"），替代 `showLoading('Connecting...')` 的居中框（loading 只保留在文件传输/终端首帧）；`QualityMonitor` 改为 12px 等宽、白底描边卡、`UiColor.muted` 标签；只读横幅改为状态条右侧的"只读"tag（`tagHeight 20`）。
- **对话框**（密码 / 2FA / 等待接受 / 断线重连 / 中继提示 / 重启确认）：统一 `SessionDialog` 外壳 = `dialogContentWidth 352`、标题 15/600、无 50px 大图标（改 16px 行内图标 + 标题）、正文 13/400 `UiColor.textSecondary`、字段走设置文档 TextField(32) 规范、按钮组右对齐间距 8：Primary 仅确认位、其余 Secondary、危险（断开）Danger，全部**不带图标**；"记住密码"用 Switch 行（无"开/关"文字）；Reconnect 倒计时放在 Secondary 按钮文字里；中继提示的绿色改主色。`msgbox_parts.dart` 是全局共用，因此不改它，会话侧对话框改为调用新的 `SessionDialog`（新文件），逐个替换。
- **文件传输 / 终端**：把已自研样式中的硬编码色/字号/圆角替换为 token（主色 `#3979ff→UiColor.primary`、面板圆角 6→8、按钮 36→32、字体走 `UiType`），头部工具按钮改 Secondary 28 高、地址栏 TextField 28 规范；修掉"发送/发送"文案。
- **端口转发**：绿色提示条改状态条样式（成功色 `#00B42A` 点 + 文案），行高 60→48，按钮改 Secondary/Primary 规范。

## 3. 实施顺序（每步 1–3 文件 / ≤600 行 / flutter analyze 绿 / 一张截图）

1. `ui_tokens.dart` 加 `UiSession` 段（仅 token，先与 52 打招呼）→ 截图不需要。
2. 工具栏容器与按钮（theme.dart、icon_buttons.dart、toolbar_layout.dart）→ `--connect` 截图。
3. 工具栏菜单样式（theme.dart defaultMenuStyle、menu_buttons.dart）→ 截图。
4. 拖拽手柄（draggable_show_hide.dart、edge.dart）→ 截图。
5. 会话 tab 条（tabbar_theme.dart + tab_item.dart + action_buttons.dart；主窗口路径保持旧值）→ 截图。
6. `SessionStatusBar` + remote_page/body.dart 接入 + QualityMonitor 改样 → 截图。
7. `SessionDialog` 外壳 + 密码框（connect_dialogs.dart）→ `--connect` 到需密码的被控端截图。
8. 2FA / 等待接受 / 重连 / 中继提示 / 重启确认逐个切到 `SessionDialog`（每提交 1–2 个）→ 截图。
9. 文件传输头部工具与 token 化、终端页 token 化、端口转发页（各 1–2 提交）→ `--file-transfer` / `--terminal` / `--port-forward` 截图。

预计 14–18 个提交。截图方式：本机便携版 `--connect <id>`（画面走已登录配置）用 PowerShell 截当前窗口，不动虚机桌面。

## 4. 0f 批复补充（2026-09-13）

- 状态条"已连接"后 3 s 自动收起；鼠标移到画布顶部 8px 热区或状态变化时再出现（不常驻）。
- 断线重连不再弹框：状态条内显示"已断开，N 秒后重连"倒计时 + "立即重连 / 断开"两个按钮（Secondary / Danger）。
- 截图只截本机窗口（PowerShell 截当前进程窗口）；不动虚机桌面。

## 5. 实施结果（2026-09-13）

9 步全部落 master，每步 `flutter analyze` 均为 224 个问题（基线 227，无新增诊断）。

| 步骤 | 提交 |
| --- | --- |
| 1 `UiSession` token | 先于本表（`ui_tokens.dart`） |
| 2 工具栏容器与按钮 | `5853f5319` |
| 2b 激活态改主色淡底 | `39e12ebf6` |
| 3 菜单走 menu token | `5980a0f9a` |
| 4 拖拽手柄 | `cd9a8f279` |
| 5 会话 tab 项与窗口按钮 | `ea6beb38f` |
| 6 状态条组件 / i18n / 接入画布 | `641982bc2`、`d2f925cb4`、`d84c98abe` |
| 7 连接密码框 + 公共对话框控件 | `9bcbfcbc5` |
| 8 2FA 与等待接受 / 中继提示 / 断线重连 | `e05471593`、`f266873bf`、`a18401c4a` |
| 9 端口转发 / 文件传输 / 终端页 | `4242def67`、`0ec0a3088`、`50fdcf0b0` |

方案外的三项补充：

* 通用外壳做成了 `flutter/lib/common/widgets/ui_dialog.dart`（`UiDialog` + `UiDialogAction`），
  而不是只供会话使用的 `SessionDialog`，登录框等也用它；`dialogManager.show` 的 builder 要求
  `CustomAlertDialog`，因此 `UiDialog.alert(context)` 是它的正式入口，`build` 只是转调。
* 对话框里重复出现的控件抽成 `flutter/lib/common/widgets/ui_fields.dart`
  （`uiDialogField` / `uiDialogToggle` / `uiDialogText`）。
* 断线重连的联动靠 `SessionStatusRegistry`（按 peer id 找到打开中的状态条）：
  没有状态条的会话（移动端、其他窗口类型）仍走原来的对话框，改动不触及它们。

截图仍未出：笔记本上有用户自己打开的主窗口，此时启动便携版发 `--connect` 会被 IPC 投递到那个实例，
按"不打扰用户"的裁定推迟到该窗口关闭后再补。
