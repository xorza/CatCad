//! The 3D view, and everything the pointer does to it.

use aperture::{Camera, Extent};
use palantir::{Configure, GpuView, Sense, Sizing, Ui, WidgetId};
use silverpoint::Entity;

use crate::build::Build;
use crate::document::Document;
use crate::intent::Intents;
use crate::lens::Lens;
use crate::look::Theme;
use crate::model::Models;
use crate::paint::showing::Showing;
use crate::part::Part;
use crate::prompt::{Asking, Prompt, Stands};
use crate::scene_view::picture::Picture;
use crate::scene_view::pointing::Pointing;
use crate::session::Session;
use crate::timeline::FeatureId;

mod aimed;
mod click;
mod gesture;
mod picture;
mod pointing;

/// Which pane of the view's renderer the drawing is.
///
/// The renderer is built holding it, so it is the first — and whatever is
/// pushed over it is furniture, which the drawing is not.
pub(crate) const DRAWING: usize = 0;

/// Radians of orbit per logical pixel of drag.
///
/// Here rather than beside the pointer that spends it, because the orientation
/// cube is dragged for the same reason and has to turn at the same rate: two
/// gestures that both orbit and disagree about how fast read as two different
/// cameras.
pub(crate) const ORBIT_RATE: f32 = 0.008;

/// What the viewport is recorded under.
///
/// Named rather than derived from the call site, because the pointer is read a
/// phase *before* the view is drawn — see [`SceneView::poll`] — and a caller can
/// only learn an `auto_id` from the response the drawing hands back.
fn view_id() -> WidgetId {
    WidgetId::from_hash("catcad.viewport")
}

/// One scene, drawn into a viewport the pointer can orbit, zoom and drag.
///
/// **Two halves and the frame that sequences them.** What has been drawn is the
/// [`Picture`]'s — the scene, the names a pick answers through, the room both
/// were made in — and what the pointer has established is [`Pointing`]'s: which
/// gesture a press settled on, how far the middle button has travelled, what the
/// band is showing. They meet in exactly one question, asked twice a frame: what
/// is under the cursor, which is a question about the picture put with the
/// pointer's own cursor.
///
/// The app above holds a view rather than a renderer plus the loose fields that
/// were only ever about pointing at one, and the view holds two things rather
/// than ten.
#[derive(Debug)]
pub(crate) struct SceneView {
    /// What has been drawn, and the room it was drawn in.
    picture: Picture,
    /// What the pointer has established about it, and what it is in the middle
    /// of.
    pointing: Pointing,
}

impl SceneView {
    /// A view of `document`, laid out as it stands.
    ///
    /// A document and a build rather than a [`Models`], because the pairing is
    /// what the app above holds and the view is what it holds one for. Laying
    /// out is [`Picture::new`]'s and argued there; a pointer that has
    /// established nothing yet is the whole of the other half.
    pub(crate) fn new(
        document: &Document,
        build: &Build,
        theme: &Theme,
        editing: Option<FeatureId>,
    ) -> Self {
        Self {
            picture: Picture::new(document.models(build, editing), theme),
            pointing: Pointing::default(),
        }
    }

    /// What the view holds occupies in world space, or `None` if it holds
    /// nothing — what a camera is aimed at to take the whole of it in.
    ///
    /// One caller, in `CatCad::build`, and worth the method anyway: how big
    /// what you are showing is a fair question to ask a view, and the caller
    /// would otherwise reach through the renderer for the drawing's own pane to
    /// ask the scene the same thing.
    pub(crate) fn extent(&self) -> Option<Extent> {
        self.picture.extent()
    }

    /// How this view is looking at the drawing through `camera`, or `None`
    /// before it has arranged — the room the pointer was measured in, paired
    /// with the viewpoint the document holds. See [`Pointing::lens`].
    pub(crate) fn lens(&self, camera: Camera) -> Option<Lens> {
        self.pointing.lens(camera)
    }

    /// What the pointer is working on, if anything — what the status line
    /// names. See [`Pointing::hovered`], which is where it parts company with
    /// what is *lit*.
    pub(crate) fn hovered(&self) -> Option<Part> {
        self.pointing.hovered()
    }

