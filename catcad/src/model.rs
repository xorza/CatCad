//! The drawing as it currently stands: what is written down, and what the last
//! solve made of it.

use glam::Vec3;
use silverpoint::{
    Along, Arrangement, CircleId, Constraint, Dimension, Entity, Outcome, Plane, Prism, Sketch,
};

use crate::build::settled::Settled;
use crate::build::{Build, Revision};
use crate::drawing::Drawing;
use crate::part::Part;
use crate::profile::Profile;
use crate::timeline::{FeatureId, Timeline};

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
/// [`Build`].
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
    /// The one place a face becomes a [`Profile`], which is the moment a
    /// position among this frame's faces turns into a name meant to outlive
    /// every edit that follows — see [`Profile`], on why the two are different
    /// types rather than one.
    ///
    /// Here beside [`Model::region`] for the reason that one is here: the
    /// sketch half of the name is what a position among the faces cannot
    /// supply, and a caller holding both is holding a model.
    pub(crate) fn profile(self, at: usize) -> Profile {
        let arrangement = self.arrangement();
        let mut bounds = Vec::new();
        arrangement.bounds(&arrangement.faces()[at], &mut bounds);
        Profile::new(self.of, bounds)
    }

    /// Where the rim of `circle` runs in the world, as points around it.
    ///
    /// What a form standing beside a circle is placed against. The rim rather
    /// than the middle and a radius, because what a placement wants is a *box*
    /// on screen and a circle seen at an angle is an ellipse — the middle plus
    /// a radius either way would be the box it would have had face-on.
    ///
    /// Eight, which is enough for a box: the widest a regular polygon's own box
    /// falls short of its circle's is at the halfway points between corners, and
    /// at eight that is under 4% — smaller than the gap a form is placed with.
    pub(crate) fn rim_of(self, circle: CircleId) -> impl Iterator<Item = Vec3> {
        let held = self.sketch().circle(circle);
        let middle = self
            .plane()
            .point(self.sketch().point(held.center).position)
            .as_vec3();
        self.rim_around(middle, held.radius.abs() as f32)
    }

    /// The same, about a middle and a radius rather than a circle the sketch
    /// holds.
    ///
    /// What a circle still being *drawn* is placed against: there is no handle
    /// to ask, only a centre already clicked and however far the pointer has
    /// carried the band. A radius of nothing collapses to the centre, which is
    /// all of the circle there is before the pointer has moved.
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
    /// What the bar offers, and so the one statement of which selections mean
    /// what. Order matters where the constraint is not symmetric, and the
    /// selection keeps the order things were picked in for exactly this.
    ///
    /// A constraint carrying a number takes the one the drawing already has, so
    /// asking for a distance *locks* what is there rather than demanding a value
    /// the user has no way to type yet. That is also what a modeller does: the
    /// dimension appears reading what it measured, and is retyped afterwards.
    ///
    /// Fills rather than returns, because the bar asks this every frame and the
    /// record pass allocates nothing.
    pub(crate) fn offers(self, picked: &[Part], into: &mut Vec<Constraint>) {
        into.clear();
        match *picked {
            // Entities of *this* sketch only. A region is what the curves
            // enclose rather than something a sketch holds, so there is nothing
            // to state a relation about — and neither is a part of another
            // sketch, which is a different system entirely. A pair with either
            // in it admits nothing at all, rather than admitting whatever the
            // other half would on its own.
            [one, two] => {
                if let (Some(one), Some(two)) = (self.entity(one), self.entity(two)) {
                    self.between(one, two, into);
                }
            }
            // The one relation a single pick admits: a radius takes the size
            // the circle already is, so asking for one locks what is there
            // rather than demanding a number nobody can type yet.
            [only] => {
                if let Some(Entity::Circle(circle)) = self.entity(only) {
                    into.push(Constraint::Radius {
                        circle,
                        dimension: Dimension::new(self.sketch().circle(circle).radius),
                    });
                }
            }
            _ => {}
        }
    }

    /// What a pair of entities admits, in the order they were picked.
    ///
    /// Order matters only where the relation is not symmetric, and none of
    /// these is: every pair below reads the same whichever way round it was
    /// reached, which is why each mixed one is matched both ways.
    fn between(self, one: Entity, two: Entity, into: &mut Vec<Constraint>) {
        match (one, two) {
            (Entity::Point(a), Entity::Point(b)) => into.extend([
                Constraint::Coincident { a, b },
                Constraint::Distance {
                    a,
                    b,
                    along: Along::Shortest,
                    dimension: Dimension::new(
                        (self.sketch().point(a).position - self.sketch().point(b).position)
                            .length(),
                    ),
                },
                Constraint::Horizontal { a, b },
                Constraint::Vertical { a, b },
            ]),
            (Entity::Segment(first), Entity::Segment(second)) => into.extend([
                Constraint::Parallel { first, second },
                Constraint::Perpendicular { first, second },
                Constraint::EqualLength { first, second },
            ]),
            (Entity::Point(point), Entity::Segment(segment))
            | (Entity::Segment(segment), Entity::Point(point)) => {
                into.push(Constraint::PointOnSegment { point, segment });
            }
            (Entity::Point(point), Entity::Circle(circle))
            | (Entity::Circle(circle), Entity::Point(point)) => {
                into.push(Constraint::PointOnCircle { point, circle });
            }
            (Entity::Circle(first), Entity::Circle(second)) => {
                into.push(Constraint::EqualRadius { first, second });
            }
            (Entity::Segment(segment), Entity::Circle(circle))
            | (Entity::Circle(circle), Entity::Segment(segment)) => {
                into.push(Constraint::Tangent { segment, circle });
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
            Part::Plane(_) | Part::Solid { .. } | Part::Growing => false,
        }
    }
}

