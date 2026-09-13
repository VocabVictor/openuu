# RustDesk Guide

## Project Layout

### Directory Structure
* `src/` Rust app
* `src/server/` audio / clipboard / input / video / network
* `src/platform/` platform-specific code
* `flutter/` current UI
* `libs/hbb_common/` shared with the server: rendezvous proto, sockets, `Config` core
* `libs/base/` (crate `base`) client-only: option keys, message proto, file transfer, platform code
* `libs/scrap/` screen capture
* `libs/enigo/` input control
* `libs/clipboard/` clipboard
* `libs/base/src/config/keys.rs` the single import path for all options

### Key Components
- **Remote Desktop Protocol**: Custom protocol implemented in `src/rendezvous_mediator/` for communicating with rustdesk-server
- **Screen Capture**: Platform-specific screen capture in `libs/scrap/`
- **Input Handling**: Cross-platform input simulation in `libs/enigo/`
- **Audio/Video Services**: Real-time audio/video streaming in `src/server/`
- **File Transfer**: Secure file transfer implementation in `libs/base/src/fs/`

`hbb_common` is a git submodule shared with the server, so changing it costs a
round-trip. Put client-only code in `libs/base` instead; it is a normal
workspace member. `base::config::keys` re-exports the handful of keys
`hbb_common` still reads, so callers get the whole set from that one path.

### UI Architecture
- **UI**: Flutter only (the Sciter UI was removed) - files in `flutter/`
  - Desktop: `flutter/lib/desktop/`
  - Mobile: `flutter/lib/mobile/`
  - Shared: `flutter/lib/common/` and `flutter/lib/models/`

## File Size Rule (mandatory)

OpenUU is an independent product; upstream RustDesk is never merged back, so
upstream file layout carries no weight.

* No source file (`.rs`, `.dart`, `.cc`, `.cpp`, `.py`) may exceed **300 lines**
  (blank lines and comments included). `src/lang/*.rs` translation tables,
  generated files (`bridge_generated*.rs`, `generated_bridge*.dart`) and the
  `libs/hbb_common` submodule are the only exceptions.
* A file that would cross 300 lines must be split into a directory module
  (`foo.rs` -> `foo/mod.rs` + `foo/<topic>.rs`; `foo.dart` -> `foo/foo.dart`
  + `foo/<topic>.dart`) grouped by responsibility, not by line count.
* Splits are mechanical: move code, keep names, re-export from the module root
  (`pub use`) so call sites do not change in the same commit. Behaviour changes
  and splits never share a commit.
* Every split commit must pass `cargo check --lib --features flutter` (the
  `flutter` feature is on by default) and `flutter analyze` with no new
  diagnostics.
* When a task touches a legacy file that is still over 300 lines, split that
  file first in its own commit, then make the change.

This rule overrides "Be minimally invasive" for the purpose of splitting
oversized files; it does not license unrelated refactoring inside the split.

### Commit granularity

Every change lands as a series of small, incremental local commits; one huge
commit (dozens of files, thousands of changed lines) cannot be reviewed.

* Splitting an oversized file is not "one file, one commit". Each commit moves
  out only 1-3 new files and changes roughly 300-600 lines. Commit the
  directory plus the `mod.rs` / barrel shell first, then move code block by
  block, and delete what is left of the original file in the final commit.
* Every commit must build (`cargo check` with the `flutter` feature, or
  `flutter analyze`) with no new diagnostics, so `git bisect` stays usable.
* The first line of the commit message says what moved (for example
  `refactor(client): move audio handler out of client.rs`); the body lists the
  moved items and any visibility or import changes that were required.
* Feature work follows the same rule: one logical unit per commit (a config
  key, a platform function, a UI component and a translation key are separate
  commits).
* Large commits that already exist are left as they are (no rebase, no amend);
  the rule applies from the next commit on.
* Several sessions share this working tree and its index. Stage only your own
  new files (`git add <new file>`; a pathspec commit does not pick up untracked
  files), then commit with an explicit pathspec, `git commit -m "..." --
  <your paths>`, which takes those paths from the working tree and ignores
  whatever else is staged. A bare `git commit`, `git commit -a` or
  `git add -A` sweeps other people's staged work into your commit. Check
  `git show --stat HEAD` afterwards.

