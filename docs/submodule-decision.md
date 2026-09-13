# Should OpenUU fork `hbb_common`?

Written for a decision on 2026-09-14. The short answer this page argues for is
**no, not yet**, and the reason is that the change which was supposed to force the
question turns out to be reachable without a fork on the two transports that matter.

`libs/hbb_common` is a git submodule pointing at the upstream repository. Every commit
in this repository that touches it only moves the pointer to one of theirs; we have
never changed a line in it. That is the state the options below start from.

## 1. What the two walls cost today

**The WebSocket Nagle line.** `websocket.rs` passes a hardcoded `disable_nagle = false`
when connecting, so a WebSocket deployment pays Nagle against the peer's delayed
acknowledgement on traffic that is all small messages wanted at once. The TCP transport
already disables it.

Cost of not fixing it: **zero today.** Nothing uses WebSocket by default; it exists for
deployments where nothing else traverses. The bill only arrives if WebSocket becomes a
default, and it is a one-line change when it does.

**The video write in the connection's main loop.** A write that blocks holds the whole
`select!`, so while a frame is going out the loop is not reading the socket: input,
clipboard and the probe replies wait behind the picture. Measured at up to 1.4 s on a
2 Mbps link.

Cost of not fixing it properly: **bounded, not zero.** Three changes already landed
against it. The capture loop holds the encode while a frame is unfetched, the controller
measures the link from the blocked send and cuts the bitrate to fit, and a stalled write
ends the session in five seconds rather than twelve. What is left is bounded by how long
one frame takes to flush. That is a real residual on a thin link, and it is the item
this decision was supposed to be about.

## 2. The claim that made this a decision was too strong

`docs/backlog.md` says the writer split "has no workaround at all", because the send
path carries the encryption sequence and only one writer may own it. I wrote that. It
is wrong, and here is what the code says instead.

The encryption state is `Encrypt(pub Key, pub u64, pub u64)`: a key, a **send** counter
and a **receive** counter, which are independent of each other. Nothing couples the two
directions except that they are stored in one struct.

Transport by transport:

| Transport | Can a reader/writer split be built in this repository? |
| --- | --- |
| WebRTC | **Yes, already.** `WebRTCStream` has a hand-written `Clone`, and its own documentation says every clone points at the same peer connection. A second handle costs nothing. |
| TCP | **Yes.** `FramedStream`'s fields are all public, `Encrypt`'s are too, and we already construct one by hand in `src/kcp_stream/kcp_impl.rs`. |
| WebSocket | **No.** `WsFramedStream`'s fields are private and it exposes no accessor. This one really is closed. |

The server is the existence proof for the counter half: its rendezvous input path keeps
an `Encrypt` used only for decryption, which is exactly the one-direction-per-instance
shape a split needs.

Two caveats, so this is not read as cheaper than it is:

* The split must be taken at the byte-stream level, splitting the inner stream and then
  framing each half separately. Splitting the framed layer itself through its sink and
  stream halves puts both behind one lock, which reinstates the very blocking the change
  is for.
* It means owning a copy of the framing and encryption wrapper in our repository, on the
  order of 150 lines, built on fields that are public today by accident rather than by
  promise. An upstream pointer move can break it, and it will break at compile time,
  which is the good kind.

WebSocket keeps the coupled path. That is acceptable precisely because it is the
transport nothing uses by default, which is the same reason the Nagle line costs
nothing.

## 3. What forking would cost

The pointer has moved **19 times in the last 90 days**, about twice a week, and the
moves are whole upstream merges covering WebRTC, the base-crate split and port-forward
changes, not small patches. Each one becomes a merge we perform instead of a pointer we
move, against a library whose internals we do not follow day to day.

Of the 96 commits in that window, 81 come from the upstream organisation account or its
two long-standing developer handles, and 15 from two other names. It is an active
repository with outside contributions landing, not a dormant one we would be rescuing.

