# Publish readiness (openuu + openuu-server)

Status 2026-09-13: **history rewritten and published**. Both repositories were
scanned, cleaned in HEAD (phase 1) and then rewritten with `git filter-repo
--replace-text` / `--replace-message` on mirror clones (phase 2). The rewritten
history was force-pushed to GitHub and to the build machine's bare
repositories; every clone and worktree was re-attached with
`E:\openuu-cache\rewrite\resync.ps1`. The replacement list is kept outside the
repositories.

## What was scanned

Every commit reachable from any ref of both repositories: gitleaks 8.29 over
the full history, plus a custom sweep (`history_scan.py`, kept with the
toolchain) for IPv4 addresses, host names, account names, e-mails, credential
literals, key blocks, Windows SIDs, MSI/manifest GUIDs, mobile numbers and
messenger handles.

Not found anywhere in either history: passwords, tokens, private key blocks,
SSH public keys, Windows SIDs, local user profile paths.

## What was replaced (in HEAD and in history)

| Category | Replacement |
| --- | --- |
| A colleague's Windows account name (test fixture and a Windows sessions doc) | `alice`, `<other-user>` |
| A VM account name in a profile path | `<user>` |
| Two test accounts on the production account service | `<test-account-1>`, `<test-account-2>` |
| Cloud instance id and security group id | `ins-xxxxxxxx`, `sg-0123456789` |
| Operator's build directory on the server | `/home/<user>/...` |
| Production rendezvous/relay/account server address | `rs.example.com` |
| Build machine, VM and public addresses seen in smoke tests | RFC 5737 documentation addresses (`192.0.2.x`, `198.51.100.x`) |
| VM host name | `<vm>` |
| Device ids of the two test peers | `100000001`, `100000002` |
| The production server's public key | `SERVER_PUBLIC_KEY_PLACEHOLDER=` |
| Session names in doc status lines | dropped |

Replacements use word and digit boundaries, so base64 payloads and longer
numbers that merely contain a token were left untouched (verified: the only
remaining matches are inside upstream minified JavaScript bundles from 2022).

## Deliberately kept

* Upstream RustDesk sample keys and Firebase config flagged by gitleaks: public
  in upstream.
* MSI `UpgradeCode` and component GUIDs: not secrets; they must stay stable for
  upgrades of installed builds.
* Committer identities (GitHub noreply addresses and upstream authors).
* Documentation and test addresses that already were placeholders.

## Verification of the rewrite

* Commit count and tag count unchanged (openuu 12099 commits / 39 tags,
  openuu-server 501 commits); every tag's tree identical to the old tag.
* openuu: new `master` tree identical to the old one (phase 1 had cleaned HEAD);
  openuu-server: only `src/account/audit_tests.rs` changed (device ids).
* `git log -p --all` over the new history: zero matches for every replaced value.
* Build machine: openuu `cargo check` lib and tests, openuu-server tests, all
  green on the rewritten `master`.

## Going forward

`AGENTS.md` ("Documentation and test data") requires placeholders in
documents, commit messages and test fixtures. Run gitleaks on a branch before
pushing anything that touches docs or fixtures.
