# The backlog, sorted for 2026-09-14

Every item in `docs/backlog.md` classified two ways — does it have a prerequisite, and
whose area is it — and then an order that can be handed out tomorrow morning. Nothing
here is new work; it is the existing list arranged so that two people do not wait on the
same thing.

## The one that is not like the others

**Clearing the test residue on the Hyper-V peer** is not a backlog item in the sense the
others are. That machine logs into a local administrator account automatically with its
password stored in clear text under the Winlogon key, and it has been in that state since
it was shut down on 2026-09-13 with a fixture still installed. It is now item 10 of
`docs/vm-boot-checklist.md`, deliberately last, because the automatic logon it removes is
what gives the peer the interactive desktop that most of that list needs. Last, and not
optional: if the list runs out of time, this still happens.

## No prerequisite — can start tomorrow morning

| Item | Area | Size |
| --- | --- | --- |
| The duplicated configuration preview and confirm flow (mobile and desktop implement the same two calls twice) | 8f | small |
| Fourteen files importing `consts.dart` with one `..` too many | 8f | trivial, style only |
| The writer split for TCP and WebRTC (section 5 of `docs/submodule-decision.md`) | f0 | the largest thing on this page; needs 0f's decision first |
| `Connection` test constructor | f0 | half a day, and it unblocks two others |
| Bundle-swap scripts must verify their restore | e9 | small, and it is the thing that already destroyed one bundle |
| The MSI logging two expected failures as failures | anyone with the WiX work | deferred on purpose until the custom actions are touched again |

## Has a prerequisite

| Item | Waiting on | Area |
| --- | --- | --- |
| Reducing the tracked single-function exceptions | tests first, and the `Connection` constructor for the two connection ones | f0 |
| Visual confirmation of the three mobile dialog regressions | the Android build working again | 8f |
| Mobile design tokens | the same Android build | 8f |
| Dark-mode tokens | a decision on whether dark mode is a supported appearance | 0f decides, 52 implements |
| The insecure-connection dialog onto `UiDialog` | agreement that mobile takes the same visual | 52 and 8f together |
| `MenuButton` shared with the connection manager | the connection-manager restyle landing first | 52 |
| Remote-session toolbar restyle | none stated, but it is a rework, not a follow-up | 52 |
| Login dialog restyle | none stated; restyle, do not replace | 52 |
| Encrypting the controlled peer's registration channel | a design, and it touches both repositories | f0, with the server side |
| Nagle on the WebSocket transport | a deployment that actually needs WebSocket | nobody, by decision |

## What that means for tomorrow

**Two things gate other people and should be decided or done first.**

1. **0f rules on `docs/submodule-decision.md`.** If the recommendation stands, f0 starts
   the writer split, which is the largest remaining item and the only one on the
   performance list with nothing else in front of it. If it does not stand, f0 needs new
   work, so this cannot wait until the afternoon.
2. **0f rules on dark mode.** It gates the desktop token work and the mobile token work
   at once, and it is a product decision nobody else can make. The entry is explicit that
   the answer changes the shape of the fix rather than just its size.

**Then the areas run in parallel without touching each other.**

* **f0**: the writer split, if approved. `Connection` test constructor if not, since it
  unblocks two of the tracked exceptions and needs no machine.
* **52**: the connection-manager restyle, because `MenuButton` and the insecure-connection
  dialog both sit behind it. Doing it first turns two blocked items into unblocked ones.
* **8f**: the Android build, for the same reason. Two mobile items wait on it and neither
  can be judged without a screen. The two tidy-ups with no precondition are the fallback
  if the build fights back.
* **e9**: the machine, all day, starting the moment the peer boots. The bundle-swap
  script fix belongs to e9 and should go in before the next swap rather than after the
  next loss.

**Nothing here needs the build machine at the same time as the peer**, which is the
constraint that actually bites: the peer has two virtual processors and anything else
running on it invalidates the numbers. The desktop and mobile work is all local.

## Deliberately not scheduled

* **The WebSocket Nagle line.** Zero value until something uses WebSocket. Revisit when
  that changes, and at that point read `docs/submodule-decision.md` again, because that
  is the trigger that also reopens the fork question.
* **The MSI log strings.** They read like defects during installer review but are expected
  outcomes; rebuilding and revalidating an MSI for two log lines is not worth it until the
  custom actions are being touched anyway.
