//! What a gesture asks the drawing to become.

use aperture::Projection;
use glam::Vec3;
use silverpoint::{Constraint, ConstraintId, Entity};

use crate::drawing::Grip;
use crate::drawing::anchor::Anchor;
use crate::timeline::FeatureId;

/// What the document answers, and the whole of what it answers.
///
/// Everything a document can be *asked* for, which is not quite everything that
/// changes one: an undo puts a snapshot back without passing through here.
/// Adding a step is asked for like anything else and only its *recording* is
/// special, a step that was not there having no before to put back. Everything
/// here reaches
/// [`Document::apply`](crate::document::Document), which matches it exhaustively
/// — so a new one added to this enum is a compile error until the document says
/// what to do with it, where a new one added beside it in [`Intent`] cannot
/// reach the document at all.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Change {
    /// Take what a drag has hold of to a point in the world.
    ///
    /// Names where the entity should end up rather than how far to move it,
    /// which is what lets a settling frame apply the same drag twice and land
    /// in the same place. See the note on clearing the inbox in `CatCad::record`.
    Drag {
        sketch: FeatureId,
        grip: Grip,
        to: Vec3,
    },
    /// Put a point where this click landed, held to whatever it landed on.
    ///
    /// An [`Anchor`] rather than a place, because where a click landed is only
    /// half of what it said: on an edge or a rim it also said what the new point
    /// is to be held to, and a position alone would have thrown that away.
    AddPoint { sketch: FeatureId, at: Anchor },
    /// Put a straight edge between these two ends.
    ///
    /// One intent for the whole edge, though it is asked for by two clicks and
    /// may make two points on the way. Nothing reaches the document until the
    /// second click, so a line abandoned half-drawn leaves no stray point
    /// behind — and the one that is finished is one step to take back rather
    /// than three.
    AddSegment {
        sketch: FeatureId,
        from: Anchor,
        to: Anchor,
    },
    /// Put a circle about `center`, out as far as `rim`.
    ///
    /// The rim says how big and nothing else: a radius is a number, so no point
    /// is made out there however the click that gave it landed.
    AddCircle {
        sketch: FeatureId,
        center: Anchor,
        rim: Anchor,
    },
    /// State this relation over the drawing.
    ///
    /// The whole constraint rather than what was picked and which button was
    /// pressed, because working out what a selection admits is the drawing's —
    /// see [`Model::offers`](crate::model::Model). What arrives here is
    /// already an answer, so a replayed pass states the same relation twice
    /// rather than reading a selection that has since moved on.
    Constrain {
        sketch: FeatureId,
        constraint: Constraint,
    },
    /// Restate a dimension at a new magnitude, and let the drawing settle onto
    /// it.
    ///
    /// Names the value it wants rather than a step to take, like everything
    /// here: a scrub sends one of these a frame and a replayed pass restates the
    /// same number, where "a bit larger" would grow twice over.
    Resize {
        sketch: FeatureId,
        constraint: ConstraintId,
        to: f64,
    },
    /// Put a dimension's number where this drag has taken it.
    ///
    /// Names the place it wants rather than how far to move, like everything
    /// here: a drag sends one of these a frame and a replayed pass puts the
    /// number in the same place, where "a little further" would travel twice
    /// over.
    ///
    /// A place in the *world*, which is then flattened onto the sketch and read
    /// against the dimension's own frame — see
    /// [`Sketch::place`](silverpoint::Sketch). An intent says where a gesture
    /// landed; what that comes to in the frame a number is stored in is the
    /// drawing's, and working it out here would be working it out without the
    /// geometry to work it out from.
    ///
    /// The one change to a sketch that moves no geometry. What it edits is what
    /// the drawing *shows* — and that is still the document's, because it is
    /// written down and taken back like everything else the drawing says.
    Place {
        sketch: FeatureId,
        constraint: ConstraintId,
        at: Vec3,
    },
    /// Take out geometry that duplicates other geometry and carries nothing.
    ///
    /// Names no geometry, unlike [`Change::Delete`] beside it, and needs to
    /// name none: what qualifies is a property of the drawing rather than of
    /// what is picked out, so a replayed pass asks the same question of the
    /// same sketch. The second pass finds nothing left to remove, which is the
    /// same answer arrived at twice rather than a second removal.
    Tidy { sketch: FeatureId },
    /// Take this out of the drawing, with whatever was built on it.
    ///
    /// Names what to remove rather than saying "the selection", for the same
    /// reason: a replayed pass would otherwise delete whatever is picked out by
    /// the time it ran, which after the first pass is nothing.
    Delete { sketch: FeatureId, entity: Entity },
    /// Grow a solid off a region of a sketch.
    ///
    /// The one change that *adds* a step rather than rewriting one, which is
    /// what [`About::Makes`] says of it and what the history has to be told
    /// before it can record anything: a step that was not there has no before.
    ///
    /// The region by position rather than as a
    /// [`Profile`](crate::profile::Profile), and that is forced: an intent is
    /// [`Copy`] and load-bearingly so, where a profile owns the list of curves
    /// naming it. Minting the durable name is the document's, one line later —
    /// see [`Document::apply`](crate::document::Document). Nothing is lost by
    /// the wait, because a position holds for as long as the arrangement it was
    /// read from does, and that is the frame this intent lands in.
    Extrude {
        sketch: FeatureId,
        region: usize,
        distance: f64,
    },
    /// Start a sketch on a plane.
    ///
    /// The second change that *adds* a step rather than rewriting one — see
    /// [`About::Makes`] — and the one that makes a plane worth drawing: without
    /// it a world plane is decoration, nothing open is a state only reachable by
    /// leaving, and an empty document is unreachable outright.
    ///
    /// **Names the plane and not the sketch**, which is forced the same way
    /// [`Change::Extrude`] is forced: the sketch does not exist when the gesture
    /// is read, so there is no handle to put here. Minting one is the document's,
    /// and *entering* it is the application's a line later — see
    /// [`Session::entered`](crate::session::Session), which is the one place the
    /// inbox is not what carries an answer back.
    ///
    /// Not idempotent in the way the rest of these are, and cannot be: a second
    /// pass starts a second sketch, where every change here names a state to
    /// arrive at and lands twice on the same one. What holds it to once is that
    /// nothing raises it a frame at a time — a button press and a double-click
    /// are each one frame's asking, where a drag is sixty.
    AddSketch { on: FeatureId },
    /// Take a step out of the recipe, with everything built on it.
    ///
    /// **Not [`Change::Delete`]**, which takes geometry out of a *drawing*. The
    /// two are the same gesture one level apart — a key press on what is picked
    /// out, taking the thing and whatever stood on it — and they are two changes
    /// because they name two different kinds of thing. Which one a press comes
    /// out as follows from what is picked.
    ///
    /// Names the head of the cascade and not the cascade, because what goes with
    /// it is the document's to work out — see
    /// [`Timeline::doomed`](crate::timeline::Timeline). A replayed pass would
    /// otherwise name steps the first pass had already taken away.
    ///
    /// The three planes the world comes with are refused, which is a rule about
    /// the document rather than about this: everything is measured from them,
    /// however many links back.
    DeleteStep { step: FeatureId },
    /// Move a step to a different place in the recipe.
    ///
    /// **A final position, clamped before it is asked for.** Where a step may go
    /// is a run bounded by what it is built on and what is built on it — see
    /// [`Timeline::moves_within`](crate::timeline::Timeline) — and whatever
    /// raises this clamps to that run, so an invalid position is unreachable
    /// rather than refused. Naming where it wants to end up is also what makes a
    /// replayed pass land in the same place, like everything else here; "one
    /// place up" asked twice would travel two.
    ///
    /// **It changes nothing built, today.** Every step resolves what it is built
    /// on by reference rather than by position, so the model comes out identical
    /// whatever order the recipe runs in. What order decides is what the tree
    /// shows, what the file writes, and — once there is a bar — how much of the
    /// recipe is built at all. It stops being cosmetic the moment a solid can be
    /// built on another.
    Reorder { step: FeatureId, to: usize },
    /// Take a plane to a new offset from the one it is measured off.
    ///
    /// Names the offset it wants rather than a step to take, like everything
    /// here: a drag sends one of these a frame and a replayed pass restates the
    /// same number, where "a little further" would travel twice over.
    ///
    /// The one change that edits no sketch. What it moves is where the sketches
    /// hanging off that plane *land*, and none of them says anything different
    /// for it — see [`Datum::Offset`](crate::timeline::feature::Datum).
    MovePlane { plane: FeatureId, to: f64 },
    /// Carry a solid to a new distance off the plane its region was drawn on.
    ///
    /// Names the distance it wants rather than a step to take, like
    /// [`Change::MovePlane`] above and for the same reason: a drag sends one of
    /// these a frame and a replayed pass restates the same number, where "a
    /// little deeper" would grow twice over.
    ///
    /// Signed, so this is also how a solid is flipped to the other side of its
    /// plane — see [`Feature::Extrude`](crate::timeline::feature::Feature).
    Carry { extrude: FeatureId, to: f64 },
    /// Turn the camera about what it is looking at, in radians.
    Orbit { yaw: f32, pitch: f32 },
    /// Move the camera in or out by a multiple of how far off it is.
    Dolly { factor: f32 },
    /// Slide the camera across the view, without turning it or changing how
    /// far off it is.
    ///
    /// A world step rather than the pixels the gesture was made of, like
    /// [`Change::Drag`] and for the same reason: what a pixel is worth depends
    /// on the viewport, and the viewport is the view's. The document has no
    /// screen and is owed no chance to guess at one.
    Pan { by: Vec3 },
    /// Look through this projection.
    Project(Projection),
}

