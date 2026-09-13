# 移动端现状盘点（2026-09）

用户明确要保留 Android / iOS，而今天的 UI 工作全在桌面。本文盘点移动端现在处于什么状态、
今天的改动波及了什么、以及它自己欠着什么。**只盘点，不改造**；文中三处"已修"是盘点过程中
发现的、由今天的桌面改动引入的回归，属于修自己的错，不是移动端改造。

所有结论都经过命令行核实，核实方式随结论给出。一条被子代理报告但**经实验证伪**的结论也保留在
§2.1，因为它值得记住。

## 1. 编译可行性（静态判断，未执行任何构建）

结论：**Android 没有被拆掉，只是没人在编。** 代码侧完整且被今天的重构正确带着走，
缺的是把 Rust 核心编成 `.so` 再塞进 APK 的那套自动化。

### 1.1 Flutter 侧齐全

`flutter/android/` 完整：Gradle wrapper 8.11.1、AGP 8.10.1、Kotlin 插件 2.1.21、
compileSdk/targetSdk 36、minSdk 22。`AndroidManifest.xml` 是维护中的状态而非遗留：
MediaProjection / 麦克风 / specialUse 前台服务类型、无障碍 `InputService`、`MainService`、
`FloatingWindowService` 都在，deep link scheme 已经是本 fork 改过的 `openuu`（`e2af77f03`）。
Kotlin 源码 11 个文件加 JNI 门面 `ffi.kt` 全在。

`flutter/android/build.gradle:1-51` 还有本 fork 为 AGP 8 加的兼容层：给五个老插件注入
`android.namespace`，并强制三个模块 JVM target 1.8。这是"有人让它编过"的证据。

`pubspec.yaml` 没有平台锁定；桌面专用插件在 Android 上不注册也不阻塞构建。
`flutter/ios/` 也在（Runner.xcodeproj、Podfile、AppDelegate.swift 俱全），但 `build_ios.sh`
里硬编码了本仓库不存在的 patch 路径，属于"随仓库带着、无人维护"。

### 1.2 Rust 侧的 android 分支没有被今天的拆分波及

`libs/scrap/src/android/`（`lib.rs:23-24` 声明，`ffi.rs` 导出 8 个 JNI 入口）与
`src/flutter_ffi.rs:2894` 的 `pub mod server_side`（另外 8 个）合计 16 个导出，与
`flutter/android/app/src/main/kotlin/ffi.kt:15-30` 的 16 个 `external fun` **一一对应，无漂移**。

今天的三次大拆分都没有破坏它：base crate 抽取后 `android/ffi.rs:12` 正确改指
`base::message_proto`；输入分支移出 `Connection::on_message` 后 `input_msg.rs` 靠父模块的
`use super::*` 继承了 android/ios 的 cfg 导入；clipboard 拆分保留了三处 android 分支。

反向证据：`build.rs:39-46` 今天 12:27 才加了 `build_android_ifaddrs()`，注释写明 bionic 从
API 24 才导出 `getifaddrs` 而 jniLibs 按 API 21 编译——近期确实有人链过 Android 版新核心。

### 1.3 缺的是构建自动化

* **Gradle 不编 Rust**。`app/build.gradle` 里唯一一处调 `cargo` 是 `cargo metadata`，
  用来定位 `rustls-platform-verifier-android` 的 maven 目录；这意味着 Gradle **配置阶段**
  就硬依赖 PATH 上有 `cargo`，找不到直接 `throw new GradleException`。
* 编 `.so` 的脚本还在（`flutter/ndk_*.sh` 四个，走 `cargo ndk --platform 21`），
  依赖先跑 `flutter/build_android_deps.sh` 编 vcpkg 的 android triplet。都是 bash，
  假定 Linux 工具链布局，Windows 上要走 WSL。