    /// Take what the pointer over the view asks for, before anything is drawn.
    ///
    /// The pointer reads the picture and never writes one — see
    /// [`Pointing::poll`], which is where the asking happens. Handed here rather
    /// than reached for, because the two halves are fields of this and a caller
    /// outside could pair a pointer with a picture of another view.
    pub(crate) fn poll(
        &mut self,
        ui: &mut Ui,
        document: &Document,
        session: &Session,
        intents: &mut Intents,
    ) {
        self.pointing
            .poll(ui, document, session, &self.picture, intents);
    }

    /// Record the viewport.
    ///
    /// Draws and reads nothing: what the pointer over it asked for was taken in
    /// [`SceneView::poll`] a phase earlier and has already landed, so this paints
    /// the drawing as this frame's edits left it rather than as it stood before
    /// them.
    pub(crate) fn draw(&self, ui: &mut Ui) {
        GpuView::new(self.picture.painting())
            .id(view_id())
            .sense(Sense::CLICK | Sense::DRAG | Sense::SCROLL | Sense::PINCH)
            // Focusable so that a press on the drawing takes focus *off*
            // whatever had it — which is how clicking away from the field open
            // over a dimension closes it. The view claims no key class of its
            // own: every chord catcad binds is the application's, and the field
            // is a widget with its own scope rather than something drawn in
            // here for this to arbitrate for.
            .focusable(true)
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui);
    }

    /// Everything that reads the document once it has finished moving: lay the
    /// drawing out again if it has moved, light what the pointer is over, and
    /// hand the renderer the camera to paint through.
    ///
    /// After the intents rather than during them, which is what makes the
    /// answers agree with each other. The highlight is picked against geometry
    /// this frame's drag has already reached, and the camera the renderer is
    /// given is the one every gesture this frame settled on. Both are the same
    /// mistake avoided twice: reading a document that is still being written.
    ///
    /// Reads the document and never writes it. What a view holds is a picture
    /// of a document — the scene, the names, the highlight — and a picture is
    /// made by looking.
    ///
    /// Still in time for the frame being drawn. `GpuView` records a paint
    /// command holding the renderer and calls it at submit, after the record
    /// pass has returned, so writing to it here is writing to what is about to
    /// be painted.
    pub(crate) fn settle(
        &mut self,
        document: &Document,
        build: &Build,
        theme: &Theme,
        session: &Session,
    ) {
        // How the drawing is looked at now this frame's edits have landed,
        // which is what the controls are cut against and what the hover is
        // resolved through — the document's own camera rather than the copy the
        // picture is handed below, which is still wherever this frame's orbit
        // found it.
        let lens = self.lens(document.camera());
        let models = document.models(build, session.editing());
        // **Derived once, for both halves of the picture.** They are written on
        // different schedules — the drawing when the drawing moves, the controls
        // whenever the camera does — and read apart they could disagree about
        // what a gesture is showing: a solid and the arrow carrying it, or a
        // dimension's figure and the line under it, drawn about two different
        // answers.
        let open = session.prompt();
        let showing = Showing {
            band: self.pointing.preview(),
            typed: open.and_then(Prompt::marks),
            growing: open.and_then(|open| open.growing(models)),
        };
        self.picture.redraw(models, theme, showing, lens);
        // What the pointer is over, resolved against the picture that was just
        // written and lit on it — the two halves meeting in the one question
        // they share. What is *named* is the pointer's own answer and stays
        // there; see [`Pointing::settled`].
        let pointed = self.pointing.settled(&self.picture, lens);
        self.picture.light(theme, pointed, session.selection());
        self.picture.aimed_through(document.camera());
    }

    /// Where a form open against the drawing stands, or `None` where the view is
    /// drawing none of what it is about.
    ///
    /// **Where in the world a form is about is the view's**, which is the half
    /// [`prompt`](crate::prompt) says it cannot answer: every arm below either
    /// projects a point the drawing placed or measures the footprint of geometry
    /// the drawing cut, and both are questions about this picture of the
    /// document. What the *form* then does with the answer — how it stands,
    /// what Enter means — is the prompt's, and none of it is here.
    ///
    /// Answered afresh every frame rather than remembered. A form outlives
    /// orbits, edits and undos, so where it stands is a reading of the frame it
    /// is being shown in — see [`Asking::Extrude`], which holds a name for this
    /// to resolve rather than a position for it to trust.
    ///
    /// `None` is a frame the form is not shown for rather than a form that
    /// closes: a camera turning back brings the geometry round again, and a
    /// sketch left for another takes its marks off screen without taking the
    /// form's dimension out of the document.
    ///
    /// `&mut self` for the filler's scratch alone — see
    /// [`Picture::region_footprint`].
    pub(crate) fn stands(
        &mut self,
        about: &Asking,
        models: Models<'_>,
        lens: Lens,
    ) -> Option<Stands> {
        match about {
            Asking::Dimension { part } => {
                let part = *part;
                // Where the mark *would* be drawn, which is where the field
                // stands instead — see [`paint::redraw`], which leaves out the
                // mark of whatever is being typed into. Read off the layout
                // rather than worked out again, so the field lands on the lane
                // the drawing gave the mark and not on the one an unstacked
                // anchor would have.
                //
                // Of the sketch the part names rather than of every sketch
                // there is: a part says which drawing it belongs to, and
                // [`Model::entity`] refuses another's outright — so the walk
                // was only ever going to answer on one of them.
                let found = part
                    .sketch()
                    .and_then(|sketch| models.at(sketch))
                    .and_then(|model| match model.entity(part) {
                        Some(Entity::Constraint(id)) => Some((model.drawing(), id)),
                        _ => None,
                    });
                // Never missing, on the same terms `Session::prune` guarantees:
                // a form open over a dimension an undo took away is closed
                // before the frame that would draw it.
                let (drawing, id) =
                    found.expect("a form is open over a dimension the drawing no longer holds");
                // Unplaced is a different thing from gone, and only the second
                // is a broken promise. A drawing places the marks of the sketch
                // it is *in*, so a form outliving a click that opened another
                // sketch has a dimension the layout knows nothing about.
                self.picture
                    .placed(id)
                    .and_then(|placed| {
                        // The middle of the mark's own box rather than the point
                        // it hangs off, and worked out by the drawing: the box
                        // rises up the *run's* frame, which is the sketch
                        // plane's — see [`Mark::centre`].
                        lens.screen_of(placed.mark.centre(drawing, lens))
                    })
                    .map(Stands::Over)
            }
            // Beside the centre already placed, which is all there is of the
            // circle until a radius says otherwise.
            Asking::Circle { sketch, center } => models
                .at(*sketch)
                .and_then(|model| {
                    // The whole rim rather than the centre and the pointer
                    // between them: a circle reaches its radius in *every*
                    // direction, so a box drawn to wherever the band happens to
                    // have got to is a quarter of one — and a form standing
                    // clear of that quarter stands on the rest.
                    let middle = model.drawing().at(*center);
                    let radius = self
                        .pointing
                        .band_rim()
                        .map_or(0.0, |to| middle.distance(to));
                    lens.footprint(model.rim_around(middle, radius))
                })
                .map(Stands::Beside),
            // Resolved here rather than remembered, for the reason the form
            // holds a name at all: the arrangement it was read from is not the
            // one it is being drawn against.
            Asking::Extrude { profile, .. } | Asking::Revolve { profile, .. } => profile
                .first_face_of(models)
                .and_then(|region| {
                    self.picture
                        .region_footprint(models, profile.sketch(), region, lens)
                })
                .map(Stands::Beside),
        }
    }
}