impl Change {
    /// What this does to the timeline: makes a step, rewrites one, or neither.
    ///
    /// **One exhaustive match rather than three predicates.** The history asked
    /// three questions of every change back to back — does it create, which step
    /// is it about, does it belong to a gesture already under way — and two of
    /// them were exhaustive while the third was a `matches!`, so a variant added
    /// later could quietly join the wrong side of it. Every variant answers all
    /// of it here, once, which makes a new one a compile error in every
    /// dimension at the same time: a gesture mistaken for a single edit costs an
    /// extra press of undo, and **a creation mistaken for a rewrite is a step
    /// nothing can take back**.
    ///
    /// The three answers are one answer because they were never independent. A
    /// change that makes a step is about no step that is already there — the one
    /// it names does not exist until it lands — and a creation is never extended
    /// by what the pointer does next, because it happened once. So what was two
    /// booleans and an `Option` beside each other, most of whose combinations
    /// mean nothing, is three states and a flag inside the one arm that reads it.
    pub(crate) fn about(self) -> About {
        // A step a gesture is still writing, so a history extends the one it is
        // recording rather than starting another. A drag arrives a frame at a
        // time and is one thing the user did, so sixty of them are one step
        // back; scrubbing a dimension is the same shape by the same argument —
        // the pointer travels, the number follows it, and what the user did was
        // set a value once. Both are closed by a [`Step::Release`], which the
        // widget driving them raises when its gesture ends.
        let gesture = |at| About::Rewrites {
            at,
            coalesces: true,
        };
        // A step written in one go: a click that puts geometry down, a relation
        // stated, a command run. Whatever the pointer does next is a different
        // thing done.
        let once = |at| About::Rewrites {
            at,
            coalesces: false,
        };
        match self {
            Change::Drag { sketch, .. }
            | Change::Place { sketch, .. }
            | Change::Resize { sketch, .. } => gesture(sketch),
            Change::MovePlane { plane, .. } => gesture(plane),
            Change::Carry { extrude, .. } => gesture(extrude),
            Change::AddPoint { sketch, .. }
            | Change::AddSegment { sketch, .. }
            | Change::AddCircle { sketch, .. }
            | Change::Constrain { sketch, .. }
            | Change::Tidy { sketch }
            | Change::Delete { sketch, .. } => once(sketch),
            // The two that make a step. What each names is what the new step is
            // built *on* rather than a step this is about: the one it makes does
            // not exist until it lands, so there is nothing here for a history
            // to record a *before* against.
            Change::Extrude { .. } | Change::AddSketch { .. } => About::Makes,
            // And the one that takes steps away, which has no *after* for the
            // same reason a creation has no before.
            Change::DeleteStep { .. } => About::Removes,
            // And the one that moves a step without changing what any step says.
            // Its own arm rather than a rewrite of the step it names, because a
            // rewrite is recorded by comparing what a step held on either side
            // and this leaves every one of them holding exactly what it did.
            Change::Reorder { step, .. } => About::Moves { at: step },
            // The camera is not the drawing. Turning it names no step, so there
            // is nothing to take back and nothing to solve again.
            Change::Orbit { .. }
            | Change::Dolly { .. }
            | Change::Pan { .. }
            | Change::Project(_) => About::Nothing,
        }
    }
}

