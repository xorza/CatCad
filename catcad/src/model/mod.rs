//! The drawing as it currently stands: what is written down, and what the last
//! solve made of it.

pub(crate) mod faults;
pub(crate) mod models;
pub(crate) mod sheeted;

use glam::Vec3;
use silverpoint::{Arrangement, Constraint, Entity, Outcome, Plane, Sketch};

use crate::build::settled::Settled;
use crate::drawing::Drawing;
use crate::drawing::measurable::Measurable;
use crate::part::Part;
use crate::profile::Profile;
use crate::timeline::FeatureId;

/// A sketch and what the last solve made of it, read together.
///
/// Nothing new — every field is something the application already owns. What it
/// is for is that they are never apart: what a drawing *says* and what the last
/// solve *made* of that are two readings of one moment, and a caller handed one
/// without the other could answer out of a mix of two frames. So everything
/// that reads the model reads it through here, and they travel as one argument
/// rather than as three.
///
/// Which is also why the build is taken whole and read here rather than picked
/// apart by the caller: a settling and a revision that came from two different
/// builds would be the very mix this exists to refuse.
///
/// The drawing rather than the whole document, deliberately. What paints a
/// drawing has no business with the camera looking at it or the solids standing
/// beside it — those belong to whoever is laying out a *scene*, and are asked of
/// the document directly by the two calls that want them.
///
/// One sketch rather than all of them, likewise. A document that holds several
/// hands out one of these apiece, and what draws them draws each in turn — so
/// nothing below has to say *which* sketch it means.
///
/// Borrowed and [`Copy`], so passing one down a stack costs what passing a
/// reference costs. A caller that wants to *write* takes the halves separately,
/// because writing them is exactly what has to happen in an order — see
/// [`Build`](crate::build::Build).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Model<'a> {
    /// Which sketch of the timeline this is, which is half of what names
    /// anything picked out of it — see [`Part`].
    of: FeatureId,
    /// Whether this is the sketch being edited.
    ///
    /// Not a fact about the document — which sketch you have open is the
    /// session's, and saving writes none of it. It is here because a model is a
    /// *reading* of a sketch rather than the sketch itself, and how a sketch
    /// stands to a reader includes whether it is the one being worked in: what
    /// draws one draws it in the colours of what it has left to decide, and
    /// what draws the rest draws them as ground.
    live: bool,
    drawing: Drawing<'a>,
    settled: &'a Settled,
}