* **CI 里已经没有 Android 构建**。`playground.yml:233-406` 留着一个完整的 android job
  （NDK r26d、cargo-ndk、拷 `.so` 进 jniLibs、`flutter build apk --split-per-abi`），
  但 `schedule:` 已被注释掉，只剩手动触发。今天删掉的 `flutter-build.yml`（`25a1f4974`）
  核对过内容，**不含任何 android/apk/ndk 字样**，所以今天没弄丢 Android CI；带 Android 的
  nightly/tag 工作流是更早转 Windows-only 时删的。
* `build.py` 里 android/ndk/apk 出现次数为 **0**。

### 1.4 判定

`flutter build apk --debug` 在装了 Android SDK 36 + JDK 17 + PATH 有 cargo 的机器上
**大概率能出包，但那个包一启动就崩**：`ffi.kt:11-13` 在 object 初始化里
`System.loadLibrary("rustdesk")`，而没有任何环节把 `librustdesk.so` 放进 `jniLibs/`（该目录 gitignore）。

要拿到能用的 APK 必须手工补三步：`build_android_deps.sh` 编 vcpkg 依赖
（`build.rs:64` 对 `VCPKG_ROOT` 是 `unwrap()`，没设就 panic，这是第一堵硬墙）→
`ndk_arm64.sh` 编 `.so` → 手工拷进 `jniLibs/arm64-v8a/` 并从 NDK sysroot 拷 `libc++_shared.so`。
前两步在 Windows 上要走 WSL。

次要风险：`build.gradle:31` 仍列着 2021 年停用的 `jcenter()`；`app/build.gradle:137` 把
kotlin-stdlib 强锁 1.9.10 而插件是 2.1.21；没有 pin `ndkVersion`；
`generated_bridge.dart` 是 gitignore 的，干净克隆需先跑 `flutter_rust_bridge_codegen` 1.80.1。

## 2. 今天的改动对移动端的影响面

### 2.1 一条被证伪的结论，值得记住

盘点时子代理报告"今天的拆分留下 16 个无法解析的 `import '../../consts.dart'`，移动端无法编译"，
并列出了 14 个 `flutter/lib/common/*.dart` 加两处。路径推演看着成立：`flutter/lib/common/` 上溯
两级是 `flutter/`，而 `flutter/consts.dart` 确实不存在（`ls` 与编译机 `Test-Path` 都确认）。

但今天每一轮 `flutter analyze` 都是 0 error。两者必有一错，于是做了判决性实验：
把 `branding.dart:9` 改成一个确定不存在的 `'../../zzz_nonexistent.dart'` 推上编译机分析——
立刻报 `error - Target of URI doesn't exist ... uri_does_not_exist`，另外两条 `undefined_identifier`
一并出现。换回原样则干净。

**所以 analyze 确实覆盖该文件，而 `'../../consts.dart'` 是能解析的**：lib 下的文件按
`package:flutter_hbb/...` 解析，超出包根的多余 `..` 被 URI 规范化吞掉，最终落到
`package:flutter_hbb/consts.dart` = `flutter/lib/consts.dart`。

结论：这 16 处**不是编译错误**，只是多写了一级 `..` 的不规范写法，可在顺手时收敛，不紧急。
教训是：与 `flutter analyze` 的结论冲突时，先做能证伪的实验，不要让路径推演压过工具事实。

### 2.2 删除与拆分：对移动端无影响

Web 平台今天被整体删除（`flutter/lib/web/*`、`web_model.dart`、`printer_model.dart` 等），
Sciter UI 也删了。`grep` 移动端对 `web_model|kIsWeb|isWebDesktop|printer_model` 的引用为 **0**。
`6216f50b2` 只折掉了 web-desktop 分支，`isMobile`/Android/iOS 分支全部保留。

今天触及 `flutter/lib/mobile/` 的提交有 **43 个，其中 33 个是 `refactor(mobile)`**——移动端在
**结构上**今天被大量整理（拆到 300 行规则以内），并非无人照看。所有被拆的共享文件都在原路径
留了 barrel，移动端的 import 路径仍然解析；移动端 64 个 dart 文件里只有
`settings_page/settings_state.dart` 仍超 300 行。

