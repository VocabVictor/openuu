//! Regression tests for sustained capacity drops and path-delay/content changes.
//! Capacity drops use the closed-loop model; delay and activity fixtures are open-loop.
use super::*;

mod oscillation;
mod recovery;
mod recovery_pace;
