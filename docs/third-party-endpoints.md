# Which third parties this client can contact

An audit of the network endpoints OpenUU may reach that do **not** belong to
the deployment running it, in the state that matters for a default: nothing
configured beyond the server details baked into the build.

Method: read what the code does on the unconfigured path, rather than trust a
name or a comment. Two findings this year came from exactly that gap — a
switch named "IPv6 punch" turned out to query public STUN servers, and the
guard protecting it sat on one of three call sites.

Scope and honesty note: this is a source audit. Where a row says a request is
not made, that is from reading the call path, not from watching the wire. The
rows marked **verified at runtime** were seen in logs today; the rest are
worth confirming with a packet capture before this is quoted as a guarantee.

## Endpoints that are not this deployment's

### 1. The built-in public STUN list

`stun.cloudflare.com:3478`, `stun.l.google.com:19302`,
`stun.antisip.com:3478`, `stun.nextcloud.com:443`
(`DEFAULT_ICE_SERVERS`, `libs/hbb_common/src/webrtc.rs`).

Injected whenever `ice-servers` holds no STUN entry of its own. Two features
reach them:

| Feature | Trigger | Gate | Gate position | Default | Operator can disable |
| --- | --- | --- | --- | --- | --- |
| WebRTC transport | A session when WebRTC is enabled | `enable-webrtc` | `get_local_option`, the single read point | **off** while the deployment has no `ice-servers` | yes: leave it unset, or set `N` |
| IPv6 address probe (`test_ipv6`) | Before offering a v6 candidate, on either side | `enable-ipv6-punch` | inside `test_ipv6`, the capability's entry point | **off** while the deployment has no `ice-servers` | yes: leave it unset, or set `N` |

Both were fixed today. The probe previously had no gate on two of its three
call sites, so a controlled machine queried these servers because the
*controlling* end had the feature on; the gate now sits at the probe itself.
Configuring `ice-servers` replaces this list rather than adding to it, so a
deployment that wants peer-to-peer without public STUN can point both at its
own STUN/TURN.

### 2. `api.rustdesk.com/version/latest` — update check

`version_check_request`, `libs/hbb_common/src/lib.rs`. The request body
carries the operating system, its version, the CPU architecture and a
**device fingerprint**.

| Path | Gate | Default |
| --- | --- | --- |
| Start-up check (`check_software_update`) | `enable-check-update`, and the capability gate below | on by default, but the request is refused for a rebranded build |
| Updater thread, on its timer | `allow-auto-update`, and the capability gate below | off unless the user ticks the switch |
| `manually_check_update` | the capability gate below | **no callers**; dead code today |

The gate now sits inside `do_check_software_update`, so a rebranded build
never sends the request whichever path asked. That is the fix for what this
audit originally found, and the original description was wrong in an
instructive way: the exposure was not a "manual check" button, which does not
exist, but the **Auto update switch**, which Windows showed on every install
including rebranded ones while only the macOS branch excluded them. A user
who ticked it got periodic requests to upstream carrying a device
fingerprint. The switch is now hidden for a rebranded build as well, so no
control is left that quietly does nothing.

### 3. `admin.rustdesk.com` — API fallback

`get_api_server_`, `src/common/server_url.rs`, the final fallback when no API
server and no rendezvous server are configured. A build with server details
baked in never reaches it, because the API address is derived from the
rendezvous address. Relevant only to a build shipped with no defaults at all.

### 4. `api.telegram.org` — two-factor notifications

`src/auth_2fa.rs`. Reached only when the user has entered a Telegram bot
token; no token, no request. This is the user choosing a third party
explicitly, which is what the rule allows.

### 5. Links to websites

`rustdesk.com/download` (mobile connection page), the project repository and
licence links on the About page, and the documentation links in the language
tables. All are opened in a browser on a click; nothing is fetched by the
client. The download link still points at upstream and is arguably a bug of
its own.

## Endpoints that are this deployment's own

Listed so the audit is complete, not as a concern: the rendezvous server
(registration, NAT test, punch brokering), the relay, the API server
(heartbeat, address book, account) and the audit endpoint. All derive from
the configured or baked-in server address. The UDP NAT probe deliberately
accepts a punch port only from the rendezvous server's own reply.

## What a self-hosted operator should know

* With nothing configured beyond the built-in server, **no third-party
  endpoint is contacted without the user asking for it.** The two that could
  be reached automatically are off by default, and the update check is either
  skipped for this build or off.
* The exception is a **manual** update check, which contacts upstream and
  includes a device fingerprint.
* To use peer-to-peer transports without public STUN, set `ice-servers` to
  the deployment's own STUN/TURN; that replaces the built-in list.

## Open items, for triage rather than for fixing here

1. The mobile connection page links to upstream's download page.
2. None of this is stated in the README, although it is the kind of thing a
   self-hosted product is chosen for. Confirm the audit with a packet capture
   before quoting it there: this is a reading of the source, not of the wire.

The update check that stood here has been fixed; see the row above.
