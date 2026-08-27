//! How long the application takes to change what it is showing.

use palantir::{AnimSpec, Easing};

/// The two transitions the application animates.
///
/// **No preset axis, unlike every other roster here.** A dark theme and a light
/// one disagree about colour and agree about time: how long a control takes to
/// lift is a property of the *interface* rather than of the palette, so this has
/// a `Default` and no `DARK` beside it.
#[derive(Debug, Clone)]
pub(crate) struct Motion {
    /// A control changing state under the pointer.
    ///
    /// Short enough that it never stands between a press and what the press
    /// did, and long enough that the change reads as one thing moving rather
    /// than as two frames of different pictures.
    pub(crate) lift: AnimSpec,
    /// The camera turning to a view the orientation cube was asked for.
    ///
    /// A quarter of a second, so it reads as a turn. Anything shorter is a cut,
    /// and a cut leaves the viewer to work out for themselves which way the
    /// model went. Eased at both ends, because the camera starts and ends at
    /// rest — a turn that began at full speed would read as a jump that then
    /// slowed down.
    pub(crate) turn: AnimSpec,
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            lift: AnimSpec::FAST,
            turn: AnimSpec::duration(0.25, Easing::InOutCubic),
        }
    }
}
