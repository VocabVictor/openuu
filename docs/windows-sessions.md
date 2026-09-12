# Windows 多用户会话与 OpenUU 被控端

调研日期：2026-09-13。基于 `master`（`bfdc512c0`）只读分析，未改代码、未构建。

痛点：远程控制 Administrator 时，别人用 RDP 登录 `alice`，远程画面跟着跳到 `alice` 的桌面。
目标：OpenUU 固定控制某一个会话（Administrator），RDP 登录其他账户互不影响，最好主控端可以选会话。

## 1. 现状机制

### 1.1 两种运行形态

| 形态 | 判定 | 谁负责抓屏 | 会话切换 |
|---|---|---|---|
| 服务模式 | `platform::is_installed()`（`src/platform/windows.rs:1866`，HKLM 里有 InstallLocation 且 exe 存在）+ 系统服务 `OpenUU --service` | 服务以 SYSTEM 用 `LaunchProcessWin` 在**某一个会话**里拉起 `OpenUU --server`（`windows.rs:814-845`） | 服务主循环决定，见 1.3 |
| 便携模式（当前用户级安装，无服务） | `is_installed()` 为 false | GUI 进程自己在进程内 `start_server(true, …)`（`src/server.rs:575`，`src/core_main.rs:205` → IPC 连不上就自起 server） | 不存在：进程活在启动它的那个会话里，永远只抓该会话 |

`--server` 进程的核心事实：

- 它由 `CreateProcessAsUserW` 用指定会话 ID 的 winlogon/explorer token 创建（`src/platform/windows.cc:232-273`），所以它**只能看到自己所在会话的桌面**。DXGI/GDI 抓的是本会话的 `winsta0`。
- 全机只有一条主 IPC 管道 `\\.\pipe\OpenUU\query`（`libs/hbb_common/src/config.rs:842-853`，无会话后缀）。第二个 `--server` 起不来会 `exit(-1)`（`src/server.rs:588-599`）。因此**同一时刻只能有一个会话被服务**。这是"一次只能控一个会话"的根本约束。
- 主 IPC 的授权（`src/ipc/auth.rs:622`, `:760-790`）要求对端进程要么是 SYSTEM，要么与 server 在同一会话。其他会话里的 GUI/tray 连不上，不会互相干扰。

### 1.2 会话 ID 的来源

`get_current_session(include_rdp)`（`src/platform/windows.cc:543-580`）：

- `include_rdp == false`：直接返回 `WTSGetActiveConsoleSessionId()`，即物理控制台会话。
- `include_rdp == true`：`WTSEnumerateSessions` 遍历 `WTSActive` 的会话；若控制台会话处于 Active 立即返回它；否则返回**枚举到的最后一个** `RDP-*`/`ICA-*` Active 会话。

`include_rdp` 来自注册表 `share_rdp`（`windows.rs:1053-1063`），缺省即 `true`。UI 在"设置 → 安全"里叫"共享 RDP"（`flutter/lib/desktop/pages/desktop_setting_page.dart:1447`），只在安装模式显示。

`get_available_sessions()`（`windows.rs:1151-1225`，底层 `windows.cc:641-690`）列出所有 Active 的 Console/RDP/ICA 会话，附用户名，供主控端选择。

### 1.3 服务主循环（会话跟随的实现）

`run_service`（`src/platform/windows.rs:672-812`）：

1. 启动时 `session_id = get_current_session(share_rdp())`，在该会话里拉 `--server`。
2. 每轮循环开头：只有当当前 `session_id` 已不在 Active 列表里、或 `share_rdp` 关闭时，才重新取 `get_current_session` 并换会话（`:698-712`）。
3. 等 IPC 300 ms（`SERVICE_INTERVAL`，`src/platform/mod.rs:37`）。收到 `Data::UserSid(Some(sid))` 就切到该会话，并记 `stored_usid = Some(sid)`（`:739-752`）。
4. **超时分支**（`:760-800`）：重新算 `tmp = get_current_session(share_rdp())`，若 `tmp != session_id` 且 `stored_usid != Some(session_id)`，则判定"会话变了"：对旧 `--server` 发 `Close`、在新会话里重新拉起。有端口转发连接时不切。
5. 切换 = 杀旧 `--server` + 起新 `--server`，正在进行的远程连接被断开，主控端自动重连后看到的就是新会话。

`stored_usid` 只在内存里，服务重启后丢失。

### 1.4 上游已有的"选择会话"支持（本 fork 已完整带入）

