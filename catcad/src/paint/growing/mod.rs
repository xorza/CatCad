//! A solid the user is still deciding the depth of.

use silverpoint::{Body, Boolean, Builder, Extrusion, Operation, Revolution, Step};

use crate::build::bodied;
use crate::lens::Lens;
use crate::model::Models;
use crate::paint::LIVE_FACES;
use crate::paint::cut::Cut;
use crate::paint::gizmos::Carried;
use crate::profile::Profile;
use crate::prompt::Form;
use crate::timeline::Sweep;

/// The step a solid nobody has taken one for is grown by.
///
/// A body names its faces by which feature grew each of them, and there is no
/// feature here — so this names one no
/// [`FeatureId`](crate::timeline::FeatureId) will ever be. What reads
/// it back is the tagging: a face grown by this carries no tag, because there
/// is nothing yet for a tag to point at, where a face the *model* brought
/// through the boolean carries the one it always had.
pub(super) const UNTAKEN: Step = Step(u32::MAX);

/// What the drawing has to show while a depth is being decided.
///
/// Three answers rather than a flag, because the third is not a failure. A
/// preview that cannot show the answer shows the tool, which is what was on
/// screen before there was a boolean to run in a frame — honest about being a
/// tool rather than a lie about being a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Deciding {
    /// Nothing at all, and the last one must not be left standing.
    Nothing,
    /// The whole answer, model and all.
    ///
    /// So the document's own solids are **not** drawn beside it: the answer
    /// already holds the model, and drawing both would put the same geometry
    /// on screen twice fighting for one depth.
    Answer,
    /// The step's own solid, standing beside the model.
    ///
    /// What a first step that says cut comes to, what a refusal leaves, and
    /// what a tool too detailed to combine on a frame's clock is shown as.
    Beside,
}

/// The room a preview is raised and combined in.
///
/// A bundle for the reason the rebuild of a real step has one: every field is
/// held by the layout across frames and lent for the length of one, and three
/// `&mut`s in a row at the call site would be three chances to hand over the
/// wrong one.
#[derive(Debug)]
pub(super) struct Raising<'a> {
    pub(super) builder: &'a mut Builder,
    pub(super) boolean: &'a mut Boolean,
    /// Where the tool is raised, before it is put together with what stands.
    pub(super) raised: &'a mut Body,
    /// Where the profile is resolved to positions among its sketch's faces.
    ///
    /// Held by the layout and refilled rather than made here, for the reason
    /// the bodies are: a depth typed a digit at a time resolves this on every
    /// frame the form is open.
    pub(super) regions: &'a mut Vec<usize>,
}

/// A solid being decided: a region, how deep it currently reads, and what it
/// does to what stands.
///
/// What an open form hands the drawing, so the solid is on screen from the
/// moment it is asked for. Everything a body is built from is here — an
/// arrangement, a region, a plane, a sweep and an operation — so all of this is
/// drawable without a step existing, and cancelling the form leaves the
/// timeline never having heard of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Growing<'a> {
    /// Which opening the form is, which is what tells two of them apart.
    ///
    /// A profile is a list of curves and this is [`Copy`], so what the picture
    /// *keeps* between frames is this rather than the name — see
    /// [`Stamped`](crate::paint::layout::Stamped). A form closing and another
    /// opening moves nothing in the document, so nothing else here would say
    /// they are two.
    pub(crate) form: Form,
    /// The regions being swept, by name.
    ///
    /// Borrowed off the open form rather than resolved here, and resolved
    /// afresh wherever it is built: a position is only good for the arrangement
    /// it was read from, and a form outlives several.
    pub(crate) profile: &'a Profile,
    /// What is done to those regions, resolved — see [`Sweep`], which the
    /// timeline's own steps are read through as well.
    pub(crate) sweep: Sweep,
    pub(crate) operation: Operation,
}

