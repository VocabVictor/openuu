# OpenUU

基于 Flutter 和 Rust 的远程桌面客户端，当前重点适配 Windows 横屏电脑。

[English](../README.md) · [OpenUU Server](https://github.com/VocabVictor/openuu-server)

支持远程控制、文件传输和会话录屏；首页、设置和账号登录采用统一桌面界面。

## 开发与构建

先执行 `git submodule update --init --recursive`。
当前 Windows 界面使用 Flutter 3.24.5 和 Visual Studio 2019 构建。
先构建原生核心并生成 Flutter 桥接文件，再在 `flutter/` 下执行 `flutter build windows --release`。
原生动态库暂保留兼容文件名 `librustdesk.dll`；使用预编译核心的 UI 快速构建不会更新原生服务。

## 许可证与来源

参见 [LICENCE](../LICENCE) 和 [第三方声明](../THIRD_PARTY_NOTICES.md)。