/// The reach-ins one layer in — see [`CatCad::internals`](crate::internals),
/// where the shape and both its gates are argued.
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::scene_view::SceneView;
    use aperture::{Pane, Renderer, Tag};
    use std::cell::{Ref, RefCell, RefMut};
    use std::rc::Rc;

    impl SceneView {
        /// Whether the pointer is over the thing `tag` names, as this frame's
        /// layout resolved it.
        ///
        /// A question rather than the tag itself, because the view keeps what it
        /// is hovering as a [`Part`](crate::part::Part) and a tag is only how
        /// the *scene* reported it — answering with one would mean keeping a
        /// second copy of the map that already turns one into the other.
        ///
        /// Here rather than beside [`SceneView::hovered`] because nothing in the
        /// application asks it: what the view lights and what the readout names
        /// are both parts, and a tag is what only a *harness* holds — it reads
        /// one off the scene it just drew and wants to know whether the pointer
        /// found it.
        pub(crate) fn hovering(&self, tag: Tag) -> bool {
            self.picture
                .part(tag)
                .is_some_and(|part| Some(part) == self.pointing.hovered())
        }

        /// The pane the drawing is in — see
        /// [`Picture::pane`](crate::scene_view::picture::Picture::pane), where
        /// the borrow it hands back is argued.
        pub(crate) fn pane(&self) -> Ref<'_, Pane> {
            self.picture.pane()
        }

        /// The same to write into.
        pub(crate) fn pane_mut(&self) -> RefMut<'_, Pane> {
            self.picture.pane_mut()
        }

        /// The renderer being drawn — see
        /// [`Picture::renderer`](crate::scene_view::picture::Picture::renderer),
        /// which is the field this reaches and where it is argued.
        pub(crate) fn renderer(&self) -> &Rc<RefCell<Renderer>> {
            self.picture.renderer()
        }
    }

    /// The `test`-only half — see [`CatCad::internals`](crate::internals).
    ///
    /// Named for looking rather than for picking, which is what it held when
    /// there was one of them: what a tag stands for and what a gesture is
    /// showing are both readings of what the view is holding, and neither is a
    /// pick.
    #[cfg(test)]
    mod looking {
        use crate::paint::marks::mark::Mark;
        use aperture::{Lit, Tag};
        use glam::Vec3;
        use silverpoint::ConstraintId;

        use crate::part::Part;
        use crate::preview::Preview;

        use crate::scene_view::SceneView;

        impl SceneView {
            /// The shape a tool is half-way through, for a test that wants to
            /// read what a gesture is *showing* rather than what it drew.
            ///
            /// A preview is the one thing the view holds that is neither in the
            /// document nor in the picture it can be measured out of: a
            /// dimension's is the whole constraint the next click would state,
            /// so reading it is how a test asks whether the preview and the
            /// click agree without picking a mark no paint has measured.
            pub(crate) fn preview(&self) -> Option<Preview> {
                self.pointing.preview()
            }

            /// What `tag` stands for in the layout this view last made — see
            /// [`Picture::part`](crate::scene_view::picture::Picture::part).
            pub(crate) fn part(&self, tag: Tag) -> Option<Part> {
                self.picture.part(tag)
            }

            /// What the renderer was last told to light — see
            /// [`Picture::lit`](crate::scene_view::picture::Picture::lit).
            pub(crate) fn lit(&self) -> &[Lit] {
                self.picture.lit()
            }

            /// The mark the drawing put up for the relation `of` names.
            ///
            /// Named for what a harness wants rather than for the method it
            /// forwards to, which the view keeps to itself: where a form stands
            /// over a mark is the view's own to answer — see
            /// [`SceneView::stands`] — and what is left for a caller outside is
            /// picking a dimension by how its mark came out, which is a test
            /// choosing a fixture.
            pub(crate) fn marked(&self, of: ConstraintId) -> Option<Mark> {
                self.picture.placed(of).map(|placed| placed.mark)
            }

            /// Where every marker in the scene sits, in the order they are
            /// drawn.
            ///
            /// What the *renderer* is still showing, which is not the same
            /// question as where the document says its points are — a frame
            /// nothing has redrawn shows the drawing as it stood.
            pub(crate) fn markers(&self) -> Vec<Vec3> {
                self.pane()
                    .scene
                    .points
                    .iter()
                    .map(|point| point.position)
                    .collect()
            }

            /// Where the band has carried the rim of the circle being drawn.
            ///
            /// The same kind of reach-in as the rest of this module: what a form
            /// standing beside a half-drawn circle is placed against is the
            /// view's, and a test asking how far the band has been carried is
            /// asking what the pointer made of a gesture rather than what the
            /// drawing came out as.
            pub(crate) fn banded(&self) -> Option<Vec3> {
                self.pointing.band_rim()
            }
        }
    }
}

#[cfg(test)]
mod tests;
