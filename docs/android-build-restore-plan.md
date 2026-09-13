# 恢复 Android 可构建：方案

依据 `docs/mobile-status-2026-09.md` §1：Android 的代码、清单、JNI 契约都完整，缺的是把
Rust 核心编成 `librustdesk.so` 并放进 APK 的那一步。本文给出恢复路径、代价与取舍，
**批准后再执行**。

## 1. 现在差什么

`flutter/android/app/build.gradle` 从不调用 cargo 去编库（它只用 `cargo metadata` 定位一个
maven 目录）。所以 `flutter build apk` 产出的包里没有 `jniLibs/<abi>/librustdesk.so`，
而 `flutter/android/app/src/main/kotlin/ffi.kt:11-13` 在 object 初始化里
`System.loadLibrary("rustdesk")` —— 包能装、一启动就 `UnsatisfiedLinkError`。

编库需要的东西仓库里都还在，只是没人调用：
`flutter/build_android_deps.sh`（vcpkg android triplet）、`flutter/ndk_arm64.sh` 等四个
`cargo ndk` 脚本、`flutter/build_android.sh`（打包驱动）。
`.github/workflows/playground.yml:233-406` 还留着一个完整可用的 android job，
只是首行 `if: false` 把它关掉了。

## 2. 三条路径

### 方案 A：复活 playground 里的 job（建议）

改动最小：把 `playground.yml` 的 android job 摘出来成
`.github/workflows/android-build.yml`，去掉 `if: false`，触发器设为 `workflow_dispatch`
（先不上定时）。它已经把整条链写全了：装 NDK r26d → `build_android_deps.sh <abi>` →
`cargo install cargo-ndk` → `ndk_arm64.sh` → 拷 `liblibrustdesk.so` 与 NDK sysroot 里的
`libc++_shared.so` 进 `jniLibs/arm64-v8a/` → `flutter build apk --release --split-per-abi`。

* **代价**：一次运行约 40–70 分钟（vcpkg 编 android 依赖是大头，首次无缓存可能更久），
  ubuntu runner，单 abi。按 GitHub 免费额度 2000 分钟/月算，每次约占 2–3.5%。
* **风险**：低。这段 YAML 是从上游 nightly 继承的、曾经跑通过的形态。
  主要不确定性是 vcpkg 的 android triplet 在当前 `vcpkg.json` 下能否一次编过。
* **不做的事**：不改 Gradle，不引入新构建系统。

### 方案 B：让 Gradle 自己调 cargo-ndk

在 `app/build.gradle` 里加一个 task，在 `mergeJniLibs` 前跑 `cargo ndk`。
好处是本地 `flutter build apk` 一条命令就能出可用包，不必手工摆 `.so`。
代价是 Gradle 配置期已经硬依赖 cargo（见 §1），再加编译期依赖会让任何一次
Android 构建都必须有完整 Rust + NDK + vcpkg 环境；对只想改 Dart 的人是负担。
且 `build_android_deps.sh` 是 bash，Windows 上仍要 WSL，Gradle 里包一层不会消除这点。

**不建议现在做**，除非确定会有人经常在本地出 Android 包。

### 方案 C：只在发布时手工出包

把 §1 的三步写成一个脚本，需要时在 WSL 里跑。零 CI 额度，但"能不能构建"这件事
仍然不会被任何自动化守住，下一次拆分再打破 android 分支时不会有人知道。
**只适合确定移动端短期不发布**的情况。

## 3. 建议

**选 A，且先只挂 `workflow_dispatch`。** 理由：一次点击就能回答"移动端还能不能构建"，
这正是当前缺的那个事实；不上定时就不会持续烧额度。跑通一次之后再决定要不要加
`push` 到 `master` 的触发，或者每周一次的 `schedule`。

配套两件小事：
* job 末尾上传 APK 作为 artifact，这样不必进 runner 就能看到产物大小与 abi 是否对。
* 按 `AGENTS.md` 的 Linux check 经验，给它也加一个 `guard` job：前置被跳过时必须红灯，
  否则额度或环境问题会让它静默变灰。

## 4. 执行后如何验证

只用命令行判定，不需要真机：

1. workflow 结束为 success，且 artifact 里有 `app-arm64-v8a-release.apk`。
2. 在 job 里加一步 `unzip -l` 断言包内存在 `lib/arm64-v8a/librustdesk.so` 与
   `libc++_shared.so` —— 这两个文件在，才说明这次构建真的解决了原来的空壳问题。
3. `flutter build apk` 的输出里没有 `Target of URI doesn't exist` 之类的 Dart 错误
   （移动端 Dart 侧今天已被 `flutter analyze` 覆盖，这一步只是兜底）。

真机安装与目视确认（含 `docs/mobile-status-2026-09.md` §2.3 那三处对话框回归）
仍需一台 Android 设备，排在 CI 跑通之后，不在本方案范围内。

## 5. 不在本方案内

* iOS：`flutter/ios/` 在，但 `build_ios.sh` 指向一个本仓库不存在的 patch，且需要 macOS
  runner，本轮不碰。
* 移动端 token 化、暗色 token：见 `docs/backlog.md`。