### Tracked exceptions

A file whose size comes from a single function longer than 300 lines may stay
over the limit only when listed here with the reason. Reducing it is a separate
task with tests, never a line-count-driven rewrite inside a split commit. Add a
row when a split leaves such a file behind; remove it when the function is
broken up.

| File | Reason |
| --- | --- |
| `src/client/start_inner.rs` | single `_start_inner` function (~570 lines) |
| `src/client/io_loop/ui_msg.rs` | single `Remote::handle_msg_from_ui` function (~470 lines) |
| `src/client/io_loop/peer_msg.rs` | single `Remote::handle_msg_from_peer` function (~750 lines) |
| `src/server/terminal_service/proxy_open.rs` | single `TerminalServiceProxy::handle_open` function (~330 lines) |
| `src/server/video_service/run_loop.rs` | single `run(vs: VideoService)` function (~380 lines) |
| `src/flutter/invoke_ui.rs` | single `impl InvokeUiSession for FlutterHandler` block (~520 lines); a trait impl cannot span files |
| `src/ui_cm_interface/ipc_runner.rs` | single `IpcTaskRunner::run` function (~330 lines) |
| `src/ui_cm_interface/fs_handler.rs` | single `handle_fs` function (~320 lines) |
| `src/core_main/mod.rs` | single `core_main` function (~680 lines) |
| `flutter/lib/common/my_theme.dart` | single `MyTheme` class of static members (~380 lines); a Dart class body cannot span files |
| `flutter/lib/models/ab_model/legacy_ab.dart` | single `LegacyAb` class (~390 lines) made of `BaseAb` overrides; overrides cannot move into an extension |
| `flutter/lib/models/ab_model/ab.dart` | single `Ab` class (~520 lines) made of `BaseAb` overrides; overrides cannot move into an extension |
| `flutter/lib/desktop/pages/file_manager_page/view_head_tools.dart` | single `headTools` method (~460 lines) |
| `flutter/lib/desktop/pages/remote_page/remote_page_widget.dart` | `_RemotePageState` fields plus ~400 lines of `@override` lifecycle/build methods; overrides cannot move into an extension |
| `flutter/lib/mobile/pages/settings_page/settings_state.dart` | single `_SettingsState.build` override (~770 lines) |
| `flutter/lib/common/widgets/toolbar/controls.dart` | single `toolbarControls` function (~310 lines) |
| `flutter/lib/common/widgets/login/login_dialog.dart` | single `_openLoginDialog` function (~350 lines) |
| `flutter/lib/web/bridge.dart` | single `RustdeskImpl` class (~1930 lines) mirroring the generated bridge API one-to-one; callers reach it through the conditional import in `models/platform_model.dart`, which does not re-export the library, so extension members would be invisible on the web target and `flutter analyze` (native branch) cannot catch that |
| `src/ipc/handle.rs` | single `async fn handle` IPC request dispatcher (~430 lines) |
| `src/server/connection/start.rs` | single `Connection::start` function (~720 lines) |
| `src/server/connection/logon_response.rs` | single `Connection::send_logon_response_and_keep_alive` function (~360 lines) |
| `src/server/connection/on_message.rs` | single `Connection::on_message` function (~1200 lines) |
| `src/flutter_ffi.rs` | flutter_rust_bridge v1 single-file codegen input (`--rust-input`); splitting needs frb v2 or changes to every build script. New exported functions added here must be one-line forwards to the owning module; no logic lives in this file. |

### Linux check (mandatory for `cfg(linux)` / `cfg(unix)` changes)

The Windows build machines cannot see items gated on Linux or Unix, and the
first run of `.github/workflows/linux-check.yml` found seven split-induced
errors that had already landed on master. So: any commit that touches a
file containing `cfg(target_os = "linux")` / `cfg(unix)` items (or a module
only compiled there) is pushed to the `ci/linux-check` branch first and lands
on master only after that workflow is green; the workflow also runs on every
push to master as a backstop. Purely Windows files are not held to it. Quote
the run URL in the commit body or the landing report.

### Deferred: needs macOS CI