### 2.3 三处由今天的桌面改动引入、影响移动端的回归（**已修**）

这三处都是我把会话对话框搬到 `UiDialog` 时引入的。共同根因：`UiDialog` 是照桌面固定宽度、
固定行布局设计的，而它承载的对话框由 `FfiModel.handleMsgBox` 打开，那条路径**与平台无关**，
手机上同样会弹。

| 问题 | 后果 | 修复 |
| --- | --- | --- |
| 2FA 的 OK 按钮被我移出了 `Obx` | 验证码输完按钮**永不启用**，Enter 也随之失效；桌面移动端都中招 | `98b6185bf`：输入回调同时 `setState` |
| `UiDialog` 写死 `minWidth: 352` | 手机可用宽度约 280dp（Material inset 40×2 + 内容 padding 24×2），必然溢出；原 `CustomAlertDialog` 只设 `maxWidth: 500`，是自适应的 | `ae9482116`：桌面保持固定，其余平台改为上限 |
| 按钮行用固定 `Row` | `AlertDialog` 原本用 `OverflowBar`，窄时自动竖排；换成 `Row` 后只会溢出。中继提示有三个按钮，正是该情形 | `741ca0d9b`：改回 `OverflowBar` |

2FA 那条是功能阻断且影响桌面，所以必须修；另两条当前只影响移动端，而移动端眼下根本跑不起来
（§1.4），实际影响为零，但修复都是一行、零风险，留着是定时炸弹。

### 2.4 已知但**不**改的一处：暗色模式

`UiColor`（`desktop/widgets/ui_tokens.dart:127-152`）是硬编码的亮色调色板，`UiDialog` 与
`ui_fields.dart` 还直接用了 `Colors.white` 做按钮与输入框底色。移动端有实时暗色主题，
桌面也有，所以暗色模式下这些对话框会是暗底上的深色文字。

这不是今天引入的，而是整个 token 体系的既有状态——设置页、主窗口的 token 化同样只有亮色。
已记在 `docs/backlog.md` 的"Dark-mode tokens are missing"，前置条件是先定夺暗色是否为受支持外观。

### 2.5 其余共享组件：无风险

`msgbox.dart` / `msgbox_parts.dart` 是纯抽取，`CustomAlertDialog` 仍保留移动端分支
（`isAndroid` 时开软键盘），`MyTheme` 的三个 dialog padding 仍按 `isDesktop` 分叉。
`peer_card/` 是机械拆分，`isPortrait` / `isMobile` 分支完整。登录框上 `UiDialog` 的那条路径
被 `login_dialog.dart:241` 的 `if (isWindows)` 挡住，移动端走的仍是旧对话框；唯一的行为变化是
`_LoginPrefs` 现在在移动端也会实例化并读写三个本地选项，默认值使其成为空操作。
`ui_fields.dart` 引用了 `desktop/widgets/` 两个文件，但两者都只依赖 `material.dart`，
不会把桌面专用代码拖进移动端构建——是分层味道，不是编译风险。

## 3. 移动端自身的待办

### 3.1 设计 token 一次都没用上

`grep -rl "UiColor\|UiSpace\|UiType" flutter/lib/mobile` 的结果是 **0 个文件**。桌面端今天走完了
设置页、主窗口、会话窗口三轮 token 化，移动端一个调用点都没有。两端的视觉语言已经分叉：
同一个概念（行高、次要文字色、危险色）在两边是两套值。

### 3.2 移动端连接管理面板违反信任界面规则（当前就成立）

`mobile/pages/server_page/connection_manager.dart:97` 与 `:120`，新连接请求与语音通话请求两处：
**拒绝是 `TextButton`（无边框纯文字），接受是 `ElevatedButton.icon`（实心带勾）**。

`docs/cm-restyle-plan.md` 对信任界面的约束是"接受与拒绝视觉上可区分且同等可达，拒绝保留真实边框
和全尺寸命中区，两者都不能沦为纯文字链接"。移动端正好踩中最后一条，而触摸屏误触代价比桌面更高。

