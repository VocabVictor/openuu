//! The controller's invariants as properties over random sessions.  A scenario
//! test pins one trajectory; these hold whatever the trajectory:
//!
//! 1. viewer isolation: a viewer's private target is a function of its own
//!    replies, timeouts and limit, never of another viewer's (with ABR on, the
//!    shared bitrate state is the one designed input: the frame rate keeps a
//!    floor while the bitrate can still come down);
//! 2. bad evidence never raises anything: a bad reply or a timeout tick keeps or
//!    lowers that viewer's target and the bitrate ratio;
//! 3. lifecycle: a join adds a constraint and a leave removes it, and neither
//!    touches any other viewer's state;
//! 4. evidence ownership: a bitrate cut is asked for by a viewer's own evidence,
//!    by the step that viewer's own evidence calls for, and a newcomer's first
//!    reply does not spend that evidence again;
//! 5. caps: a reply leaves the target within `[MIN_FPS, cap]`, and the stream is
//!    the aggregation of the targets, the caps and the start-up guards;
//! 6. pairing: the late reply of a braked probe does not brake again.
use super::*;

mod harness;
use harness::*;
mod evidence_tests;
use evidence_tests::*;
mod aggregation_tests;