Files that only compile on macOS cannot be verified anywhere yet: OpenUU
ships Windows builds only, and no macOS runner is planned until a macOS
target exists. Linux-only files below are no longer deferred; they are split
under the Linux check rule above. macOS-only files stay as they are; do not
split them before a check exists:

* `src/server/uinput.rs`, `src/server/wayland.rs`, `src/server/rdp_input.rs`,
  `src/server/drm_capturer.rs`
* `src/platform/linux.rs`, `src/platform/macos.rs`, `src/platform/gtk_sudo.rs`
* `src/ipc/drm.rs` (Linux + `drm` feature), `src/ipc/auth/mod.rs` (Unix
  socket credentials); `src/ipc/fs/` was split under the Linux check
* `libs/scrap/src/wayland/`, `libs/scrap/src/x11/`, `libs/scrap/src/quartz/`
* `libs/clipboard/src/platform/unix/`, `libs/enigo/src/linux/`,
  `libs/enigo/src/macos/`
* `libs/scrap/src/common/drm_reader.rs`, `libs/scrap/src/common/drmtap_dl.rs`
  (Linux + `drm` feature), `libs/scrap/src/android/ffi.rs`,
  `libs/base/src/platform/linux/wayland_probe.rs`, `src/whiteboard/linux.rs`,
  `src/whiteboard/macos.rs`

`libs/scrap/examples/benchmark.rs` is a sample, not product source, and is
left as it is.

## Rust Rules

* Avoid `unwrap()` / `expect()` in production code.
* Exceptions:

  * tests;
  * lock acquisition where failure means poisoning, not normal control flow.
* Otherwise prefer `Result` + `?` or explicit handling.
* Do not ignore errors silently.
* Avoid unnecessary `.clone()`.
* Prefer borrowing when practical.
* Do not add dependencies unless needed.
* Keep code simple and idiomatic.

## Tokio Rules

* Assume a Tokio runtime already exists.
* Never create nested runtimes.
* Never call `Runtime::block_on()` inside Tokio / async code.
* Do not hide runtime creation inside helpers or libraries.
* Do not hold locks across `.await`.
* Prefer `.await`, `tokio::spawn`, channels.
* Use `spawn_blocking` or dedicated threads for blocking work.
* Do not use `std::thread::sleep()` in async code.

## Editing Hygiene

* Change only what is required.
* Prefer the smallest valid diff.
* Do not refactor unrelated code.
* Do not make formatting-only changes.
* Keep naming/style consistent with nearby code.

### Imports

* One `use` per crate. Everything a file takes from the same crate goes in a
  single braced block, not one statement per item:

  ```rust
  // no
  use base::fs;
  use base::message_proto::*;

  // yes
  use base::{fs, message_proto::*};
  ```

* The only reason to split is a `#[cfg(...)]` that does not apply to the whole
  block -- an attribute binds to one item, so a differently-gated import has to
  stand on its own. A `pub use` re-export likewise cannot join a plain `use`.

  ```rust
  #[cfg(not(feature = "flutter"))]
  use base::fs;
  use base::message_proto::*;
  ```

* When splitting an existing `use` because some of its items moved to another
  crate, fold each side into that crate's existing block rather than leaving a
  second statement behind.

### Comments

* Avoid comments unless they explain a non-obvious reason, constraint, or workaround.
* Never restate what the code does; prefer clearer code instead.
* If the code is self-explanatory, add no comment.

### Be minimally invasive

* Prefer purely additive changes: layer new (`#[cfg]`-gated) blocks or new functions around existing code instead of restructuring it. The ideal diff for a fix adds lines and modifies/deletes none.
* Do not extract or reshape existing code just to enable your new code; look for a mechanism that leaves existing lines untouched (e.g. hide/show an existing object instead of refactoring its construction into a helper for rebuilding).
* Accept a little duplication over a restructure. A new function that repeats a few lines of an existing one is a better diff than reshaping the original so both can share it.
* Put new logic in self-contained functions in the module it belongs to (platform-specific logic in `src/platform/`, with `use` inside the function body to avoid churning shared import blocks). Call sites in shared files (`src/tray.rs`, `src/core_main/`, `src/server/connection.rs`, …) should be thin one-line hooks.

### Scope check before touching shared code

