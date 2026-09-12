//! Closed-loop network simulation for the QoS controller.
//!
//! The controller is driven the way `Connection` drives it: one TestDelay probe per
//! second, a single probe outstanding, `user_delay_response_elapsed` on every timer
//! tick, `update_display_data` once per second.  Video frames and probes share one
//! FIFO, which stands for the downstream shared path (stream, transport, link): the
//! probe measures the bytes that were handed to that path in front of it.  It is not
//! the server's `tx_video` channel, which the probe does not pass through, and the
//! model does not stall the timer while a send is blocked, as the real loop does.
//!
//! Three independent random streams keep an A/B comparison paired: the network
//! trace (capacity wobble, stalls, loss events) is generated before the run from the
//! network stream alone, probe jitter is a per-second table from its own stream,
//! and scene changes follow the wall clock, so two controllers with the same seed
//! face the same link, the same jitter and the same content timeline whatever they
//! decide.  Only the frame size noise depends on how many frames were produced.
//!
//! The encoder model conserves its bitrate budget: a scene change costs three
//! frames' worth of data and the surplus is repaid by the following frames, so the
//! long-term offered load does not depend on the frame rate under CBR.
//!
//! It still is a model, not a network: it does not reproduce a real transport's
//! congestion control or a real encoder.  Its job is to show how the controller
//! reacts to the *kind* of behaviour a home Wi-Fi, a stable relay or a saturated
//! uplink produce, deterministically and over many seeds.
//!
//! Against overfitting: the CI run uses seeds 1 to 20; `robustness.rs` applies the
//! same bounds to seeds 21 to 120 and sweeps the scenario parameters.  Scenario
//! parameters are educated guesses until a recorded `qos_trace` calibrates them.
use super::*;

mod link;
pub use link::*;
mod scenario;
pub use scenario::*;
mod run;
pub use run::*;
mod summary;
pub use summary::*;
mod scenarios;
pub use scenarios::*;
mod bounds;
pub use bounds::*;
