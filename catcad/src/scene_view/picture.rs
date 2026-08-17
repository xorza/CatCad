//! What the view has drawn, and the room it was drawn in.

use std::cell::RefCell;
use std::rc::Rc;

use aperture::{Aim, Camera, Extent, Highlight, Hit, Lit, Renderer, Tint};
use glam::Vec3;
use palantir::{GpuPaint, Rect};
use silverpoint::ConstraintId;

use crate::lens::Lens;
use crate::model::Models;
use crate::paint;
use crate::paint::layout::Layout;
use crate::paint::marks::Placed;
use crate::paint::showing::Showing;
use crate::part::Part;
use crate::scene_view::aimed::Aimed;
use crate::selection::Selection;
use crate::timeline::FeatureId;

/// What the thing under the cursor looks like. How far forward a highlight
/// reads is the renderer's, not this — see `Highlight`.
const HOVERED: Highlight = Highlight {
    tint: Tint::Ink(Vec3::new(1.0, 0.85, 0.25)),
    scale: 1.8,
};

/// What something picked out looks like.
///
/// Green, which is the one hue the drawing does not already use: its own
/// colours run blue through yellow to orange for how much freedom is left, and
/// red for pinned, and a selection that reused any of them would be saying two
/// things in one colour. Lifted like the hover and drawn a little smaller, so
/// that the thing under the cursor still reads over the rest of what is picked.
const SELECTED: Highlight = Highlight {
    tint: Tint::Ink(Vec3::new(0.30, 0.95, 0.45)),
    scale: 1.5,
};

/// What a datum's axes look like singled out, hovered and picked out alike.
///
/// Brighter rather than recoloured, because an axis arrow is *saying something*
/// in its colour — which of the two it is — and the looks above exist to
/// override exactly that. Lighting the x arrow yellow would light up an arrow
/// that had stopped being the x arrow.
///
/// One look for both, where everything else has two. A datum is a place to
/// work rather than a thing to gather, so there is no state to tell apart:
/// what a hover means here is only "this is what pressing would take".
///
/// Unscaled, unlike the two above, and by the constructor rather than by hand:
/// a shape that keeps its own colour is one already saying what it is, and
/// growing it would move the control out from under the cursor pointing at it.
const AXIS_LIT: Highlight = Highlight::lifted(1.9);

/// How `part` reads when it has been singled out.
///
/// One place rather than two matches at the call, so that a kind whose
/// highlight is its own cannot be given the general look by whichever of the
/// two branches was written second.
fn singled(part: Part, ordinary: Highlight) -> Highlight {
    match part {
        Part::Plane(_) => AXIS_LIT,
        _ => ordinary,
    }
}

/// The picture of a document this view last wrote, and everything it takes to
/// keep one.
///
/// **The half of a view that has nothing to do with the pointer.** It holds the
/// scene, the names a pick reports through and the room both were made in; it
/// answers what it has drawn and where; and it is written by exactly the calls
/// that draw. What the pointer is *doing* is held beside it — see
/// [`SceneView`](crate::scene_view::SceneView) — and the two meet only where a
/// frame makes them: a pick, which is a question about what was drawn asked with
/// a cursor.
///
/// The scene is behind an [`Rc<RefCell<_>>`] because palantir paints from a
/// command recorded during the pass and run at submit, so the renderer has to
/// outlive the record. That is also why every borrow of it opens and closes
/// inside one call here: a caller holding one open across a second call is the
/// panic this type exists to make unreachable — see [`Picture::under`].
#[derive(Debug)]
pub(super) struct Picture {
    renderer: Rc<RefCell<Renderer>>,
    /// What was laid out, and what it claims to describe.
    ///
    /// The view's rather than the drawing's, for the same reason the scene is:
    /// what it holds describes this view's picture of the drawing and would
    /// mean nothing to another. It says which revision it drew and is written
    /// only by the call that draws — see [`Layout`].
    layout: Layout,
    /// What the renderer was last told to light: the hover and the selection,
    /// rebuilt every settle. Kept for its room rather than its contents, so a
    /// frame that lights the same set as the last asks the heap for nothing.
    lit: Vec<Lit>,
}