/// What a [`Change`] does to the timeline.
///
/// The one question the history and the document both ask of a change before
/// they do anything with it — see [`Change::about`], which is where every
/// variant answers it and the only place any of them does.
///
/// Three states rather than a pair of flags, because that is how many there
/// are: a change puts a step on the end, rewrites one that is there, or is
/// about no step at all.
#[derive(Debug, Clone, Copy)]
pub(crate) enum About {
    /// It adds a step to the timeline.
    ///
    /// Which step is not said, because it does not exist yet: what it comes out
    /// as is [`Document::apply`](crate::document::Document)'s answer, and
    /// recording it is [`Edit::Added`](crate::history::Edit)'s shape — a step
    /// that was not there has no *before* to put back, only its absence.
    Makes,
    /// It takes steps out of the timeline.
    ///
    /// *Steps*, plural, and that is the arm rather than a detail of it: what a
    /// delete really takes is the step named and everything built on it, and
    /// putting them back is putting each where it sat. Which ones is not said
    /// here for the reason the arm above says nothing either — it is the
    /// document's answer, and [`Edit::Removed`](crate::history::Edit) is its
    /// shape.
    ///
    /// Its own arm and not [`About::Makes`] read backwards, because the history
    /// does opposite things with them and the match is what makes a third kind
    /// of structural change a compile error rather than a step nothing can take
    /// back.
    Removes,
    /// It moves the step at `at` without changing what any step says.
    ///
    /// The one structural change with nothing to record on either side but a
    /// *place*: no step is made, none is taken away, and every one of them holds
    /// afterwards exactly what it held before. Which is why it cannot be a
    /// [`About::Rewrites`] — that arm decides whether to record anything by
    /// comparing the step's value at either end, and here they are equal.
    Moves { at: FeatureId },
    /// It rewrites the step at `at`, which the timeline already holds.
    Rewrites {
        at: FeatureId,
        /// Whether it belongs to a gesture already under way, so the history
        /// extends the step it is recording rather than starting another.
        ///
        /// Inside this arm because it is only ever read here: a creation is
        /// never extended — it happened once, and whatever the pointer does next
        /// is a different thing done — and a change about no step is not
        /// recorded at all.
        coalesces: bool,
    },
    /// It is about no step of the timeline: the camera, which is not the
    /// drawing.
    Nothing,
}
