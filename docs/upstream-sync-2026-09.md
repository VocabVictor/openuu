# Upstream sync inventory, 2026-09

An inventory only: nothing here has been ported. It answers what upstream
RustDesk has that OpenUU does not, as of upstream `bf1ebe5be` (2026-09-12).

## How the gap was determined

Commit ancestry cannot be used. OpenUU's history was rewritten twice on
2026-09-13 (placeholders for real hosts and keys, then removal of AI
attribution trailers), which gave every commit after the earliest edited one
a new hash. Only 436 very old commits still share a hash with upstream, and
`git merge-base` therefore reports a 2022 merge that is not the real fork
point.

The gap was taken by commit subject instead: every upstream subject in the
most recent 1500 commits was checked against the set of subjects in our
history. By that measure OpenUU carries upstream through **2026-09-11**
(`e82dd1235`, lowering QoS sample logging to trace), and six upstream
commits in that window are absent.

The method has one blind spot worth stating: a change we adopted but
reworded, or folded into a larger commit of our own, would show up here as
missing. Each entry below was therefore checked against the actual code
before a recommendation was made.

## What is missing

| Upstream | Date | What it is | Port? |
| --- | --- | --- | --- |
| `3c7c13d79` | 09-12 | Makes the WebRTC prefer window, which holds back the relay fallback, configurable as `relay-fallback-delay` instead of a hardcoded 2500 ms | **Worth porting**, see below |
| `bf1ebe5be` | 09-12 | CI job building arm64 natively with msbuild | No |
| `e54f21e10` | 09-12 | Upstream CI fixes | No |
| `5439ec38b` | 05-06 | Reverts the commit below | No |
| `d5d0b0126` | 04-29 | Fixes a web build broken by the Linux mouse side-button change | No |
| `6c541f7bf` | 02-11 | `libxdo3` dependency for the Debian package | No |

The four CI and web entries need no judgement: OpenUU ships Windows x64
only, runs its own workflows, and removed the web target, so none of them
has anything to act on. The `d5d0b0126` / `5439ec38b` pair is a fix and its
own revert, a net change of nothing even for a fork that kept the web. The
`libxdo3` entry is Debian packaging, which OpenUU does not produce.

## The one that matters: `3c7c13d79`

Upstream replaced the constant `WEBRTC_PREFER_WINDOW_MS = 2500` with a
`relay-fallback-delay` option, read as seconds from the settings page and
clamped so that an unparseable, zero or negative value falls back to the
default rather than collapsing the delay and handing every race to the
relay. It adds the key to `KEYS_LOCAL_SETTINGS`, a field on the network
settings page, and one string to every language table.

Why it is relevant to us. That window is exactly the knob behind the
behaviour investigated today: a peer whose direct path is viable but slow to
accept loses the race and the session settles on the relay. Our own case
turned out to be a missing firewall rule rather than a too-short window, but
a user on a high-latency link has no way to widen it, and the value that
suits a LAN is not the value that suits a satellite link.

Conflict risk: **medium to high**, entirely because of our own
restructuring, not because the change is large.

* `src/client.rs` no longer exists as one file. The three upstream call
  sites map to four in our tree: `src/client/connect.rs` (two),
  `src/client/start.rs` and `src/client/start_relay.rs`. The constant and
  the new accessor would sit in whichever of those modules owns the racing
  policy, which is a decision the port has to make rather than copy.
* `flutter/lib/desktop/pages/desktop_setting_page.dart` has been split and
  restyled onto the design tokens, so the upstream settings-field diff will
  not apply; the field has to be rewritten in our network tab's idiom.
* The language tables take one new key, which follows our own localisation
  rules (sentence case, appended, `it.rs` left empty) rather than upstream's.
* `libs/base/src/config/keys.rs` is the one file where the upstream diff
  should apply nearly as-is.

Suggested shape if it is approved: take the idea, not the patch. One commit
adding the key and the accessor with unit tests for the clamping, one commit
moving the four call sites onto it, one commit for the settings field, one
for the language key. That keeps each within the commit-size rule and leaves
the racing behaviour unchanged when the option is unset.

## Standing note

OpenUU tracked upstream to within a day, so this inventory is short by
construction. It will stay short only if it is repeated; after another few
weeks the subject-matching method still works, but the number of entries to
judge grows, and entries touching `src/client/`, the settings pages or the
server will keep carrying the same restructuring cost described above.