| 环节 | 位置 | 说明 |
|---|---|---|
| 被控端在登录响应里带会话列表 | `src/server/connection.rs:2155-2177` `handle_windows_specific_session` | 条件：`is_installed() && is_share_rdp() && 非端口转发连接数 == 1 && Active 会话数 > 1 && 主控端版本 ≥ 1.2.4`。满足才把 `pi.windows_sessions` 填上并暂停订阅视频（`wait_session_id_confirm`）。 |
| 协议 | `libs/base/protos/message.proto:138,141,815,860` | `PeerInfo.windows_sessions`、`Misc.selected_sid` |
| 主控端弹框 | `flutter/lib/models/model.dart:705` → `flutter/lib/common/widgets/dialog.dart:2412` `showWindowsSessionsDialog` | 下拉选会话，点"连接"发 `selected_sid` |
| 主控端记忆 | `src/ui_session_interface.rs:1625-1650, 1880-1892` | `lc.selected_windows_session_id` 仅进程内记忆；重连时若与被控端当前会话一致则直接确认、否则再弹框 |
| 被控端处理 | `src/server/connection.rs:3843-3870` | 目标 sid ≠ 当前 `--server` 所在会话 → `ipc::connect_to_user_session(Some(sid))`（`src/ipc.rs:1987-1992`）向服务发 `UserSid`，服务按 1.3 第 3 步切换；随后本连接结束、主控端重连到新会话。 |

结论：

- 功能完整、默认开启（`share_rdp` 缺省 true），**但只在服务模式可用**（三处都有 `is_installed()` 门槛）。
- 便携模式下整套逻辑不触发，也没有需要：进程只服务自己所在会话。
- "多 UI 会话"（`multi_ui_session`、`is_support_multi_ui_session`，`connection.rs:340, 4474`）是主控端一个远程分多窗口显示多屏的功能，与 Windows 登录会话无关。
- fork 自己的 `5f456df51 launch isolated read-only desktop sessions` 是主控端只读连接选项，与本议题无关。

## 2. RDP 抢占的根因

服务模式、`share_rdp = true`、主控端从未显式选过会话（`stored_usid == None`）时：

1. Administrator 若是通过 RDP 登录的（云主机常态），或控制台会话不处于 `WTSActive`，`get_current_session(true)` 返回的是"最后一个 Active 的 RDP 会话"。
2. `alice` 通过 RDP 登录后，多了一个更靠后的 Active RDP 会话，`get_current_session(true)` 的结果变成 `alice` 的 sid。
3. 300 ms 后超时分支发现 `tmp != session_id`，杀掉 Administrator 会话里的 `--server`，在 `alice` 会话重新拉起 → 画面跳走。
4. 主控端断线重连后此时 Active 会话数 > 1，才会第一次弹出选会话框；选了 Administrator 后 `stored_usid` 被设置，之后不再自动跳。但每次服务重启、或 `alice` 登录发生在只有一个会话时，都会先跳一次。

若 Administrator 在物理控制台且控制台 Active，`get_current_session(true)` 会优先返回控制台，不会被 RDP 抢走；此时抢占只会在 `share_rdp=false` 之外的其他原因（如控制台被锁到 Disconnected）发生。

另外注意 Windows 客户端版（10/11 Pro）只允许一个交互会话：别人 RDP 登录会把控制台会话强制断开，这不是 OpenUU 能解决的；本文假设目标机是 Windows Server 或允许多会话并存。

便携模式下不存在这个跳转：进程在哪个会话启动就抓哪个会话。但 Administrator 会话若是 RDP 会话且被断开（非注销），该会话没有显示设备，抓屏会得到"无显示器/黑屏"，这是 Windows 行为，服务模式同样受影响。

## 3. 可行方案

### 方案 A：零代码，用现有配置和选会话框

1. 以 MSI/服务模式安装（`res/msi/Package/Package.wxs` 为 `perMachine`，自定义动作 `CreateStartService` 建服务）。
2. 情况一：Administrator 在控制台登录 → 设置里**关闭"共享 RDP"**。`get_current_session(false)` 恒为控制台会话，RDP 登录永远不影响。缺点：无法远程控制任何 RDP 会话。
3. 情况二：Administrator 也是 RDP 登录 → 保持"共享 RDP"开启，第一次跳转后重连、在弹框里选 Administrator；之后 `stored_usid` 生效直到服务重启。

改动文件：无。风险：情况二仍会先跳一次，且服务重启后复现；选择只在主控端进程内记忆。