impl Picture {
    /// A picture of `models`, laid out as they stand.
    ///
    /// Lays the drawing out itself rather than being handed a scene, which is
    /// what lets it say honestly which revision it has drawn — the one claim it
    /// makes about its own contents is one it is in a position to make.
    ///
    /// Everything in the scene comes out of the document, solids included. It
    /// did not always: the solids used to be scenery handed in from outside and
    /// written once, because nothing in a document could yet *make* one. Now a
    /// step does, so they are laid out with the rest of it and by the same call
    /// — which is also what lets one be pointed at.
    pub(super) fn new(models: Models<'_>) -> Self {
        let mut layout = Layout::default();
        let scene = paint::scene(models, &mut layout);
        Self {
            renderer: Rc::new(RefCell::new(Renderer::new(scene))),
            layout,
            lit: Vec::new(),
        }
    }

    /// The renderer as the one thing a paint command needs.
    ///
    /// Handed out rather than shown from here, because showing it is declaring a
    /// *widget* — an id, what it senses, whether it takes focus — and none of
    /// that is the picture's. What is the picture's is that the thing painted
    /// from is the very renderer these calls write into.
    pub(super) fn painting(&self) -> Rc<RefCell<dyn GpuPaint>> {
        self.renderer.clone()
    }

    /// What the picture holds occupies in world space, or `None` if it holds
    /// nothing — what a camera is aimed at to take the whole of it in.
    pub(super) fn extent(&self) -> Option<Extent> {
        self.renderer.borrow().scene().extent()
    }

    /// Lay the drawing out again if it has moved, and cut the controls against
    /// `lens`.
    ///
    /// **Two schedules, one call.** The drawing is rewritten only when the
    /// drawing moves — the layout compares what it describes against what it is
    /// handed and returns without writing a batch — where a control holds its
    /// size on screen, so it is built against the lens and the lens moving is
    /// what invalidates it. Gating the two together would mean re-cutting every
    /// face and solid on every frame of an orbit; splitting them across two
    /// calls would let a caller run one and forget the other. See
    /// [`paint::gizmos::write`].
    ///
    /// Into the batches the renderer already holds, so a drag rewrites the
    /// drawing every frame without asking the heap for anything.
    ///
    /// No lens is a view that has not arranged yet: there is a drawing to write
    /// and no room to cut a control in.
    pub(super) fn redraw(&mut self, models: Models<'_>, showing: Showing, lens: Option<Lens>) {
        let mut renderer = self.renderer.borrow_mut();
        paint::redraw(models, &mut self.layout, showing, renderer.scene_mut());
        if let Some(lens) = lens {
            paint::gizmos::write(
                models,
                &mut self.layout,
                showing,
                lens,
                &mut renderer.scene_mut().gizmos,
            );
        }
    }

    /// Light what is picked out, and `pointed` over the top of it.
    ///
    /// One walk of the names for both, going the way the names run: what is
    /// picked out are the sketch's own handles and what is lit are the tags this
    /// layout gave them, and the same is true of what is hovered now that a
    /// hover is a part.
    ///
    /// The hover wins where something is both, which is what says the pointer
    /// would act on *it*.
    ///
    /// Unconditionally, and cheap when nothing moved: the renderer compares the
    /// set before it rewrites anything, so a still frame over a settled
    /// selection dirties no batch.
    pub(super) fn light(&mut self, pointed: Option<Part>, selection: &Selection) {
        self.lit.clear();
        for (tag, part) in self.layout.names().iter() {
            let look = if Some(part) == pointed {
                singled(part, HOVERED)
            } else if selection.contains(part) {
                singled(part, SELECTED)
            } else {
                continue;
            };
            self.lit.push(Lit { tag, look });
        }
        self.renderer.borrow_mut().highlight_all(&self.lit);
    }

    /// Paint what is here through `camera` from now on.
    ///
    /// Wholesale rather than on change: the document owns the camera and the
    /// scene holds the copy the next paint reads, so overwriting it every frame
    /// is what keeps the two from ever disagreeing. Copied here rather than
    /// pushed by the document, which has no business knowing a renderer exists.
    pub(super) fn aimed_through(&mut self, camera: Camera) {
        *self.renderer.borrow_mut().camera_mut() = camera;
    }

    /// What `aimed` is over, seen through `lens`, or `None` where it is over
    /// nothing the layout names.
    ///
    /// **The scene and the names asked as one question.** A pick is a hit on a
    /// primitive and what this picture calls it, and holding both is what lets
    /// this be one call: the scene used to arrive by argument, because the
    /// caller was holding the renderer open to write into and a second borrow
    /// would have panicked. Nothing holds one open across this now — every
    /// borrow here is opened and closed by the call that needs it — so the
    /// question is asked of the picture rather than of a scene the caller
    /// happened to be carrying.
    ///
    /// Aimed through whatever lens the caller holds, which is built on the
    /// **document's** camera. The renderer keeps a copy of that camera, written
    /// at the end of every frame — see [`Picture::aimed_through`] — so a lens
    /// built on *that* would answer through wherever the camera was before this
    /// frame's orbit.
    pub(super) fn under(&self, aimed: Aimed, lens: Lens) -> Option<Under> {
        let aim = aimed.aim(lens);
        let renderer = self.renderer.borrow();
        let hit = renderer.scene().nearest(aim)?;
        Some(Under {
            aim,
            hit,
            part: self.layout.names().get(hit.tag)?,
        })
    }

