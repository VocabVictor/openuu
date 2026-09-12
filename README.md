# OpenUU

OpenUU is a remote desktop client with a Windows-focused interface built with Flutter and Rust.

[中文](docs/README-ZH.md) · [Server](https://github.com/VocabVictor/openuu-server)

## Features

- Adaptive desktop home, settings and account login.
- Remote control, file transfer and session recording through the existing core.
- Self-hosted device rendezvous and relay with OpenUU Server.

## Source layout

- `flutter/lib/desktop`: desktop interface.
- `flutter/lib/common`: shared interface and login.
- `src`: native remote desktop core.
- `libs`: shared libraries and platform integrations.

## Build

Initialize dependencies with `git submodule update --init --recursive`.
The current Windows UI was built with Flutter 3.24.5 and Visual Studio 2019.
Build the native core and generated Flutter bridge before running `flutter build windows --release` from `flutter/`.
The native library currently retains its compatibility filename `librustdesk.dll`.
A UI-only build using a prebuilt core does not rebuild native services.

## License and attribution

See [LICENSE](LICENSE) and [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