* Before changing a shared trait, a shared struct, or the signature of a widely used function, check whether the bug or feature is specific to one path. If it is, keep the change inside that path unless that is impossible, and say in the PR why it was.
* If an unrelated caller needs `Default::default()`, `None`, or another placeholder solely to satisfy a signature you changed, the diff is too broad: stop and redesign.
* The expected shape of a fix is a new function in the feature's own module, plus at most a new field or a thin hook in the shared code it needs. Feature-specific state belongs beside the feature's existing state, not in a new abstraction every caller has to learn.

### Mandatory regression-surface check

Before considering any implementation complete, perform a minimization pass over the final diff.

* Inspect every modified existing file and every modified existing code path. Each must be strictly necessary for the requested change. Revert changes that are merely cleanup, refactoring, consistency improvements, or fixes for pre-existing issues.
* For new features, preserve the existing implementation path when the feature is disabled or unsupported whenever practical. `feature off` should run the old code, not a rewritten equivalent.
* Do not route existing behavior through a new abstraction merely to share code with the new feature. Prefer a parallel new function or a small amount of duplication over changing a proven existing path.
* Keep new implementation logic in new or feature-specific modules. Changes to shared/core files should normally be thin hooks, capability checks, or protocol plumbing.
* Do not fix unrelated pre-existing bugs in the same PR. Put them in a separate change unless they directly block correctness or security of the requested work.
* For submodule bumps, inspect the exact commit range and ensure unrelated changes are not being pulled into the parent PR.
* Before finalizing, explicitly report the regression surface: list the existing files and existing runtime paths whose behavior changed, and explain why each change is unavoidable.
* During review, treat an unnecessarily modified legacy path as a review finding even if tests pass and the rewritten behavior appears equivalent.

## Reviewing a PR

* Review only what the diff introduces. Verify ownership with `gh pr diff` before reporting a finding — if the offending lines are untouched context, it is a pre-existing problem, not this PR's.
* List pre-existing problems in a separate section at the end, or leave out the ones that are not fatal. Never mix them into the findings the author has to fix.
* Before re-reviewing, read the author's reply comments. Do not re-raise items they declined on scope grounds.
* State a finding's consequence exactly: distinguish "the value is lost" from "the shortcut is inert but the value still saves".

## Localization (`src/lang/*.rs`)

Each file is a `HashMap<key, translation>`. Layout:

* `template.rs` is the master list of every key. **Never edit it** as part of translation work.
* `en.rs` holds only the keys whose English display text differs from the key itself.
* Every other file (`de.rs`, `fr.rs`, …) carries the full key set; an untranslated entry has an empty value: `("key", "")`.
* `it.rs` is maintained by hand by its translator. Never fill or change its entries; when adding new keys, append them to it with `""` and leave the translation to the maintainer.

### Finding the English source for a key

When filling an empty entry, determine the source English text with this rule:

* If `key` exists in `en.rs` **with a non-empty value**, that value is the source text (look it up in `en.rs`).
* Otherwise the **key string itself is the source text** (the key is already plain English).

Then translate that source into the file's target language (infer the language from the file's existing non-empty entries / filename).

### Translation hygiene

* Only fill empty values. Never change keys, and never touch existing non-empty translations.
* Preserve placeholders (`{}`) and escape sequences (`\n`, `\"`) exactly as in the source.
* Do not translate brand or technical tokens: `RustDesk`, `Socks5`, `TLS`, `UAC`, `Wayland`, `X11`, `TCP`, `UDP`, `2FA`, `RDP`, `D3D`, etc.
* Copy URL values (e.g. `doc_*` keys) verbatim from `en.rs`.

### Adding new keys (feature work)

* New English-text keys use sentence case, not Title Case: `Use ID whitelisting`, **not** `Use ID Whitelisting`. Acronyms (ID, IP, 2FA…) stay uppercase. Legacy Title-Case keys (e.g. `Use IP Whitelisting`) stay as-is — do not rename them.
* Since the key itself is the English display text, a sentence-case key usually needs **no** `en.rs` entry; add one only when the display text must differ from the key (e.g. `*_tip` keys).
* Append each new key to `template.rs` (with `""`) and to every `src/lang/*.rs` file (translated, or `""` if unsure; always `""` for `it.rs`), at the end of the list.
