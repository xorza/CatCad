//! One connected volume.

use crate::arena::Id;
use crate::solid::topology::shell::ShellId;

pub(crate) type LumpId = Id<Lump>;

/// A single connected piece of material: the shell around it, and one more
/// around every cavity shut inside it.
///
/// A cavity is a shell like any other and differs only in facing inward, which
/// is why a void is not a second kind of thing here. A [`Body`] holding several
/// lumps is a body in several pieces — which a boolean can produce without
/// anything above having to arrange for it.
///
/// [`Body`]: super::body::Body
#[derive(Debug)]
pub(crate) struct Lump {
    pub(crate) outer: ShellId,
    /// The cavities, if any. Empty for everything a single extrusion makes: a
    /// hole through a profile is a hole *through*, which the one shell goes
    /// round rather than a second shell sitting inside it.
    pub(crate) voids: Vec<ShellId>,
}