### 方案 B（推荐）：被控端持久化"固定会话"策略 + 主控端可主动选会话

思路：保留现有路径，加一个被控端选项，让服务主循环在有固定会话时不自动跟随；并让主控端在任何时候都能拿到会话列表重新选择。

| 改动点 | 文件 | 内容 |
|---|---|---|
| 新选项键 | `libs/base/src/config/keys.rs` | 例如 `OPTION_WINDOWS_SESSION_POLICY`：`follow`（现状）/ `console` / `pinned:<sid>`。或复用注册表 `share_rdp` 同一位置新增 `pinned_session`（读写走 `get_reg`/`run_cmds reg add`，与 `set_share_rdp` 同款，`windows.rs:1065-1073`）。 |
| 服务主循环 | `src/platform/windows.rs` `run_service`（`:694-712`, `:739-752`, `:760-770`） | 启动时从持久化值初始化 `stored_usid`；收到 `UserSid` 时写回持久化；超时分支的自动切换加一条"策略为 pinned 时且目标 sid 仍 Active 则不切"。目标 sid 消失（用户注销）时回落到现状逻辑。新增一个 `fn pinned_session_id() -> Option<u32>` 放在 windows.rs，主循环只加几行。 |
| 弹框触发条件 | `src/server/connection.rs:2160-2176` `handle_windows_specific_session` | 当前要求 `非端口转发连接数 == 1`。若希望第二个主控端也能选，需放宽或改为"仅当目标 sid ≠ 当前会话才走 IPC 切换"。第一版可不动。 |
| 主控端持久化选择 | `src/ui_session_interface.rs:1625` 附近、`PeerConfig`（`libs/hbb_common/src/config.rs`，是子模块，尽量避免） | 可选：把 `selected_windows_session_id` 存到 peer 的 `options` 里（`PeerConfig.options` 是 `HashMap<String,String>`，不改结构）。这样重连自动确认，不再弹框。 |
| 设置 UI | `flutter/lib/desktop/pages/desktop_setting_page.dart:1447` 附近 | 在"共享 RDP"旁加一个"固定到会话"下拉（列表来自新 FFI `main_get_windows_sessions`，`src/flutter_ffi.rs` 包一层 `get_available_sessions(true)`）。 |
| 远程工具栏 | `flutter/lib/desktop/widgets/remote_toolbar.dart` | 可选：加"切换会话"菜单，复用 `showWindowsSessionsDialog`；需要被控端在 `PeerInfo` 之外能回传会话列表（新增一个 `Misc` 请求，`libs/base/protos/message.proto`）。第一版可跳过，靠重连弹框。 |

行为：服务启动即在固定会话里起 `--server`；`alice` RDP 登录只是多一个 Active 会话，不触发切换；主控端连上就是 Administrator。想控 `alice` 时通过弹框/菜单选，选后固定值更新。

风险：

- 固定会话被注销后 sid 消失，需回落（已在设计内）。
- 固定会话是 RDP 会话且断开时抓屏黑屏，与现状一致，文档里要说明。
- 改动集中在 `windows.rs` 一个函数、`connection.rs` 一行条件、一个 FFI、一个设置控件；不碰 `hbb_common`。
- 便携模式不受影响（`is_installed()` 门槛照旧），也不会获得此功能。

### 方案 C：真正的多会话并发（每会话一个 `--server`）

让服务为每个 Active 会话各拉一个 `--server`，主控端连接时按 sid 路由，两个主控端可同时控不同会话。

需要打通：

- IPC：主管道名加会话后缀（`libs/hbb_common/src/config.rs:842` 在子模块里）；`src/ipc.rs` 所有 `connect(…, "")` 调用方要知道目标会话；`src/ipc/auth.rs` 同会话校验。
- 设备 ID/注册：`src/rendezvous_mediator.rs` 每台机只有一个 ID 与信令连接，多个 `--server` 会互相顶替。要么单进程做信令再按 sid 把连接交给对应会话（跨进程传 socket，Windows 需 `WSADuplicateSocket`），要么每会话独立 ID。
- 连接管理器 `--cm` 窗口、tray、剪贴板、文件传输等每会话一份；`src/server/connection.rs` 端口转发/权限/密码路径都要区分会话。
- 便携服务、`privacy_mode`、虚拟显示器等假设单实例的逻辑。

估计涉及 `src/ipc.rs`、`src/ipc/auth.rs`、`src/rendezvous_mediator.rs`、`src/server.rs`、`src/server/connection.rs`、`src/platform/windows.rs`、`src/core_main.rs`、`libs/hbb_common/src/config.rs`（子模块往返）以及 Flutter 主控端的会话选择入口。与 AGENTS.md"最小 diff"原则冲突大，回归面极广，不推荐。