Against that, our divergence today is **zero**. A fork starts free and gets more
expensive the longer we hold changes upstream does not want. That asymmetry is the whole
argument for deciding late rather than early: there is no accumulating cost to waiting,
and there is one to forking.

## 4. The three ways that are not a fork

**(a) Send it upstream.** Costs us their schedule and their view of what belongs in a
shared library. The Nagle line would plausibly be taken: it is one line, it matches what
the TCP transport already does, and outside commits do land there. The socket split
would plausibly not, since it changes a type every one of their transports goes through
and they carry deployments we know nothing about. Reasonable use: the Nagle line, when
someone needs it. Not a route for anything structural, and not a route with a date on
it.

**(b) Wrap it in our repository.** Section 2 is this option. It reaches WebRTC and TCP,
leaves WebSocket alone, needs no upstream involvement and no fork, and costs us roughly
150 lines we own plus a compile break whenever upstream reshapes those structs. This is
the option this page recommends.

**(c) Move what we need into `libs/base`.** AGENTS.md already says client-only code goes
there, and upstream is moving the same way on their own account: a recent merge is
titled "move the modules only rustdesk uses out to the base crate". So the shared floor
is shrinking without us doing anything.

The limit is what "shared" means. The TCP module is not client-only. The server uses its
listener constructors and its `Encrypt`, and the wire format and the encryption sequence
have to stay identical on both ends or sessions stop working. Moving the transport into
`libs/base` would fork it from the server's copy in the one place where divergence is
silent and fatal. So (c) is right for anything client-only still sitting in the
submodule, and wrong for the transport, which is the thing we actually want to change.

## 4a. Two things building it turned up

Found while implementing option (b), after this page was written. Neither changes the
recommendation; both make the estimate in section 2 slightly less cheap than it looked,
and both are the kind of thing only writing the code finds.

**`Stream` implements `Drop`.** It closes a WebRTC session on the way out, which means the
enum's variants cannot be destructured at all — `cannot move out of type which implements
the Drop trait`. The socket has to be taken out through a `&mut` with `mem::replace`, which
needs an inert stand-in to leave behind, and the emptied husk is then dropped. That is safe
only because `close_webrtc` does nothing on a TCP stream, which is one more upstream detail
the module now depends on.

The same `Drop` is why WebRTC is harder than section 2 implies. Its handle clones freely,
but two `Stream`s over one connection would close the session when the first of them is
dropped, so the halves cannot simply be two `Stream`s. Still easy, still not free.

**The chaining adaptor's type cannot be named.** Bytes already pulled off the socket but not
yet framed have to be read out before the socket itself, and the adaptor that does that is
not exported by tokio, so the read side is a boxed trait object. One allocation per
connection, and one more place where a version bump is felt.

## 5. Recommendation, and what would change it

**Do not fork. Take option (b): build the writer split in this repository for TCP and
WebRTC, leave WebSocket on the coupled path, and keep the Nagle line in the backlog
until a deployment needs WebSocket.**

The reasoning in one line: the change that was supposed to force a fork does not force
one, and everything else on the list is worth nothing today.

Fork when **any one** of these becomes true, and not before:

* **WebSocket becomes a transport people actually use.** Then both walls land on the
  same file at once, and the wrapper route is closed for it by private fields.
* **A change is needed to the rendezvous protocol messages or to the encryption sequence
  itself.** No wrapper can help there, because both ends must agree, so the change has
  to be in the shared floor.
* **Upstream breaks the wrapper more than about twice.** The public fields the wrapper
  stands on are not a promise. If holding it costs a repair every few pointer moves, we
  are already paying fork maintenance without having the control a fork buys.

Deciding later costs nothing, because our divergence is zero and stays zero under this
recommendation. That is also why the third trigger matters: the moment we are repairing
a wrapper every other week, waiting has started costing something and the arithmetic has
changed.