impl<'a> Model<'a> {
    /// The sketch and the plane it lies on.
    pub(crate) fn drawing(self) -> Drawing<'a> {
        self.drawing
    }

    /// The geometry and the constraints over it.
    pub(crate) fn sketch(self) -> &'a Sketch {
        self.drawing.sketch()
    }

    /// Where the drawing lies in the world.
    pub(crate) fn plane(self) -> Plane {
        self.drawing.plane()
    }

    /// How the last run went, and what the constraints have decided.
    pub(crate) fn outcome(self) -> &'a Outcome {
        self.settled.outcome()
    }

    /// What the drawing's curves shut in.
    pub(crate) fn arrangement(self) -> &'a Arrangement {
        self.settled.arrangement()
    }

    /// Which sketch of the timeline this is.
    pub(crate) fn of(self) -> FeatureId {
        self.of
    }

    /// Whether this is the sketch being edited.
    pub(crate) fn live(self) -> bool {
        self.live
    }

    /// One of this sketch's entities, as something that can be picked out.
    ///
    /// Here rather than on [`Part`] itself, because the sketch half of the name
    /// is the one thing an entity handle cannot supply: a caller holding both
    /// is holding a model, and one that is not has no business minting a name.
    pub(crate) fn part(self, entity: impl Into<Entity>) -> Part {
        Part::Entity {
            sketch: self.of,
            entity: entity.into(),
        }
    }

    /// The region at `at` in what this sketch's curves enclose, likewise.
    pub(crate) fn region(self, at: usize) -> Part {
        Part::Region {
            sketch: self.of,
            at,
        }
    }

    /// The same region as something a feature can be built on.
    ///
    /// The one place faces become a [`Profile`], which is the moment
    /// positions among this frame's faces turn into names meant to outlive
    /// every edit that follows — see [`Profile`], on why the two are different
    /// types rather than one.
    ///
    /// Here beside [`Model::region`] for the reason that one is here: the
    /// sketch half of the name is what a position among the faces cannot
    /// supply, and a caller holding both is holding a model.
    pub(crate) fn profile(self, at: &[usize]) -> Profile {
        let faces = self.arrangement().faces();
        Profile::of(self.of, at.iter().map(|&at| faces[at].named()))
    }

    /// Where a circle's rim runs in the world, as points around it.
    ///
    /// What a form standing beside a circle is placed against — and it is asked
    /// for a middle and a radius rather than a handle because the circle is
    /// still being *drawn*: there is nothing for the sketch to hold yet, only a
    /// centre already clicked and however far the pointer has carried the band.
    /// A radius of nothing collapses to the centre, which is all of the circle
    /// there is before the pointer has moved.
    ///
    /// Points around the rim rather than the middle and radius handed back for
    /// the caller to square off, because what a placement wants is a *box on
    /// screen* and a circle seen at an angle is an ellipse — squaring off the
    /// pair would give the box it would have had face-on.
    ///
    /// Eight, which is enough for a box: the widest a regular polygon's own box
    /// falls short of its circle's is at the halfway points between corners, and
    /// at eight that is under 4% — smaller than the gap a form is placed with.
    pub(crate) fn rim_around(self, middle: Vec3, radius: f32) -> impl Iterator<Item = Vec3> {
        const AROUND: usize = 8;
        let plane = self.plane();
        let (across, up) = (plane.x.as_vec3(), plane.y.as_vec3());
        (0..AROUND).map(move |step| {
            let angle = step as f32 / AROUND as f32 * std::f32::consts::TAU;
            middle + (across * angle.cos() + up * angle.sin()) * radius
        })
    }

    /// The entity `part` names, or `None` where it names a region or belongs
    /// to another sketch.
    ///
    /// The sketch half of the check is the one a handle cannot make for itself:
    /// two sketches are two arenas and mint the same handles, so a part of
    /// another would resolve here as whatever happens to sit at that slot. What
    /// asks this is anything that would go on to *use* the handle.
    pub(crate) fn entity(self, part: Part) -> Option<Entity> {
        (part.sketch() == Some(self.of))
            .then(|| part.entity())
            .flatten()
    }

    /// Every constraint `picked` admits, written into `into`.
    ///
    /// What the bar offers. Order matters where the constraint is not
    /// symmetric, and the selection keeps the order things were picked in for
    /// exactly this.
    ///
    /// Two halves, and the split is which of them the bar decides alone: a
    /// relation is offered here and nowhere else, where a *dimension* is also
    /// what the dimension tool places — so which dimension a selection admits is
    /// [`Measurable`]'s to say, and both sides read it. See
    /// [`Model::relations`] and [`Model::dimensions`].
    ///
    /// A constraint carrying a number takes the one the drawing already has, so
    /// asking for a distance *locks* what is there rather than demanding a value
    /// the user has no way to type yet. That is also what a modeller does: the
    /// dimension appears reading what it measured, and is retyped afterwards.
    /// Fitting it is [`Sketch::fitted`]'s, which also drops a dimension that
    /// would measure nothing — see the note there.
    ///
    /// Fills rather than returns, because the bar asks this every frame and the
    /// record pass allocates nothing.
    pub(crate) fn offers(self, picked: &[Part], into: &mut Vec<Constraint>) {
        into.clear();
        // Entities of *this* sketch only. A region is what the curves enclose
        // rather than something a sketch holds, so there is nothing to state a
        // relation about — and neither is a part of another sketch, which is a
        // different system entirely. A pair with either in it admits nothing at
        // all, rather than admitting whatever the other half would on its own.
        let named = match *picked {
            [one, two] => self
                .entity(one)
                .zip(self.entity(two))
                .map(|(one, two)| (one, Some(two))),
            // A relation needs two things to hold between, so a single pick
            // admits nothing but what that one thing measures about itself.
            [only] => self.entity(only).map(|one| (one, None)),
            _ => None,
        };
        let Some((one, two)) = named else {
            return;
        };
        // What the pair *is* before what it measures: a relation says something
        // that holds without a number, and a dimension is the number. Stated
        // here rather than inside either, so the bar's order is one line.
        if let Some(two) = two {
            self.relations(one, two, into);
        }
        self.dimensions(one, two, into);
    }

    /// Every dimension the selection admits, in the order they are offered.
    ///
    /// Read off [`Measurable`], which is the one table of which dimension goes
    /// with which selection — the dimension tool places what this offers, and a
    /// table apiece is a table that can drift. What is decided *here* is only
    /// that the bar offers every reading a selection leaves open, since a
    /// selection has no pointer to say which of them was meant.
    fn dimensions(self, one: Entity, two: Option<Entity>, into: &mut Vec<Constraint>) {
        let Some(measurable) = Measurable::of(self.sketch(), one, two) else {
            return;
        };
        self.admits(
            measurable
                .readings()
                .iter()
                .map(|&along| measurable.stated(along)),
            into,
        );
    }

    /// Whatever of `candidates` the drawing can actually state, appended to
    /// `into`.
    ///
    /// Every offer goes through here, which is what makes "a dimension holds
    /// what the drawing measures" one rule rather than one per row of the two
    /// tables above: a candidate is written with the geometry it is about and a
    /// placeholder number, and the sketch fills the number in — or refuses the
    /// candidate outright where there is nothing to measure. A relation has no
    /// number and passes straight through.
    fn admits(self, candidates: impl IntoIterator<Item = Constraint>, into: &mut Vec<Constraint>) {
        let sketch = self.sketch();
        into.extend(
            candidates
                .into_iter()
                .filter_map(|candidate| sketch.fitted(candidate)),
        );
    }

    /// What a pair of entities states about each other, in the order they were
    /// picked.
    ///
    /// The relations alone — everything here holds without saying how much.
    /// What a pair can be given a *number* for is [`Model::dimensions`] beside
    /// it, and the split is what keeps the bar and the dimension tool reading
    /// one table: a relation is the bar's alone, where a dimension is placed by
    /// a tool as well as offered here.
    ///
    /// Order matters only where the relation is not symmetric, and none of
    /// these is: every pair below reads the same whichever way round it was
    /// reached, which is why each mixed one is matched both ways.
    fn relations(self, one: Entity, two: Entity, into: &mut Vec<Constraint>) {
        match (one, two) {
            (Entity::Point(a), Entity::Point(b)) => self.admits(
                [
                    Constraint::Coincident { a, b },
                    Constraint::Horizontal { a, b },
                    Constraint::Vertical { a, b },
                ],
                into,
            ),
            (Entity::Segment(first), Entity::Segment(second)) => self.admits(
                [
                    Constraint::Parallel { first, second },
                    Constraint::Perpendicular { first, second },
                    Constraint::EqualLength { first, second },
                ],
                into,
            ),
            (Entity::Point(point), Entity::Segment(segment))
            | (Entity::Segment(segment), Entity::Point(point)) => {
                self.admits([Constraint::PointOnSegment { point, segment }], into);
            }
            (Entity::Point(point), Entity::Circle(circle))
            | (Entity::Circle(circle), Entity::Point(point)) => {
                self.admits([Constraint::PointOnCircle { point, circle }], into);
            }
            (Entity::Circle(first), Entity::Circle(second)) => {
                self.admits([Constraint::EqualRadius { first, second }], into);
            }
            (Entity::Segment(segment), Entity::Circle(circle))
            | (Entity::Circle(circle), Entity::Segment(segment)) => {
                self.admits([Constraint::Tangent { segment, circle }], into);
            }
            _ => {}
        }
    }

    /// Whether `part` is still there to be picked out.
    ///
    /// The two halves of what a part can be, answered by the two halves of the
    /// model: an entity by the drawing that holds it, and a region by there
    /// still being that many. Here rather than on either half, because neither can
    /// answer the whole question — which is the same reason they are borrowed
    /// together at all.
    ///
    /// **For this sketch only.** A part of another one is not this model's to
    /// answer for and comes back `false`, so a caller with several models asks
    /// each of them and takes any yes.
    pub(crate) fn holds(self, part: Part) -> bool {
        match part {
            Part::Entity { sketch, entity } => sketch == self.of && self.drawing.holds(entity),
            Part::Region { sketch, at } => {
                sketch == self.of && at < self.arrangement().faces().len()
            }
            // None of the three is a sketch's to answer for: one is what a
            // sketch is drawn on, one is a step of its own, and the last is not
            // in the document at all — it is a form's own reading, and what
            // keeps it from outliving the form is the form closing. See
            // [`Models::holds`], which puts the question to whatever can.
            Part::Step(_) | Part::Solid { .. } | Part::Growing | Part::Turning => false,
        }
    }
}