## 4. 便携模式与服务模式分别说明

服务模式：

- 会话跟随、选会话弹框、`share_rdp` 开关全部可用。
- 痛点根因在 `run_service` 超时分支的自动跟随；方案 A 可立刻缓解，方案 B 彻底解决。
- 附带能力：登录界面/UAC/锁屏可控，开机即可连。

便携模式（当前用户级安装、无服务）：

- 进程绑定在启动它的会话，天然不会被 RDP 抢占；`alice` 会话里没有 OpenUU 实例（若 `alice` 也启动一份，会因主管道被占用而退出，`src/server.rs:588-599`）。
- 代价：无法切换到其他会话、锁屏/UAC/安全桌面不可控、Administrator 的 RDP 会话断开后无画面、用户注销即离线。
- `is_installed()` 为 false，方案 B 的所有新逻辑不会启用；若要在便携模式也支持"选会话"，需要 SYSTEM 权限跨会话拉进程，等价于服务模式。

## 5. 推荐

1. 立即：按方案 A 的情况一/二配置，验证痛点是否消失，同时确认目标机是 Windows Server（多交互会话）而非客户端版。
2. 第二阶段实施方案 B：`windows.rs` 的 `run_service` 增加持久化固定会话，`connection.rs` 放宽一行条件，Flutter 设置页加一个下拉；主控端选择持久化到 `PeerConfig.options` 作为可选项。
3. 方案 C 仅在明确需要"两个主控端同时分别控两个会话"时再评估。

## 6. 实施说明（方案 B，第二阶段）

### 6.1 机制

- 选项 `pinned-windows-session`（`libs/base/src/config/keys.rs` 的 `OPTION_PINNED_WINDOWS_SESSION`）存被控端 Config，值是 **Windows 用户名**而不是会话号（RDP 会话号每次登录都变）。空值 = 现状（自动跟随）。
- 逻辑集中在 `src/platform/windows/sessions.rs`：
  - `PinnedSession::resolve()`：最多每 5 秒直接读一次 Config2 文件里的该选项（服务进程与 `--server` 各自缓存 Config，只有读文件才能看到 GUI 改的值），要求"共享 RDP"开启，再把用户名匹配到当前 Active 会话（不区分大小写；同一用户既有控制台又有 RDP 会话时取控制台）。
  - 服务主循环（`run_service`）三处薄钩子：启动时用解析结果初始化 `stored_usid` 并直接在该会话拉起 `--server`；超时分支若解析结果与 `stored_usid` 不同则切换（或释放）；固定会话消失（用户注销）时打 warn、清 `stored_usid`，退回上游自动跟随。用户再登录后 5 秒内自动切回。
  - `pin_session_from_selection`：主控端在"多个 Windows 会话"弹框里选定会话时，`--server` 把该会话的用户名写回选项，与设置页保持一致；选中无人登录的会话则清空固定值。
- 主控端设置页：`flutter/lib/desktop/widgets/pinned_session_setting.dart`，挂在"安全"卡片"允许 RDP 会话共享"之下。会话列表来自 `main_get_common("windows-sessions")`（JSON：`user`、`name`），写入走已有的 `main_set_option`。便携模式显示但禁用，并提示需安装为服务。
- 未设置该选项时，`run_service`、`connection.rs` 的既有分支行为与上游完全一致。

### 6.2 使用步骤

1. 被控机以 MSI/服务模式安装 OpenUU，保持"允许 RDP 会话共享"开启。
2. 在被控机（或远程连上后）打开 设置 → 安全 → "固定被控会话到 Windows 用户"，选择 `Console: Administrator` 或 `RDP-Tcp#N: Administrator`。
3. 之后无论谁通过 RDP 登录其他账户，主控端看到的始终是 Administrator 的会话；主控端连接时不再弹出选会话框（若弹出并选择了其他会话，固定值会随之改为该用户）。
4. 要临时控制 alice：在下拉里选 alice，或在连接弹框中选 alice；改回 Administrator 同理。
5. 选"跟随当前活动会话（默认）"即恢复上游行为。

限制：固定的用户未登录时退回自动跟随（日志有 warn）；被固定的 RDP 会话若被断开（非注销）会黑屏，属 Windows 行为；主控端一侧不持久化选择（`PeerConfig.options` 方案本阶段未做）。
