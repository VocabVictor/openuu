# Server and client configuration provisioning

Status: approved (2026-09-13). Goal: a user
installs OpenUU and never opens the network settings; an administrator ships
one file (or one QR code) and every option in it is in effect on first start.

## 1. The layers and their precedence

| layer | where it lives | set by | wins over |
| --- | --- | --- | --- |
| built-in defaults | baked into the binary at build time | the build (env / secrets) | nothing |
| configuration file | `openuu-config.json` (exe dir, `--import-config`, MSI, settings page, QR/clipboard) | administrator | built-in defaults |
| user edits | `OpenUU2.toml` options / `OpenUU_local.toml` | the user in the UI | file and defaults, **except locked keys** |
| locked keys | the file's `locked` list | administrator | user edits: the UI shows the value greyed out |

This maps one-to-one onto the three maps `hbb_common::config` already has:
`DEFAULT_SETTINGS` (fallback when the user value is empty), `CONFIG2.options`
(the user's value) and `OVERWRITE_SETTINGS` (forced, `set_option` refuses to
persist, the Flutter pages already grey such options through `isOptFixed`).
Built-in defaults and the file's `server` / `options` sections go into
`DEFAULT_SETTINGS` (a later file import replaces the file's contribution);
`locked` keys go into `OVERWRITE_SETTINGS`. No new precedence code: what
`Config::get_option` returns today is what the design specifies.

`custom.txt` (upstream's signed custom-client file) keeps working unchanged
and sits in the same maps; OpenUU does not ship it.

## 2. Built-in defaults

* Four constants in `libs/base` (client-only crate, so no `hbb_common`
  round-trip): `BUILTIN_ID_SERVER`, `BUILTIN_RELAY_SERVER`,
  `BUILTIN_API_SERVER`, `BUILTIN_KEY`, produced by `libs/base/build.rs` from
  `OPENUU_DEFAULT_ID_SERVER` / `_RELAY_SERVER` / `_API_SERVER` / `_KEY`
  environment variables (`rerun-if-env-changed`). Unset → empty string → the
  layer contributes nothing, so a developer build behaves as today.
* The repository carries only placeholders: `res/default-config.json` with
  empty values and a comment; `.github/workflows/windows-build.yml` and
  `release.yml` export the four variables from repository secrets;
  `openuu-toolchain/build-env.ps1` reads them from `D:\openuu-tools\
  default-config.env` (git-ignored) for local builds. Real values exist only
  in those two places: never in the repository, the docs, a PR or a log line
  (the start-up log prints the id server host and `key=<set>` / `key=<unset>`,
  never the key).
* At start-up (`core_main`, before `load_custom_client`) the non-empty
  constants are inserted into `DEFAULT_SETTINGS` under
  `custom-rendezvous-server`, `relay-server`, `api-server`, `key`, and
  `PROD_RENDEZVOUS_SERVER` (read by `Config::get_rendezvous_server` today,
  written by nobody) is set to the id server. The MSI and the portable zip
  contain the same binary, so both get the defaults.
* A non-empty user value overrides a default (existing semantics); an empty
  user value falls back to it, which is what the settings page shows as the
  placeholder.

## 3. The configuration file

`openuu-config.json` (UTF-8, one object). Key names in `options` and `local`
are the constants of `libs/base/src/config/keys.rs` verbatim, so the file
needs no separate vocabulary and the parser validates against those lists.

```json
{
  "version": 1,
  "server": {
    "id": "rs.example.com",
    "relay": "rs.example.com",
    "api": "https://rs.example.com:21114",
    "key": "<server-public-key>"
  },
  "options": {
    "verification-method": "use-permanent-password",
    "permanent-password": "s3cret",
    "enable-remote-assistance": "Y",
    "pinned-windows-session": "Administrator",
    "codec-preference": "h265",
    "image-quality": "balanced"
  },
  "local": {
    "lang": "zh-CN",
    "theme": "dark"
  },
  "locked": ["custom-rendezvous-server", "relay-server", "api-server", "key",
             "verification-method"]
}
```

* `server` is sugar for the four option keys; a key given in both places is
  an error. `api` must be `http(s)://`; `id`/`relay` are checked with the
  existing `test_if_valid_server`.
* `options` → `DEFAULT_SETTINGS` (keys from `KEYS_SETTINGS`, `KEYS_DISPLAY_SETTINGS`
  and the other option lists); `local` → `LocalConfig` keys (`KEYS_LOCAL_SETTINGS`).
  Unknown keys are ignored with `log::warn!("event=config_import_unknown_key
  key=…")`; the import still succeeds.
* `locked` may name any key from `server` or `options`; those go into
  `OVERWRITE_SETTINGS`. Locked keys are only accepted from a file, never from a
  QR code or the clipboard.
* Persistence of the file layer: `OVERWRITE_SETTINGS` and `DEFAULT_SETTINGS`
  are in-memory tables loaded at start-up (today from `custom.txt`). A
  successful import therefore stores a verbatim copy of the file as
  `<config dir>/imported-config.json` (secrets stripped, they were already
  applied to their own stores) and every start reloads that copy into the two
  tables before `custom.txt`; `imported-config-hash` is the SHA-256 of that
  copy. Deleting the copy un-locks everything on the next start.
* Secrets: `permanent-password` (and any key ending in `password`, `pin` or
  `token`) is accepted from a file, applied through the existing
  `Config::set_permanent_password` path (stored hashed as today), never
  written to `DEFAULT_SETTINGS`, never logged, and shown in the UI only as
  "已设置". Private keys have no key name and cannot appear at all; the only
  key material in the file is the server's public key.
* Version: `version` is mandatory; a newer major than the binary knows is
  refused with a message naming the version; missing optional sections are
  fine.

### Import channels and timing

| channel | when | who applies it |
| --- | --- | --- |
| `<exe dir>/openuu-config.json` | every start of the service and of the UI process; skipped when its SHA-256 equals the stored `imported-config-hash` local option | Rust `core_main` |
| `--import-config <path>` | as today for `.toml`; a `.json` path takes the new parser (the MSI-generated temp service keeps working) | Rust |
| MSI | `msiexec … OPENUU_CONFIG=<path>` copies the file to the install dir as `openuu-config.json` (so the exe-dir rule picks it up) and passes it to the existing `--import-config` custom action; a `openuu-config.json` placed in the dist dir is bundled the way `custom.txt` is (`preprocess.py` per-customer file). When both a `.json` and the legacy `.toml` import path are present the `.json` wins and the conflict is logged as `event=config_import_conflict` | MSI + Rust |
| settings page → "导入配置文件 / 从剪贴板导入" | manual | Flutter calls `main_import_config_text` (new FFI, one-line forward) |
| QR / deep link | see §4 | Flutter → same FFI |

The importer is one function, `base::config::provision::apply(json: &str,
source: Source) -> Result<Report>`, where `Report` lists applied, locked,
ignored-unknown and secret keys (names only). All channels call it; the UI
shows the report.

### Compatibility

* The existing clipboard/QR payload (`{host, relay, api, key}` JSON,
  base64url, string reversed; `config=` prefix on the mobile scanner) stays
  decodable: the decoder tries the new form first, then the legacy one. The
  encoder only emits the new form.
* `--import-config <path>.toml` behaviour (8f's `import_config_files`) is
  untouched.

## 4. QR code and deep link

* Payload: `openuu://config/<base64url(JSON)>` with the JSON reduced to
  `version`, `server` and an optional `options` subset. No `local`, no
  `locked`, no secret keys (the encoder drops any key matching the secret
  rule). Optional `connect: {"id": "…", "password": "…"}` is allowed for the
  assistance flow (a one-time password for one device, not a server secret).
  Hard cap 1 200 bytes after encoding (QR version ≤ 20 at level M scans
  reliably on phones); the encoder refuses to build a larger code and tells
  the user to use a file instead.
* Desktop: Settings → Network gains "分享配置二维码" (dialog with `QrImageView`,
  already a dependency via 2FA) and "从剪贴板 / 文件导入"; the assistance page
  gets the same share button with `connect` prefilled when a session code is
  showing. Import shows the decoded server values in a confirmation dialog
  before applying; it never applies silently.
* Mobile: first start with no server configured opens the existing scanner
  (`mobile/pages/scan_page.dart`) with a "扫码导入配置" title; the settings
  page keeps its scan entry. The scanner accepts `openuu://config/…` and the
  legacy `config=…`. `allow-deep-link-server-settings` defaults to on in
  OpenUU. The URL-scheme registrations still say `rustdesk` (Android
  manifest, iOS/macOS plists) while `get_uri_prefix()` returns `openuu://`;
  they are changed to `openuu` in the same change.

## 5. Ownership and steps

Rust (openuu-e9), one logical unit per commit:
1. `libs/base` build-time constants + `res/default-config.json` placeholder +
   workflow/env plumbing; start-up injection into `DEFAULT_SETTINGS`.
2. `base::config::provision`: schema, validation against `keys.rs`, secret
   rule, `apply` with the three maps, report; unit tests (all sections,
   unknown key warning, locked key, secret handling, legacy payload decode,
   version refusal, QR size cap).
3. Channels: exe-dir file with hash marker, `--import-config` `.json`
   dispatch, `main_import_config_text` FFI, MSI property + bundling.

Flutter (openuu-52):
4. Desktop share-QR dialog and import (clipboard / file) on Settings →
   Network and the assistance page; locked keys shown through `isOptFixed`.
5. Mobile first-start scan, new prefix, scheme registration fix.

Verification: build-machine `cargo test` for the parser and channels,
`flutter analyze`, then a CVM smoke test: fresh portable zip with baked-in
defaults connects with no settings touched; MSI with `OPENUU_CONFIG=` applies
the file; a phone scans the desktop QR and connects.
