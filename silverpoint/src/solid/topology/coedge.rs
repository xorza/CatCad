//! One face's use of one edge.

use crate::sided::Sided;
use crate::solid::topology::edge::EdgeId;

/// An edge as one face's loop walks it.
///
/// [`Sided`] over an [`EdgeId`], which is the whole of what a body's version of
/// a half-edge is. What it means and why it is stored inline are that type's;
/// what is here is which arena the edge lives in.
pub(crate) type Coedge = Sided<EdgeId>;