    /// Where the drawing put the mark for the relation `of` names.
    ///
    /// Forwarded rather than the layout being handed out, because a layout is
    /// written by exactly one call and everything else reads one answer out of
    /// it — see [`Layout`].
    pub(super) fn placed(&self, of: ConstraintId) -> Option<Placed> {
        self.layout.placed(of)
    }

    /// Where the region at `region` of `sketch` lands on screen, or `None` where
    /// the projection draws none of it.
    ///
    /// Here rather than in [`crate::prompt`] because of the one thing this owns
    /// and nothing else does: the [`Filler`](silverpoint::Filler) the layout
    /// keeps. A region's shape is cut rather than read, and cutting it through
    /// the same filler the sheets use is what makes a form stand clear of
    /// exactly what is drawn. The lens comes in from outside, being the caller's
    /// frame rather than the picture's.
    ///
    /// `&mut self` to cut the region where the layout has not already, not to
    /// write anything drawn — see [`Cut`](crate::paint::cut::Cut), which is what
    /// keeps that off every frame.
    pub(super) fn region_footprint(
        &mut self,
        models: Models<'_>,
        sketch: FeatureId,
        region: usize,
        lens: Lens,
    ) -> Option<Rect> {
        let cut = self.layout.region(models, sketch, region)?;
        lens.footprint(cut.corners().iter().copied())
    }
}

/// What the pointer is over: the aim it was asked through, what the scene
/// answered, and what the layout calls it.
///
/// **One question the hover, the click and the press all ask.** They asked it
/// three times over in three spellings — build an aim through the document's
/// camera, take the scene's nearest, look the tag up in the names — and two of
/// them once came apart over which camera to aim through, which is the argument
/// [`Picture::under`] carries now. Three callers reading one answer is what
/// stops a fourth from inventing a second.
///
/// The aim rides along because a press wants it afterwards, to resolve where the
/// grab landed on the motion. Taking it off the same value is what keeps the hit
/// and the ray from coming from two viewpoints.
#[derive(Debug, Clone, Copy)]
pub(super) struct Under {
    pub(super) aim: Aim,
    pub(super) hit: Hit,
    pub(super) part: Part,
}

/// The reach-ins two layers in — see [`CatCad::internals`](crate::internals),
/// where the shape and both its gates are argued.
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use std::cell::RefCell;
    use std::rc::Rc;

    use aperture::{Renderer, Tag};

    use crate::part::Part;
    use crate::scene_view::picture::Picture;

    impl Picture {
        /// The renderer being drawn, for a harness that wants to edit the scene
        /// or move the camera without going through a pointer.
        ///
        /// The picture lays itself out from the document and paints itself from
        /// what that left, so there is nothing in the application that could
        /// want this — which is the module's rule rather than the renderer's,
        /// and argued there.
        pub(crate) fn renderer(&self) -> &Rc<RefCell<Renderer>> {
            &self.renderer
        }

        /// What `tag` stands for in the layout this picture last made.
        ///
        /// For a harness asking what a press would find without a press to ask
        /// it through — a test sweeping candidate cursors for something to
        /// grab, or the visual suite checking that what it drew is what the
        /// pointer reports.
        ///
        /// Whole parts rather than entities, because a plane is one of the
        /// things a press can take hold of and has no entity to be narrowed to.
        /// A sweep after geometry narrows it itself.
        pub(crate) fn part(&self, tag: Tag) -> Option<Part> {
            self.layout.names().get(tag)
        }
    }

    /// The `test`-only half — see [`CatCad::internals`](crate::internals).
    #[cfg(test)]
    mod looking {
        use aperture::Lit;

        use crate::scene_view::picture::Picture;

        impl Picture {
            /// What the renderer was last told to light.
            ///
            /// The set rather than the renderer's answer, because the renderer
            /// is told and does not report: what a test asking whether hovering
            /// one arrow lit the whole gizmo needs is the list that was handed
            /// over, tags and looks together.
            pub(crate) fn lit(&self) -> &[Lit] {
                &self.lit
            }
        }
    }
}
