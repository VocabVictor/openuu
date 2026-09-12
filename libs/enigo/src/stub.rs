use super::*;

#[cfg(any(target_os = "android", target_os = "ios"))]
pub(super) struct Enigo;

impl Enigo {
    /// Constructs a new `Enigo` instance.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use enigo::*;
    /// let mut enigo = Enigo::new();
    /// ```
    pub fn new() -> Self {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        return Enigo {};
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Self::default()
    }
}

use std::fmt;

impl fmt::Debug for Enigo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Enigo")
    }
}