impl Growing<'_> {
    /// Where the arrow that carries this stands, or `None` where the sketch no
    /// longer holds the region it names.
    ///
    /// At the middle of the far cap, so it sits on the face it is carrying and
    /// travels with it as the depth is typed — a handle that stayed at the base
    /// would stop being on the thing it moves the moment it moved anything.
    ///
    /// Where *on* that cap is [`Cut`]'s, and `cut` is handed in already cut for
    /// this region. It was worked out here, which meant putting the region
    /// through the filler and scanning its triangles on every frame the camera
    /// moved — a drawing's cost paid on the camera's clock, for an answer only
    /// the drawing can move. What is left is the part that does move with the
    /// camera: how far the depth carries the arrow, and which way it is laid to
    /// face the viewer.
    pub(super) fn carried(self, models: Models<'_>, cut: &Cut, lens: Lens) -> Option<Carried> {
        // A whole turn has nothing to drag. What a revolve's own handle would
        // move is the line it spins about, which is a segment of a drawing and
        // is dragged there.
        let Sweep::Carried(distance) = self.sweep else {
            return None;
        };
        let model = models.at(self.profile.sketch())?;
        let plane = model.plane();
        let normal = plane.normal().as_vec3();
        Some(Carried::new(
            cut.inside() + normal * distance as f32,
            normal,
            // Square to the view rather than to the sketch: the outline is
            // flat, and one laid out in a plane of the sketch's own collapses
            // to a line the moment the camera comes round to look along it —
            // which for a handle is the moment it stops being grabbable. The
            // fallback is the case where the camera looks straight down the
            // arrow, where there is no widest side to turn and the whole shape
            // is a dot whichever way it is laid out.
            lens.facing()
                .cross(normal)
                .try_normalize()
                .unwrap_or(plane.x.as_vec3()),
        ))
    }

    /// Build what it currently reads as into `into`, and say what that is.
    ///
    /// **The answer rather than the tool.** A cut previewed as the prism it
    /// takes away shows what is about to go rather than what will be left,
    /// which is the one thing a person deciding a depth is looking at. So this
    /// runs the same two steps the step's own rebuild will: raise the tool,
    /// then put it together with what stands.
    ///
    /// **Raised by the drawing rather than kept by the document**, unlike every
    /// solid there is a step for: this one belongs to a form that is still
    /// open, and it changes whenever the depth does. What the document keeps is
    /// a [`Bodied`](crate::build::bodied::Bodied) per *step*, and there is no
    /// step yet.
    ///
    /// Into bodies the layout holds, for the reason the layout holds every
    /// other buffer: a depth typed a digit at a time rebuilds this on every
    /// frame the form is open, and bodies refilled in place reach the heap
    /// once.
    pub(super) fn body(
        self,
        models: Models<'_>,
        raising: Raising<'_>,
        into: &mut Body,
    ) -> Deciding {
        let Raising {
            builder,
            boolean,
            raised,
            regions,
        } = raising;
        // The sketch has gone, or one of the regions has: either way there is
        // no solid to show and the last one must not be left on screen.
        let Some(model) = models.at(self.profile.sketch()) else {
            into.clear();
            return Deciding::Nothing;
        };
        if !self.profile.faces_in(model.arrangement(), regions) {
            into.clear();
            return Deciding::Nothing;
        }
        match self.sweep {
            Sweep::Carried(distance) => {
                let extrusion = Extrusion::new(
                    model.arrangement(),
                    regions,
                    model.plane(),
                    distance,
                    UNTAKEN,
                );
                builder.extrude(&extrusion, raised);
            }
            // The line has been rubbed out from under the form, which is
            // nothing to show and the last one must not be left standing.
            Sweep::Spun(None) => {
                into.clear();
                return Deciding::Nothing;
            }
            Sweep::Spun(Some(axle)) => {
                let revolution = Revolution::new(
                    model.arrangement(),
                    regions,
                    model.plane(),
                    axle.at,
                    axle.along,
                    UNTAKEN,
                );
                builder.revolve(&revolution, raised);
            }
        }

        let standing = models
            .model()
            .map(|(_, body)| body)
            .filter(|body| !body.is_empty());
        if standing.is_some() && raised.names().count() > LIVE_FACES {
            std::mem::swap(into, raised);
            return Deciding::Beside;
        }
        if !bodied::merged(boolean, standing, raised, self.operation, into) {
            // Refused, so the tool stands beside the model — which is what the
            // commit would leave too. A preview that showed an answer the step
            // cannot build would be worse than one that shows the tool.
            std::mem::swap(into, raised);
            return Deciding::Beside;
        }
        match standing {
            // A cut that takes everything leaves an answer with nothing in it,
            // and an answer with nothing in it is the right picture: the model
            // it stands in for is not drawn beside it.
            Some(_) => Deciding::Answer,
            // Nothing stood, so what came of it says which of the three this
            // was — a join is the tool itself and the other two are nothing.
            None if into.is_empty() => Deciding::Nothing,
            None => Deciding::Beside,
        }
    }
}

#[cfg(test)]
mod tests;
