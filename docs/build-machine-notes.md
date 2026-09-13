# 编译机上的坑与约定

这台机器是几个会话共用的，下面每条都是踩过一次之后记下来的。改动共享环境之前先在群里说一声。

## 跑 `flutter test` 必须临时清掉代理变量

机器上设了 `ALL_PROXY=http://127.0.0.1:1080`（还有同族的其它几个）。
`flutter test` 会起一个本地 HTTP 服务、让测试进程从回环连回来，代理把这条连接吃掉，
于是套件**根本加载不起来**，报的是

```
HttpException: Connection closed before full header was received, uri = http://127.0.0.1:<port>
Failed to load "...": Connection closed before test suite loaded.
```

这看着像测试坏了，其实一个测试都没跑。**不要去动机器上的代理变量**——它是共享的，
别人可能靠它出网；只在自己的命令里清空这六个：

```powershell
$env:ALL_PROXY=""; $env:all_proxy=""
$env:HTTP_PROXY=""; $env:HTTPS_PROXY=""
$env:http_proxy=""; $env:https_proxy=""
flutter test
```

清干净之后全套 147 条大约 9 秒跑完。CI runner 上没有代理，所以那边不受影响；
`linux-check.yml` 里的那一步仍然显式清了这几个变量，防的是以后有人给 runner 加代理。

## WSL 里的东西各有主人

WSL Debian 里同时住着两套东西，别互相踩：

* **rustup 默认工具链是 `1.75`**，Linux 检查（`C:\build\check-linux.ps1`）拿它编译，
  与 GitHub 上 Linux workflow 的 `RUST_VERSION` 对齐。**不要 `rustup default`、
  `rustup update` 或换默认**——那会让检查悄悄换编译器版本，不报错，只是结果和 CI 对不上。
  需要别的版本就 `rustup toolchain install <ver>` 再显式 `cargo +<ver> ...`，那是加法，安全。
* **`/root/build/vcpkg` 是 Linux 检查的 vcpkg**，pin 在与 CI 相同的 commit 上。
  不要往里装别的 triplet。需要 Android 的 vcpkg 依赖时另起一份，别共用。
* **`/opt/android-ndk-r26d`**（约 3 GB）是为 Android 交叉编译装的，rust 端对应
  `1.92.0` 上的 `aarch64-linux-android` target。都是加法：装完核验过 `rustup default`
  仍是 `1.75-x86_64-unknown-linux-gnu (default)`，Windows 侧的 PATH、rustup、vcpkg
  和 `check.ps1` / `build-flutter.ps1` 的行为都没变。

卸载办法：`rm -rf /opt/android-ndk-r26d`，
`rustup target remove aarch64-linux-android --toolchain 1.92.0`，
`rustup toolchain uninstall 1.92.0`。

## 交叉编译 Android 不是只要 NDK

`cargo check --target aarch64-linux-android --lib` 会停在 `libs/scrap/build.rs`：
它用 bindgen 解析 `vpx_ffi.h`，所以**即使只是 check、不链接，也需要 vcpkg 的
android triplet 真的装好**（`flutter/build_android_deps.sh arm64-v8a`，约 30–60 分钟）。
`build.rs` 里对 `VCPKG_ROOT` 是 `unwrap()`，没设直接 panic。

也就是说"本地快速 check"并不快，首次投资与一轮 CI 相当。除非要迭代很多轮，
否则直接让 `android-build.yml` 跑一轮更划算。
