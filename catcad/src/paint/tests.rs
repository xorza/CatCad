use super::*;
use crate::build::Build;
use crate::demo;
use crate::document::Document;
use crate::intent::Change;
use crate::model::Models;
use crate::paint::growing::Growing;
use crate::part::Part;
use crate::preview::{Ends, Preview};
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature};
use aperture::{Facing, Scene, Tag, Turn};
use glam::{DVec2, Vec2, Vec3};
use silverpoint::{Along, Dimension, Entity, Sketch};
use std::collections::HashSet;
use std::mem::{Discriminant, discriminant};

/// A drawing and what solving it decided — the pair every writer takes.
///
/// Lent to the writers' own tests beside these, which is the whole of what a
/// fixture is for: what they and the calls here need is one solved sketch, and
/// two spellings of raising one would be two fixtures free to drift.
#[derive(Debug)]
pub(super) struct Drawn {
    timeline: Timeline,
    build: Build,
}

impl Drawn {
    /// Every sketch it holds, which for a fixture of one is that one — open,
    /// so it is drawn in the colours of what it has left to decide.
    pub(super) fn models(&self) -> Models<'_> {
        Models::new(&self.timeline, &self.build, self.timeline.first_sketch())
    }
}

/// The drawing the writers take: `sketch` on the ground, solved.
///
/// Solved because determinacy is measured where the geometry stands, and an
/// unsolved guess is not where it will stand — which is the drawing's own job
/// to arrange, so this asks for one rather than assembling the parts.
pub(super) fn drawn(sketch: Sketch) -> Drawn {
    let mut build = Build::default();
    let mut timeline = Timeline::of(sketch);
    timeline.edit(timeline.first_sketch()).opened(&mut build);
    Drawn { timeline, build }
}

/// The demo drawing lays out to exactly what it holds, and every part of it is
/// named.
///
/// The fixture the rest of the suite leans on, checked once as a whole: the
/// counts are what the demo's sketch contains, and the names are what makes any
/// of it pickable. A part drawn without a name would be scenery — visible, and
/// impossible to point at.
#[test]
fn the_demo_draws_every_part_it_holds_and_names_each_one() {
    let one = drawn(demo::sketch());
    let mut scene = Scene::default();
    let mut layout = Layout::default();
    redraw(one.models(), &mut layout, Showing::default(), &mut scene);

    // Seven segments — four sides, the rail, and the arm's two bars — two
    // circles, and a marker on each of the nine points.
    assert_eq!(scene.curves.len(), 7);
    assert_eq!(scene.rings.len(), 2);
    assert_eq!(scene.points.len(), 9);

    // Every drawn part is named, and named as something the drawing holds: the
    // tags the scene carries are indices into what came back.
    for tag in (scene.curves.iter().map(|curve| curve.tag))
        .chain(scene.rings.iter().map(|ring| ring.tag))
        .chain(scene.points.iter().map(|point| point.tag))
    {
        let tag = tag.expect("a part of the drawing is drawn to be picked");
        assert!(layout.names().get(tag).is_some(), "{tag:?} names nothing");
    }

    // Written again into the same scene, it says the same thing rather than
    // adding to it. Through a layout that has drawn nothing, because one that
    // has already drawn this revision correctly declines to draw it twice —
    // and what is being checked here is the refill, not the skip.
    redraw(
        one.models(),
        &mut Layout::default(),
        Showing::default(),
        &mut scene,
    );
    assert_eq!(scene.curves.len(), 7);
    assert_eq!(scene.rings.len(), 2);
    assert_eq!(scene.points.len(), 9);
}

/// A scene is the document and nothing else.
///
/// What this pins is that the picture is *derived* — nothing stands in it that
/// the document does not hold, which is the whole reason saving the document is
/// enough. It used to have an exception: the solids arrived from beside the
/// document because no step could make one, and were what saving did not write.
/// A step makes one now, so the exception is gone and the claim is the whole
/// scene.
///
/// The overlay counts are the fixture above's, laid out from the same sketch by
/// the same writer, so what this adds is the solids and the fact that one call
/// produces every part of it.
#[test]
fn a_scene_is_made_of_the_document_and_nothing_else() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let picture = scene(
        document.models(&build, document.opening()),
        &mut Layout::default(),
    );

    // One object per face of the one solid the demo grows: the two ends and the
    // single wall swept off the hub's circle, which is the whole boundary of a
    // cylinder.
    assert_eq!(picture.solids.len(), 3);
    // And both sketches: the frame's seven edges and the triangle's three, two
    // rims, nine markers and three.
    assert_eq!(picture.curves.len(), 10);
    assert_eq!(picture.rings.len(), 2);
    assert_eq!(picture.points.len(), 12);
    // And no controls, which are not the document's: they are built against a
    // camera there is none of here, and `gizmos::write` is what writes them.
    assert!(picture.gizmos.is_empty());
}