/// Every sketch a document holds, as it currently stands.
///
/// The plural of [`Model`], and what anything drawing or pruning a *document*
/// reads: a picture is made of every sketch it holds, not of the one you happen
/// to be in. Which one that is comes along, because it is what tells a model
/// apart from its neighbours to a reader.
///
/// Borrowed and [`Copy`] like the models it hands out, and for the same reason
/// — but it hands them out by walking rather than by holding a list, so a
/// caller that wants them five times over asks five times. Each walk is over
/// the timeline's own steps and costs what that costs; holding the models
/// instead would mean a buffer of borrows, which is a thing no struct can keep.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Models<'a> {
    timeline: &'a Timeline,
    build: &'a Build,
    editing: FeatureId,
}

impl<'a> Models<'a> {
    /// Every sketch `timeline` holds as `build` last left it, with `editing`
    /// the one being worked in.
    ///
    /// The timeline rather than the document, because that is where the
    /// sketches are: a picture of them wants nothing the document adds — not
    /// the camera looking at them, and not the solids standing beside them.
    pub(crate) fn new(timeline: &'a Timeline, build: &'a Build, editing: FeatureId) -> Self {
        Self {
            timeline,
            build,
            editing,
        }
    }

    /// Each of them, in the order the timeline holds them.
    pub(crate) fn iter(self) -> impl Iterator<Item = Model<'a>> {
        let Self {
            timeline,
            build,
            editing,
        } = self;
        timeline.sketches().map(move |at| Model {
            of: at,
            live: at == editing,
            drawing: timeline.drawing(at),
            settled: build.settled(at),
        })
    }

    /// The one being edited.
    ///
    /// Found among the rest rather than built apart from them, so there is one
    /// place a model is made and no second one to drift: whether a model is
    /// live is decided by the same line for all of them.
    /// The reading of the sketch at `at`, or `None` where the timeline holds no
    /// sketch there.
    ///
    /// Beside [`Models::open`] rather than folded into it: that one answers for
    /// the sketch being *worked in*, which is a fact about the session, and this
    /// answers for one named outright — a region a form is deciding over names
    /// the sketch it came from, whichever is open.
    pub(crate) fn at(self, sketch: FeatureId) -> Option<Model<'a>> {
        self.iter().find(|model| model.of() == sketch)
    }

    pub(crate) fn open(self) -> Model<'a> {
        self.iter()
            .find(|model| model.of() == self.editing)
            .expect("the sketch being edited is one the document holds")
    }

    /// Every plane that can be moved, with where it lies.
    ///
    /// What is *drawn* as a datum — see
    /// [`Timeline::movable_planes`](crate::timeline::Timeline::movable_planes).
    pub(crate) fn planes(self) -> impl Iterator<Item = (FeatureId, Plane)> {
        let timeline = self.timeline;
        timeline
            .movable_planes()
            .map(move |at| (at, timeline.plane(at)))
    }

    /// How many extrudes no longer know which region they are grown from.
    ///
    /// What a drawing can do to a feature standing downstream of it: a line
    /// drawn across a region takes away the thing an extrude was built on, and
    /// neither of the two regions that replaced it is what the name meant. Said
    /// as a count because that is what a reader can act on — which extrude went
    /// wrong is a question for the timeline, which nothing shows yet.
    ///
    /// Here rather than on the build, though the build could count these on its
    /// own — every [`Modelled`](crate::build::modelled::Modelled) carries both
    /// the step it answers for and its answer. What this is instead is the first
    /// of the readings that join the two, and the shape the rest will take: what
    /// *draws* a solid wants the region, the plane its sketch is on, and how far
    /// it stands off, and only a pairing of the timeline and the build has all
    /// three. A count that went round the back would be the one reader that did
    /// not.
    pub(crate) fn lost(self) -> usize {
        self.timeline
            .extrudes()
            .filter(|step| self.build.modelled(step.at).is_none())
            .count()
    }

    /// Which version of the document these describe.
    pub(crate) fn revision(self) -> Revision {
        self.build.revision()
    }

    /// Which sketch is being edited.
    pub(crate) fn editing(self) -> FeatureId {
        self.editing
    }

    /// Whether the document still holds `part`.
    ///
    /// Asked of every sketch rather than of the open one: what is picked out
    /// may span them, and a part of one nobody is in is still there to be
    /// picked. A plane belongs to no sketch, so it is the timeline that
    /// answers for one.
    pub(crate) fn holds(self, part: Part) -> bool {
        match part {
            Part::Plane(at) => self.timeline.holds(at),
            // A face of a solid outlives an edit exactly while the solid still
            // knows which region it is grown from *and* that face is still one
            // of its own — a wall goes when the curve it was swept from stops
            // bounding the region, which is a thing drawing across a sketch can
            // do without taking the solid away.
            Part::Solid { of, face } => self
                .solids()
                .any(|(at, prism)| at == of && prism.holds(face)),
            _ => self.iter().any(|model| model.holds(part)),
        }
    }

    /// Every solid the document holds, as it currently stands.
    ///
    /// One per extrude whose profile still names a region — an extrude that has
    /// lost its footing is not half a solid, it is no solid, and what says so is
    /// its absence here. [`Models::lost`] is what counts those.
    ///
    /// A [`Prism`] is a reading rather than a thing the document holds: it pairs
    /// what the timeline says with where the last solve put it, which is the
    /// same pairing every other reading here is.
    pub(crate) fn solids(self) -> impl Iterator<Item = (FeatureId, Prism<'a>)> {
        let Self {
            timeline, build, ..
        } = self;
        timeline.extrudes().filter_map(move |step| {
            let region = build.modelled(step.at)?;
            let of = step.profile.sketch();
            let arrangement = build.settled(of).arrangement();
            let prism = Prism::new(arrangement, region, timeline.plane_of(of), step.distance);
            Some((step.at, prism))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Timeline;
    use crate::timeline::feature::{Datum, Feature};
    use glam::DVec2;
    use silverpoint::Sketch;

    /// Two sketches mint the same handles, and a part is what tells them apart.
    ///
    /// The arenas are per sketch, so the first point of one and the first point
    /// of the other are the *same* [`Entity`] — identical bits, different
    /// geometry, and nothing in the handle that could say which arena it came
    /// from. [`Id`](silverpoint::Id)'s own doc says as much. So a name carrying
    /// only the entity would make the two one thing, and a click on either
    /// would light both.
    #[test]
    fn two_sketches_mint_the_same_handles_and_are_told_apart_by_the_part() {
        let mut here = Sketch::default();
        let one = here.add_point(DVec2::ZERO);
        let mut there = Sketch::default();
        let other = there.add_point(DVec2::new(9.0, 9.0));
        assert_eq!(one, other, "two fresh arenas stopped agreeing on a handle");

        let mut timeline = Timeline::default();
        let ground = timeline.add(Feature::Plane(Datum::Ground));
        let first = timeline.add(Feature::Sketch {
            on: ground,
            sketch: here,
        });
        let second = timeline.add(Feature::Sketch {
            on: ground,
            sketch: there,
        });

        let mut build = Build::default();
        timeline.edit(first).opened(&mut build);
        timeline.edit(second).opened(&mut build);
        // Each through its own `Models`, because that is the only way to one:
        // whether a model is the live one is not a caller's to assert.
        let a = Models::new(&timeline, &build, first).open();
        let b = Models::new(&timeline, &build, second).open();

        // The same entity, named twice, comes out as two different parts.
        assert_ne!(a.part(one), b.part(other), "one name for two points");

        // And each model answers for its own and refuses the other's, which is
        // what stops a prune over one sketch from dropping another's selection.
        assert!(a.holds(a.part(one)) && b.holds(b.part(other)));
        assert!(
            !a.holds(b.part(other)),
            "a model answered for another sketch"
        );
        assert!(!b.holds(a.part(one)));

        // And nothing can be stated across the two. The handles are the same,
        // so a pair read without their sketches would resolve entirely inside
        // whichever model was asked — and would state a relation about geometry
        // nobody picked.
        let mut offers = Vec::new();
        a.offers(&[a.part(one), b.part(other)], &mut offers);
        assert!(offers.is_empty(), "a relation was offered across sketches");
        b.offers(&[a.part(one), b.part(other)], &mut offers);
        assert!(offers.is_empty(), "a relation was offered across sketches");

        // The same pair within one sketch does admit something, which is what
        // says the refusal above is about the sketches and not the shape of the
        // selection.
        let also = a.drawing().sketch().points().last().expect("a point").0;
        a.offers(&[a.part(one), a.part(also)], &mut offers);
        assert!(
            !offers.is_empty(),
            "a pair of one sketch's points admitted nothing"
        );
    }
}
