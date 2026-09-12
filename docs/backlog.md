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