/// Every symbol a mark is drawn as has a glyph in the faces the shaper falls
/// back through.
///
/// The failure this guards is silent and total: a symbol the fonts lack
/// rasterizes to nothing, so the relation is simply not drawn and the drawing
/// says a constraint is absent when it is not. Nothing else notices — the
/// records are built, the quads are laid out, and the sheet has no ink to give
/// them.
///
/// Every variant, driven off `offers` rather than a list written twice, so a
/// tenth relation is covered the moment the drawing can state it.
#[test]
fn every_mark_has_a_glyph_to_draw_it() {
    let shaper = palantir::TextShaper::new();
    let mut glyphs = shaper.glyphs();
    let mut placed = Vec::new();

    // The relations alone. A dimension is drawn as its number, so it never
    // reaches `symbol` and asking it for one panics — see the arm there.
    for constraint in every_statable()
        .into_iter()
        .filter(|constraint| constraint.value().is_none())
    {
        let mark = super::symbol(constraint);
        // The face and the size the drawing sets marks in, not a stand-in: a
        // symbol the mono bold face lacks falls through to whatever the system
        // offers, and one nothing offers draws blank.
        glyphs.line(mark, super::mark_font(), 1.0, &mut placed);
        let [glyph] = placed[..] else {
            panic!(
                "{mark:?} for {constraint:?} shaped to {} glyphs",
                placed.len()
            );
        };
        let image = glyphs
            .rasterize(glyph.raster_key)
            .unwrap_or_else(|| panic!("{mark:?} for {constraint:?} has no glyph"));
        assert!(
            image.placement.width > 0 && image.placement.height > 0,
            "{mark:?} for {constraint:?} rasterized to nothing",
        );
    }
}

/// One of every relation the drawing can state, so a sweep over them is a sweep
/// over the enum.
///
/// Named for what it gathers rather than for relations alone, because it
/// gathers dimensions too — and the two want different things of a caller: a
/// relation is drawn as a symbol and a dimension as a number, so a sweep over
/// marks has to sift them and a sweep over the enum must not.
fn every_statable() -> Vec<silverpoint::Constraint> {
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

/// The drawing's faces reach the scene as sheets, and what they cover is what
/// its curves shut in.
///
/// The join between the arrangement and the picture. Everything else in this
/// module turns one piece of geometry into one primitive; a face is the one
/// thing drawn that no piece of the sketch corresponds to, so what it takes to
/// go wrong is a whole step being skipped rather than a field being mis-set.
#[test]
fn the_faces_a_drawing_encloses_are_written_as_sheets() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let scene = scene(
        document.models(&build, document.opening()),
        &mut Layout::default(),
    );

    // The demo's first sketch draws a rectangle with a circle inside it and an
    // eye at the end of the arm — the ring between rectangle and circle, the
    // circle's own disc, and the eye's — and its second draws a triangle.
    assert_eq!(scene.faces.len(), 4, "the demo encloses four faces");

    // Every sheet has triangles and lies flat in the plane of the sketch it
    // came from — a face with vertices off it would be one built through the
    // wrong basis. Flat in *its own* plane rather than on the ground: the demo
    // draws its second sketch on a datum held clear of it, so the two sit at
    // two heights and only the flatness is shared.
    for face in scene.faces.iter() {
        assert!(!face.mesh.indices.is_empty(), "a face was written empty");
        assert_eq!(face.mesh.indices.len() % 3, 0, "a face is triangles");
        let lies_at = face.mesh.vertices[0].position.y;
        for vertex in &face.mesh.vertices {
            assert!(
                (vertex.position.y - lies_at).abs() < 1e-5,
                "{vertex:?} is off the plane its face lies in, at {lies_at}"
            );
            assert!(
                vertex.normal.abs_diff_eq(Vec3::Y, 1e-5),
                "{vertex:?} faces away from the plane it lies in"
            );
        }
    }
    // And the two planes really are two: one sheet at least stands clear of the
    // ground, which is what says a sketch followed the datum it names.
    assert!(
        scene
            .faces
            .iter()
            .any(|face| face.mesh.vertices[0].position.y > 1e-5),
        "every face landed on the ground, so nothing followed the offset plane"
    );

    // Together they cover the rectangle, the eye and the triangle — the circle
    // inside the rectangle is a hole in one sheet and the whole of another, so
    // it neither adds nor subtracts.
    // Each face is measured against its own corners: the batch is a list of
    // sheets, and an index means anything only inside the one it came from.
    let covered: f32 = scene
        .faces
        .iter()
        .map(|face| {
            face.mesh
                .indices
                .chunks_exact(3)
                .map(|triangle| {
                    // The ground plane's axes are world +X and −Z, so this
                    // reads a corner back into the coordinates it was drawn in.
                    let at = |of: usize| {
                        let corner = face.mesh.vertices[triangle[of] as usize].position;
                        Vec2::new(corner.x, -corner.z)
                    };
                    let (a, b, c) = (at(0), at(1), at(2));
                    (b - a).perp_dot(c - a) / 2.0
                })
                .sum::<f32>()
        })
        .sum();
    // The frame is 8 by 5 and its eye has a radius of 0.45; the triangle is
    // 2.5 across its stated base and 1.4 up to its free apex, so half of that
    // is 1.75.
    let want = 40.0 + std::f32::consts::PI * 0.45 * 0.45 + 1.75;
    assert!(
        (covered - want).abs() < want * 0.001,
        "{covered} covered against {want}"
    );
}