桌面同一决策已经改对：`desktop/pages/server_page/cm_control_panel.dart:60-86` 里 Accept 是主色填充、
Cancel 是白底加 `UiColor.inputBorder` 边框，两者包在 `Expanded` 里等宽。该规则文档写的是桌面 CM 窗口，
移动端不在其范围内，所以这是本次盘点新发现的缺口。

### 3.3 扫码导入已经接上新 provisioning（不是缺口）

今天 52 的 `cefac684c` 新增了 `flutter/lib/common/config_import.dart`：`isConfigPayload` 同时认
`openuu://config/…` 与 legacy `config=`，`confirmImportConfig` 先调 `bind.mainPreviewConfigText`
出预览再由用户确认，确认后 `bind.mainImportConfigText`，与桌面 provisioning 走同一对 FFI。
首次启动无服务器配置时 `home_page.dart:61` 会直接打开扫描页。

顺带一个观察：这个 helper 目前只有移动端在用，桌面的
`desktop_setting_page/network_provision.dart:97` 自己又写了一份预览逻辑，调的是同一个 FFI。
两端各实现一次同样的预览/确认流程，是可以合并的重复，不影响功能。

### 3.4 文件尺寸规则的记录是准确的（订正：此处原写"已过时"，是我算错了）

初稿说 `mobile/pages/settings_page/settings_state.dart` 现在 1020 行而 `AGENTS.md` 记的是
"约 770 行"、记录已过时。**这是错的**：我把文件行数当成了方法行数。逐字符数括号层级量过，
`_SettingsState.build` 实际跨 221–991 行，**771 行**，与 Tracked exceptions 记的"~770"吻合。
文件 1020 行里另外 249 行是该类的字段与其它方法。记录无需更新。

### 3.5 其它未 token 化的移动端界面

远控手势与浮动鼠标（`mobile/widgets/floating_mouse/`、`gesture_help/` 合计 1625 行，全自绘）、
远控页动作栏与按键帮助（`actions.dart` 168 行、`key_help_tools.dart` 224 行）、
连接页（`connection_page.dart` 165 行）。都能工作，只是与桌面新视觉无关联。

## 4. 结论与建议优先级

**一句话**：移动端的代码是活的、结构今天还被整理过，但它**不可构建**（缺 `.so` 自动化），
而且有一处当前就成立的信任界面缺陷。

| 优先级 | 事项 | 理由与前置 |
| --- | --- | --- |
| P0 | 移动端 CM 的拒绝按钮改成带边框的全尺寸按钮 | 信任决策，触摸屏误触代价高；桌面已有现成写法可抄（`cm_control_panel.dart:60-86`）。改动小，不需要能构建就能评审 |
| P1 | 决定移动端是否要恢复可构建 | "保留移动端"目前是一句空话：没有 `.so` 就没有可用的包。最小路径是把 `playground.yml:233-406` 那个 android job 恢复成定时或手动可用，而不是重写。**需要先定夺是否值得占 CI 额度** |
| P2 | 三处 `UiDialog` 回归的验证 | 已修（§2.3），但修复**只经 analyze 验证，没有在真机或模拟器上看过**；等移动端可构建后补一次目视确认 |
| P3 | 暗色 token | 见 backlog，前置是先定夺暗色是否为受支持外观 |
| P4 | 移动端 token 化 | 工作量与桌面三轮相当，收益是视觉统一；建议排在移动端可构建之后，否则改了也看不见 |
| P5 | `AGENTS.md` 的 `settings_state.dart` 行数更新、16 处多余 `..` 的收敛、两端配置预览逻辑合并 | 都是顺手可做的整理，无前置 |

**一条过程教训**（已记入 `docs/changelog-2026-09-13.md` 的 Defects）：把共享对话框搬上新外壳时，
我只按桌面尺寸设计，忘了那条 `handleMsgBox` 路径与平台无关。今后动 `flutter/lib/common/` 下的
任何组件，都要先问一句"移动端会不会也走到这里"。
