# Why hole punching is on or off by default

## What it used to do

`get_local_option` in `src/common/server_url.rs` answered `N` for
`enable-udp-punch`, `enable-ipv6-punch` and `enable-webrtc` whenever the user
had expressed no preference and the rendezvous server was not one of the
public ones. It was a read-time default, not a stored value: the switches
showed as off while the configuration file held nothing.

Upstream's reasoning is easy to reconstruct. The public deployment has STUN
and TURN behind it and a NAT-type service that has seen the peer before;
someone else's server might have none of that, so punching could be wasted
work. Reading "not the public server" as "probably cannot punch" is a cheap
proxy for that.

It is the wrong proxy for this fork. Our own hbbs answers the NAT test and
punches like any other server, so the rule meant that **every self-hosted
deployment relayed every session** while reporting the switches as off. That
was one half of the day's relay puzzle, the other half being a peer with no
inbound firewall rule.

## What decides the default now

Not the deployment's shape, but whose servers the capability needs when
nothing is configured.

| Switch | Default | Why |
| --- | --- | --- |
| `enable-udp-punch` | on | The NAT probe only ever sends to this deployment's own rendezvous server, and the punch port may only come from that server's `TestNatResponse`. |
| `enable-ipv6-punch` | on once `ice-servers` is set, off otherwise | `test_ipv6` queries the built-in public STUN list to learn the host's public v6 address when the deployment has configured none of its own. |
| `enable-webrtc` | on once `ice-servers` is set, off otherwise | With no ICE servers of its own it falls back to the same public STUN list. |

An explicit value always wins, in either direction; the rule only ever
applies to an unset switch. The reasoning behind the last two is the
third-party rule in `AGENTS.md`.

The IPv6 row is worth dwelling on: the name suggests a purely local
capability, and the first version of this change defaulted it on for that
reason. Only reading `test_ipv6` showed otherwise.

## How the two layers interact

Two places can set these switches, and they are not equals:

1. **Baked-in defaults.** `base::config::builtin::apply_local_defaults`
   inserts values into `DEFAULT_LOCAL_SETTINGS`, the layer
   `LocalConfig::get_option` falls back to. This build sets
   `enable-udp-punch=Y` there.
2. **The read-time rule** in `get_local_option`, described above.

**The baked-in default wins**, because the rule only fires when the value it
sees is empty, and a baked-in default is not empty. So setting a switch in
layer 1 makes layer 2 irrelevant for that switch, which is why
`enable-udp-punch` was already effectively on in this build before the rule
changed. Change one layer and check the other; a switch can look wrong in the
UI while behaving correctly, or the reverse.

Order of precedence, highest first: an overwrite (custom client), the user's
own value, the baked-in default, then this rule.

## Fallback paths and what they cost

Nothing here is a commitment to punch: each path has a timeout and a relay
behind it.

* **UDP NAT probe** sends a burst to the rendezvous server and retries with a
  widening interval. No answer means `success=false` and TCP punching carries
  the connection, which is what happened on this deployment until the server
  learned to answer `TestNatRequest`.
* **The direct attempt** is bounded by `connect_timeout` in
  `src/client/connect.rs`: 1000 ms when the peer is local or behind a
  symmetric NAT, otherwise six times the measured punch time (three after a
  previous direct failure), floored at 1000 ms, or `CONNECT_TIMEOUT`
  (18 000 ms) when there is no relay server to fall back to. On this LAN it
  is the 1000 ms case, which is exactly the second the firewalled peer spent
  before relaying.
* **WebRTC**, when enabled, holds an already-established relay result for
  `WEBRTC_PREFER_WINDOW_MS` (2500 ms) so a viable peer-to-peer path is not
  abandoned for a relay that merely answered first. That window is the worst
  case this switch adds, and upstream has since made it configurable
  (`3c7c13d79`, not yet ported; see `docs/upstream-sync-2026-09.md`).

## What this change is worth in practice

For a build with no `ice-servers` configured, which is the shipped default,
**behaviour does not change**: UDP punching was already on through the
baked-in default, and IPv6 punching and WebRTC remain off. The value is that
the deployment's shape no longer decides its capability, so a deployment that
configures its own ICE servers now gets IPv6 punching and WebRTC without
having to discover an undocumented read-time rule, and a self-hosted
deployment is no longer told its switches are off for a reason that was never
about it.
