# Backlog

Items ruled "not this round" by the coordinator; each one is a separate task
with its own tests.

* **`Connection` test constructor.** `src/server/connection/conn_struct.rs`
  has no `cfg(test)` constructor, so `Connection::on_message` cannot be
  driven from a unit test; the tests under `src/server/connection/test/` are
  all pure functions. Wanted first for "a LoginRequest on a controlled peer
  without an account session goes straight to scope check and password
  validation" (a84c5c4d1 removed the gate; coverage today is the CVM smoke
  test with the controlled peers signed out). Estimate: half a day.
* **Encrypt the controlled peer's registration channel.** The peer ticket
  hbbs forwards (openuu-server `docs/relay-ticket-peer.md`,
  `docs/relay-ticket-peer-initiated.md`) travels on plain UDP/TCP to the
  peer; the UDP path has no key exchange.
* **Reduce the tracked single-function exceptions** listed in `AGENTS.md`
  (`Connection::start`, `on_message`, `send_logon_response_and_keep_alive`,
  `core_main`, ...); each needs tests before it is broken up.
* **Remote-session toolbar in the new visual style.** The remote / file /
  terminal windows still use the upstream `remote_toolbar.dart`; the new
  shell reaches them through the real session plumbing (`connectInPeerTab`
  -> `connect()`), so the toolbar is a separate rework.
* **Login dialog in the new visual style.** `_openLoginDialog`
  (`common/widgets/login/login_dialog.dart`) is the only sign-in path and
  is reached from the welcome page, the title-bar account menu, the devices
  page and settings; restyle rather than replace.
* **Bundle-swap helper scripts must verify the restore.** A throw-away swap
  script derived its backup directory name from `%time%`, which parsed to an
  empty string, so every swap reused one directory and two rollbacks silently
  restored nothing; the backups were then deleted and the peer's original
  bundle was lost. Any such script names the backup with
  `Get-Date -Format yyyyMMdd-HHmmss` and asserts the restore with `Test-Path`
  (and a file-hash or timestamp check) before deleting a backup.
* **The MSI logs two expected failures as plain failures.** The one-shot
  configuration import service never reports to the service control manager,
  so its start request is meant to fail, and the pre-install sweep for a
  stale temporary service is meant to find nothing; both go through
  `MyStartServiceW` / `OpenServiceW` and are logged as
  `Failed to start service ... 0x41D` and `Failed to open service ... 0x424`,
  which reads like a defect during installer review. Reword them on the
  import path the next time the WiX custom actions are touched, rather than
  rebuilding and revalidating the MSI for a log string.