/// The sketch being worked in is drawn in what it has left to decide; every
/// other is drawn as ground — and switching which is which redraws the picture
/// though no geometry moved.
///
/// Two claims that are one thing. A document holds several sketches and the
/// picture is of all of them, so something has to say which one you are in; and
/// because saying it moves nothing, a layout watching only the revision would
/// go on drawing the sketch you just left as the live one. The second redraw
/// below goes through the *same* layout for exactly that reason.
/// **A dormant sketch shows no marks at all**, where its geometry still shows,
/// dimmed.
///
/// A constraint is a statement *about* a drawing, and one you are not in is not
/// a drawing you can argue with: its marks can be neither selected into a
/// relation nor typed into, so all they would do is crowd the sketch you are
/// working in — and a dimension is the densest thing the drawing puts on
/// screen. Where a dormant sketch *is* stays visible, because that is something
/// you build against.
///
/// Its own fixture rather than an extension of the test below, which needs
/// geometry with its freedom intact to have a colour worth checking where this
/// needs geometry a constraint has taken hold of.
///
/// **And a field opening over a mark is a redraw too**, which is the last thing
/// asked here and the narrowest: the models do not move, the open sketch does
/// not change, and the only difference is one field of the [`Showing`] that
/// [`Made`] carries. A layout that read the revision alone would decline the
/// redraw and leave the mark on screen under the field standing over it — two
/// copies of one number, one of them stale. Which stage it reaches is
/// [`Layout::resume`]'s, and that it reaches the marks at all is what is asked
/// here.
#[test]
fn only_the_open_sketch_shows_its_constraints() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let mut stated = || {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::ZERO);
        // Across the axes, so what the mark is set along cannot come out right
        // by having been left at the sketch's own +x — see below.
        let b = sketch.add_point(DVec2::new(4.0, 3.0));
        sketch.add_segment(a, b);
        sketch.add_constraint(silverpoint::Constraint::Distance {
            a,
            b,
            along: Along::Shortest,
            dimension: Dimension::new(5.0),
        });
        timeline.add(Feature::Sketch { on: ground, sketch })
    };
    let here = stated();
    let there = stated();

    let mut build = Build::default();
    let document = Document::new(&mut build, timeline);
    let mut layout = Layout::default();
    let mut scene = Scene::default();

    // Which sketch each mark on screen belongs to, found through the names.
    let marked = |scene: &Scene, layout: &Layout| {
        scene
            .texts
            .iter()
            .filter_map(|mark| mark.tag.and_then(|tag| layout.names().get(tag)))
            .filter_map(|part| part.sketch())
            .collect::<Vec<_>>()
    };

    redraw(
        document.models(&build, here),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    assert_eq!(
        marked(&scene, &layout),
        [here],
        "a sketch nobody is in put its constraints on screen"
    );
    // Both sketches are still *drawn* — it is the marks alone that go.
    assert_eq!(scene.curves.len(), 2, "the picture is of both sketches");

    // The same layout, so the only thing that has changed is which sketch is
    // open — and the marks have to follow it.
    redraw(
        document.models(&build, there),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    assert_eq!(
        marked(&scene, &layout),
        [there],
        "the marks did not follow the open sketch"
    );

    // And every one of them is lettered on the drawing rather than pinned over
    // it: set along the sketch plane's own +x, on the plane the strokes of the
    // same sketch declare. Read off the drawing rather than written out, so what
    // is being asked is that the mark took *its* plane and not that the ground
    // happens to be where it is.
    //
    // Which is also the limit of what this can say. Every plane the timeline can
    // hold today is the ground or an offset parallel to it — see
    // [`Datum`] — so a basis hard-coded to the ground would pass. What it does
    // catch is the pair handed over the wrong way round, since the ground's +x
    // and its normal are different axes.
    let plane = document.models(&build, there).open().plane();
    let normal = plane.normal().as_vec3();
    // The fixture's one dimension spans (0,0) to (4,3), so it runs three-fifths
    // and four-fifths across the sketch's own axes — and not along either of
    // them, which is what says the drawing read the span rather than reaching
    // for the plane it is on.
    let along = (plane.x * 0.8 + plane.y * 0.6).as_vec3();
    assert!(!scene.texts.is_empty(), "there were no marks to ask about");
    for mark in scene.texts.iter() {
        let set = mark.facing.right().expect("a mark is laid in its plane");
        assert!(
            set.abs_diff_eq(along, 1e-6),
            "set along {set:?} rather than its span, {along:?}"
        );
        assert_eq!(mark.facing.normal(), Some(normal), "not on its own plane");
        assert_ne!(
            mark.facing.right(),
            mark.facing.normal(),
            "the run is set along its plane's normal rather than in the plane"
        );
        // Standing clear of the geometry rather than on it, and clear *in the
        // plane* — the one thing about a mark the projection cannot move.
        assert!(
            mark.facing.lift_world().length() > 0.0,
            "the mark sits on the line it measures"
        );
    }
    let facing = Facing::Turned(Turn::new(along, normal));
    // The same surface the strokes of that sketch took their depth off, which is
    // the cross-check: two writers reading one drawing's plane.
    assert!(
        scene
            .curves
            .iter()
            .any(|curve| curve.plane_normal == facing.normal()),
        "the marks and the strokes disagree about the plane they are on"
    );

    // The one mark on screen, as the part a field would open over.
    let over = scene
        .texts
        .iter()
        .find_map(|mark| mark.tag.and_then(|tag| layout.names().get(tag)))
        .expect("the open sketch drew a mark to type into");
    // The same models and the same layout again, differing in one field of the
    // `Showing`: the picture is stale all the same, and the mark goes because
    // the field standing over it is drawn in its place.
    redraw(
        document.models(&build, there),
        &mut layout,
        Showing {
            typed: Some(over),
            ..Showing::default()
        },
        &mut scene,
    );
    assert!(
        scene.texts.is_empty(),
        "the mark a field stands over was left on screen under it, so a picture \
         that differs only in what is being typed into was taken for current"
    );
}

#[test]
fn only_the_open_sketch_is_drawn_in_the_colours_of_its_freedom() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::Ground));
    let mut lone = || {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::ZERO);
        let b = sketch.add_point(DVec2::new(2.0, 0.0));
        sketch.add_segment(a, b);
        timeline.add(Feature::Sketch { on: ground, sketch })
    };
    let here = lone();
    let there = lone();

    let mut build = Build::default();
    let document = Document::new(&mut build, timeline);
    let mut layout = Layout::default();
    let mut scene = Scene::default();

    // What each sketch's one stroke was drawn in, found through the names,
    // because the batch is one run of strokes from every sketch at once.
    let drawn = |scene: &Scene, layout: &Layout, of| {
        scene
            .curves
            .iter()
            .filter(|curve| {
                curve
                    .tag
                    .and_then(|tag| layout.names().get(tag))
                    .is_some_and(|part| part.sketch() == Some(of))
            })
            .map(|curve| curve.color)
            .collect::<Vec<_>>()
    };

    redraw(
        document.models(&build, here),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    assert_eq!(scene.curves.len(), 2, "the picture is of both sketches");
    // Two free ends are two degrees of freedom apiece, so the live one is drawn
    // in what a wholly free edge is drawn in.
    assert_eq!(drawn(&scene, &layout, here), [FREE]);
    assert_eq!(drawn(&scene, &layout, there), [DORMANT]);

    // The same layout, so the only thing that has changed is which sketch is
    // open — and it is enough to make the picture stale.
    redraw(
        document.models(&build, there),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    assert_eq!(
        drawn(&scene, &layout, here),
        [DORMANT],
        "the picture did not follow the open sketch"
    );
    assert_eq!(drawn(&scene, &layout, there), [FREE]);
}

/// A colour no writer produces, so a primitive still wearing it is one a redraw
/// left alone.
///
/// What the stage ladder claims cannot be seen any other way. A batch rewritten
/// with the contents it already had is indistinguishable from one that was never
/// touched, and the whole point of the stages is the work *not* done — so every
/// primitive is stamped with something no drawing could arrive at, and what
/// still carries it afterwards is what was skipped.
const UNWRITTEN: Vec3 = Vec3::splat(-1.0);

/// Stamp every batch, so the next redraw can be asked which of them it wrote
/// over.
fn stamp(scene: &mut Scene) {
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
fn untouched(scene: &Scene) -> Vec<&'static str> {
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

/// **A redraw makes again what has moved, and leaves the rest where it is.**
///
/// The whole of what the ladder is for. A band travelling a pixel once said the
/// same thing to a layout that a solve does, and the answer to that was every
/// region cut again through the filler and every face of every solid skinned
/// again — on each of the frames there are most of.
///
/// Asked stage by stage, in the order they resume, and each one asserts the
/// *whole* list of what survived rather than picking at one batch: a stage that
/// quietly ran one writer too many is exactly the failure this exists to catch,
/// and a spot check would miss it in the direction that costs.
///
/// The demo rather than a bare sketch, because it is the one fixture that draws
/// all six kinds — it takes solids to say anything about the solids stage.
#[test]
fn a_redraw_makes_again_only_the_stages_whose_own_inputs_moved() {
    let mut build = Build::default();
    let mut document = demo::document(&mut build);
    let editing = document.opening();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    redraw(
        document.models(&build, editing),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    // Which is what makes [`untouched`] mean anything: an empty batch would
    // report itself skipped whatever happened to it.
    assert!(
        !scene.curves.is_empty()
            && !scene.rings.is_empty()
            && !scene.points.is_empty()
            && !scene.texts.is_empty()
            && !scene.faces.is_empty()
            && !scene.solids.is_empty(),
        "the demo stopped drawing one of the six kinds"
    );

    // A band between two places on the ground, which is what a half-drawn line
    // shows. Two of them, so the second frame is a band that has *moved* rather
    // than one that has appeared.
    let banding = |to: f32| Showing {
        band: Some(Preview::Line(Ends {
            from: Vec3::ZERO,
            to: Vec3::new(to, 0.0, 0.0),
        })),
        ..Showing::default()
    };

    stamp(&mut scene);
    redraw(
        document.models(&build, editing),
        &mut layout,
        banding(1.0),
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["points", "texts", "faces", "solids"],
        "a band appearing rewrote more than the strokes it is drawn among"
    );

    stamp(&mut scene);
    redraw(
        document.models(&build, editing),
        &mut layout,
        banding(2.0),
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["points", "texts", "faces", "solids"],
        "a band that only moved rewrote more than the strokes it is drawn among"
    );

    // Nothing at all: the picture is current, so no batch is written and every
    // stamp survives.
    stamp(&mut scene);
    redraw(
        document.models(&build, editing),
        &mut layout,
        banding(2.0),
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["curves", "rings", "points", "texts", "faces", "solids"],
        "a picture nothing had moved was drawn again"
    );

    // A solid being decided resumes one rung further up, so the marks and the
    // strokes go with it — they stand after the solids in the naming order and
    // are remade whenever anything before them is. What must not move is the
    // drawing's own points and faces, which no gesture can reach.
    stamp(&mut scene);
    redraw(
        document.models(&build, editing),
        &mut layout,
        Showing {
            growing: Some(Growing {
                sketch: editing,
                region: 0,
                distance: 1.0,
            }),
            ..Showing::default()
        },
        &mut scene,
    );
    assert_eq!(
        untouched(&scene),
        ["points", "faces"],
        "a solid being decided did not reach the solids, or reached past them"
    );

    // And an edit to the document reaches everything, which is the rung the
    // whole ladder hangs off: a stage that could survive a solve would be a
    // stage drawing geometry that has moved.
    stamp(&mut scene);
    document.apply(&mut build, Change::Tidy { sketch: editing });
    redraw(
        document.models(&build, editing),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    assert!(
        untouched(&scene).is_empty(),
        "a solved document left {:?} standing",
        untouched(&scene)
    );
}

/// A stage rewritten on its own leaves every name exactly where it was.
///
/// **What makes the ladder sound**, and the one thing about it that could break
/// in silence. A tag is a position in the walk that named the drawing, so a
/// partial redraw is only safe while what it writes names the same parts in the
/// same order — everything a gesture adds is untagged for exactly that reason.
/// Get it wrong and nothing looks amiss: the picture is right and every tag
/// reports its neighbour, so a hover lights the wrong edge and a press takes
/// hold of something nobody pointed at.
///
/// The whole list, tag for tag, rather than a count. A stage that named one part
/// fewer and one part more would keep the count and shift everything after it.
#[test]
fn a_stage_rewritten_on_its_own_leaves_every_name_where_it_was() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let editing = document.opening();
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    let named = |layout: &Layout| layout.names().iter().collect::<Vec<(Tag, Part)>>();

    redraw(
        document.models(&build, editing),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    let whole = named(&layout);
    assert!(!whole.is_empty(), "the demo named nothing");

    // Every stage below the first, each resumed from the last: a band, then a
    // solid being decided, then a mark with a field standing over it. None of
    // them may move a name, though the last of them takes one *out* of the
    // drawing — its own stage renames what follows, which is why the marks are
    // asked separately below.
    for showing in [
        Showing {
            band: Some(Preview::Line(Ends {
                from: Vec3::ZERO,
                to: Vec3::X,
            })),
            ..Showing::default()
        },
        Showing {
            growing: Some(Growing {
                sketch: editing,
                region: 0,
                distance: 1.0,
            }),
            ..Showing::default()
        },
    ] {
        redraw(
            document.models(&build, editing),
            &mut layout,
            showing,
            &mut scene,
        );
        assert_eq!(
            named(&layout),
            whole,
            "{showing:?} renamed the drawing around it"
        );
    }

    // A field opening over a mark is the one gesture that does move the names,
    // because the drawing answers it by leaving that mark out — so the marks
    // resume at their own stage and everything after them is renamed with them.
    // What has to survive is the run *before* them: the points, the faces and
    // the solids, which is where a partial redraw would otherwise be caught
    // shifting the drawing under a tag someone was already holding.
    let over = scene
        .texts
        .iter()
        .find_map(|mark| mark.tag.and_then(|tag| layout.names().get(tag)))
        .expect("the demo drew a mark to type into");
    // Everything named before the first mark, which is where the marks' own
    // stage begins: the drawing's points, the regions its curves enclose, and
    // the faces of the solids grown off them.
    let before = whole
        .iter()
        .take_while(|(_, part)| {
            !matches!(
                part,
                Part::Entity {
                    entity: Entity::Constraint(_),
                    ..
                }
            )
        })
        .copied()
        .collect::<Vec<_>>();
    assert!(
        before
            .iter()
            .any(|(_, part)| matches!(part, Part::Solid { .. })),
        "the run before the marks holds no solid, so this asks nothing"
    );
    redraw(
        document.models(&build, editing),
        &mut layout,
        Showing {
            typed: Some(over),
            ..Showing::default()
        },
        &mut scene,
    );
    assert_eq!(
        named(&layout)[..before.len()],
        before[..],
        "a field opening over a mark renamed the drawing standing before it"
    );
    assert!(
        !named(&layout).iter().any(|(_, part)| *part == over),
        "the mark a field stands over is still named, so the field and the \
         number are both on screen"
    );
}
