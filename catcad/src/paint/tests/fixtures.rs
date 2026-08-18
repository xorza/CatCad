//! The drawings every test below is painted from, and how a scene is read
//! back.

use crate::build::Build;
use crate::model::Models;
use crate::part::Part;
use crate::timeline::Timeline;
use aperture::Scene;
use glam::{DVec2, Vec3};
use silverpoint::{Entity, Sketch};
use std::collections::HashSet;
use std::mem::{Discriminant, discriminant};

/// A drawing and what solving it decided — the pair every writer takes.
///
/// Lent to the writers' own tests beside these, which is the whole of what a
/// fixture is for: what they and the calls here need is one solved sketch, and
/// two spellings of raising one would be two fixtures free to drift.
#[derive(Debug)]
pub(crate) struct Drawn {
    pub(crate) timeline: Timeline,
    pub(crate) build: Build,
}

impl Drawn {
    /// Every sketch it holds, which for a fixture of one is that one — open,
    /// so it is drawn in the colours of what it has left to decide.
    pub(crate) fn models(&self) -> Models<'_> {
        Models::new(&self.timeline, &self.build, self.timeline.first_sketch())
    }
}

/// The drawing the writers take: `sketch` on the ground, solved.
///
/// Solved because determinacy is measured where the geometry stands, and an
/// unsolved guess is not where it will stand — which is the drawing's own job
/// to arrange, so this asks for one rather than assembling the parts.
pub(crate) fn drawn(sketch: Sketch) -> Drawn {
    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    timeline.edit(timeline.first_sketch()).opened(&mut build);
    Drawn { timeline, build }
}

/// One of every relation the drawing can state, so a sweep over them is a sweep
/// over the enum.
///
/// Named for what it gathers rather than for relations alone, because it
/// gathers dimensions too — and the two want different things of a caller: a
/// relation is drawn as a symbol and a dimension as a number, so a sweep over
/// marks has to sift them and a sweep over the enum must not.
pub(super) fn every_statable() -> Vec<silverpoint::Constraint> {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let c = sketch.add_point(DVec2::new(6.0, 0.0));
    let first = sketch.add_segment(a, b);
    let second = sketch.add_segment(b, c);
    // A third edge running with the first, because a distance between two edges
    // is offered only where they are already parallel — so a pair that crosses
    // could never reach `Spacing`, and the sweep below would be short of the one
    // variant nothing else states.
    let (aside, along) = (
        sketch.add_point(DVec2::new(1.0, 0.0)),
        sketch.add_point(DVec2::new(4.0, 4.0)),
    );
    let alongside = sketch.add_segment(aside, along);
    let circle = sketch.add_circle(c, 2.0);
    let other = sketch.add_circle(a, 1.0);
    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    let at = timeline.first_sketch();
    timeline.edit(at).opened(&mut build);
    let model = Models::new(&timeline, &build, at).open();

    let mut every = Vec::new();
    let mut offers = Vec::new();
    for picked in [
        vec![Entity::Point(a), Entity::Point(b)],
        vec![Entity::Segment(first), Entity::Segment(second)],
        vec![Entity::Segment(first), Entity::Segment(alongside)],
        vec![Entity::Point(a), Entity::Segment(second)],
        vec![Entity::Point(a), Entity::Circle(circle)],
        vec![Entity::Circle(circle)],
        vec![Entity::Segment(first), Entity::Circle(circle)],
        vec![Entity::Circle(circle), Entity::Circle(other)],
    ] {
        let picked: Vec<Part> = picked
            .into_iter()
            .map(|entity| model.part(entity))
            .collect();
        model.offers(&picked, &mut offers);
        every.extend(offers.iter().copied());
    }
    // Every variant of the enum, which is what makes a sweep over these a sweep
    // over it: one `offers` cannot reach is one nothing can state, which is its
    // own bug.
    //
    // By discriminant rather than by count, and that is what the three readings
    // of a distance forced. A count used to be readable against the enum — one
    // offer, one variant — and now several offers share a variant, so a total
    // would be a number nobody could check against anything.
    let kinds: HashSet<Discriminant<silverpoint::Constraint>> =
        every.iter().map(discriminant).collect();
    assert_eq!(kinds.len(), 14, "{every:?}");
    every
}

/// A colour no writer produces, so a primitive still wearing it is one a redraw
/// left alone.
///
/// What the stage ladder claims cannot be seen any other way. A batch rewritten
/// with the contents it already had is indistinguishable from one that was never
/// touched, and the whole point of the stages is the work *not* done — so every
/// primitive is stamped with something no drawing could arrive at, and what
/// still carries it afterwards is what was skipped.
pub(super) const UNWRITTEN: Vec3 = Vec3::splat(-1.0);

/// Stamp every batch, so the next redraw can be asked which of them it wrote
/// over.
pub(super) fn stamp(scene: &mut Scene) {
    for curve in scene.curves.iter_mut() {
        curve.color = UNWRITTEN;
    }
    for ring in scene.rings.iter_mut() {
        ring.color = UNWRITTEN;
    }
    for point in scene.points.iter_mut() {
        point.color = UNWRITTEN;
    }
    for text in scene.texts.iter_mut() {
        text.color = UNWRITTEN;
    }
    for face in scene.faces.iter_mut() {
        face.color = UNWRITTEN;
    }
    for solid in scene.solids.iter_mut() {
        solid.color = UNWRITTEN;
    }
}

/// Which batches still carry the stamp, which is which of them a redraw left
/// alone.
///
/// Named rather than counted, so a failure says which writer ran when it should
/// not have. Every batch of the fixture is checked to be non-empty first — an
/// empty one would report itself skipped whatever happened to it.
pub(super) fn untouched(scene: &Scene) -> Vec<&'static str> {
    let held = |name, kept: bool| kept.then_some(name);
    [
        held(
            "curves",
            scene.curves.iter().all(|it| it.color == UNWRITTEN),
        ),
        held("rings", scene.rings.iter().all(|it| it.color == UNWRITTEN)),
        held(
            "points",
            scene.points.iter().all(|it| it.color == UNWRITTEN),
        ),
        held("texts", scene.texts.iter().all(|it| it.color == UNWRITTEN)),
        held("faces", scene.faces.iter().all(|it| it.color == UNWRITTEN)),
        held(
            "solids",
            scene.solids.iter().all(|it| it.color == UNWRITTEN),
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}
