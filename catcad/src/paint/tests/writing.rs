//! What a drawing is painted as: the parts, the marks and the colours.

use crate::build::Build;
use crate::demo;
use crate::document::Document;
use crate::lens::Lens;
use crate::look::ink::{DORMANT, FREE};
use crate::paint::tests::fixtures::{drawn, every_statable};
use crate::paint::*;
use crate::part::Part;
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature, World};
use aperture::{Facing, Scene, Turn, Viewport};
use glam::{DVec2, UVec2, Vec2, Vec3};
use silverpoint::{Along, Dimension, Plane, Sketch};

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
        document.models(&build, Some(document.first_sketch())),
        &mut Layout::default(),
    );

    // One object per face of the one solid the demo grows: the two ends and the
    // single wall swept off the hub's circle, which is the whole boundary of a
    // cylinder.
    assert_eq!(picture.solids.len(), 3);
    // And both sketches: the frame's seven edges and the triangle's three, two
    // rims, nine markers and three. The planes are no part of this batch — they
    // hold their size on screen, so they are cut with the handles.
    assert_eq!(picture.curves.len(), 10);
    assert_eq!(picture.rings.len(), 2);
    assert_eq!(picture.points.len(), 12);
    // And no controls, which are not the document's: they are built against a
    // camera there is none of here, and `gizmos::write` is what writes them.
    assert!(picture.gizmos.is_empty());
}

/// Every relation is named both ways, and every mark it is named by has a glyph
/// in the faces the shaper falls back through.
///
/// The failure the second half guards is silent and total: a mark the fonts lack
/// rasterizes to nothing, so the relation is simply not drawn and the drawing
/// says a constraint is absent when it is not. Nothing else notices — the
/// records are built, the quads are laid out, and the sheet has no ink to give
/// them.
///
/// The first half guards the table itself. [`wording::named`] states a
/// relation's word and its mark on one line and a dimension's word and the
/// prefix its figure carries on another, and which of the two a constraint is
/// has to be the same answer [`Constraint::value`] gives — a dimension written
/// as a relation would be drawn as a mark *and* as its number, and a relation
/// written as a dimension would be drawn as neither.
///
/// Every variant, driven off `offers` rather than a list written twice, so a
/// fifteenth is covered the moment the drawing can state it.
#[test]
fn every_relation_is_named_both_ways_and_every_mark_has_a_glyph() {
    let shaper = palantir::TextShaper::new();
    let mut glyphs = shaper.glyphs();
    let mut placed = Vec::new();

    // Which of the two each is, against the one thing that decides it.
    for constraint in every_statable() {
        let named = crate::wording::named(constraint);
        assert!(
            !named.word.is_empty(),
            "{constraint:?} has no word to caption a control with"
        );
        assert_eq!(
            named.glyph.is_none(),
            constraint.value().is_some(),
            "{constraint:?} is named as a {} and states {:?}",
            if named.glyph.is_none() {
                "dimension"
            } else {
                "relation"
            },
            constraint.value(),
        );
        // A relation is drawn as its mark and has no figure, so a prefix on one
        // would be a string nothing could ever put anywhere.
        assert!(
            named.glyph.is_none() || named.prefix.is_empty(),
            "{constraint:?} is drawn as a mark and carries the prefix {:?}",
            named.prefix,
        );
    }

    // The relations alone. A dimension is drawn as its number, so it never
    // reaches `symbol` and asking it for one panics — see the arm there.
    for constraint in every_statable()
        .into_iter()
        .filter(|constraint| constraint.value().is_none())
    {
        let mark = crate::paint::symbol(constraint);
        // The face and the size the drawing sets marks in, not a stand-in: a
        // symbol the mono bold face lacks falls through to whatever the system
        // offers, and one nothing offers draws blank.
        glyphs.line(mark, crate::paint::MARK_FONT, 1.0, &mut placed);
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
        document.models(&build, Some(document.first_sketch())),
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
        assert!(
            !face.mesh.triangles().is_empty(),
            "a face was written empty"
        );
        let lies_at = face.mesh.vertices()[0].position.y;
        for vertex in face.mesh.vertices() {
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
            .any(|face| face.mesh.vertices()[0].position.y > 1e-5),
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
                .triangles()
                .iter()
                .map(|triangle| {
                    // The ground plane's axes are world +X and −Z, so this
                    // reads a corner back into the coordinates it was drawn in.
                    let [a, b, c] = triangle.map(|index| {
                        let corner = face.mesh.vertices()[index as usize].position;
                        Vec2::new(corner.x, -corner.z)
                    });
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
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
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
        document.models(&build, Some(here)),
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
        document.models(&build, Some(there)),
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
    let plane = document
        .models(&build, Some(there))
        .open()
        .expect("a fixture opens the sketch it names")
        .plane();
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
        document.models(&build, Some(there)),
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
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
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
        document.models(&build, Some(here)),
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
        document.models(&build, Some(there)),
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

/// With no sketch open every plane shows a square, and with one open only the
/// planes that square still says something about.
///
/// **The whole of the rule the squares follow**, and the two halves are one rule
/// seen from either side: a plane is drawn where it is what you are working
/// with. With none open picking a plane is the whole of what there is to do, so
/// all of them show; with one open the rest would be furniture standing in front
/// of the model — the three the world comes with cross at the origin, which is
/// where a model is built. What still says something then is the plane under the
/// drawing, and any plane that can be *moved*, whose square is the only thing
/// there is to take hold of it by.
///
/// The names follow a narrower rule again and are asserted here with them: they
/// share a batch with the marks, so they are written only while there are no
/// marks to write — see [`write::texts`](crate::paint::write::texts).
#[test]
fn every_plane_shows_itself_where_there_is_no_drawing_to_stand_in_front_of() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    // A plane's square holds its size on screen, so it is cut against the camera
    // with the other handles — which is why a lens is wanted here where the
    // drawing alone needed none.
    let lens = Lens::new(document.camera(), Viewport::new(UVec2::new(800, 600)));
    // Counted by what each names rather than by the width it is stroked at,
    // which is the claim: a square answers for its plane, and a dimension's rule
    // and the depth arrow in the same batch answer for other things or for
    // nothing.
    let squares = |scene: &Scene, layout: &Layout| {
        scene
            .gizmos
            .iter()
            .filter(|gizmo| {
                let named = gizmo.tag.and_then(|tag| layout.names().get(tag));
                matches!(named, Some(Part::Step(_)))
            })
            .count()
    };

    // Nothing open: all four planes the demo holds show a square, and the three
    // the world comes with name themselves. The shelf is one somebody put there
    // and has no name to carry.
    let mut idle = Layout::default();
    let mut looked = Scene::default();
    let models = document.models(&build, None);
    redraw(models, &mut idle, Showing::default(), &mut looked);
    gizmos::write(
        models,
        &mut idle,
        Showing::default(),
        lens,
        &mut looked.gizmos,
    );
    assert_eq!(
        squares(&looked, &idle),
        4,
        "a document shows all its planes"
    );
    assert_eq!(
        looked.texts.len(),
        3,
        "the three the world comes with name themselves"
    );
    // And no marks at all, which is the other half of that batch: a dimension is
    // something a drawing states, and none of them is being worked in.
    let said: Vec<&str> = looked
        .texts
        .iter()
        .map(|text| text.content.as_str())
        .collect();
    assert_eq!(said, ["XZ", "XY", "YZ"]);

    // **Laid into the plane each names**, rather than pinned over it: a name
    // advances along that plane's own +x and takes its depth from it, so the
    // sheet can hide it and turning the model turns it. Read off the turn rather
    // than off the pixels, because what a projection does with it is the
    // renderer's — what is decided here is which frame it was set in.
    for (text, plane) in looked
        .texts
        .iter()
        .zip([Plane::GROUND, Plane::FRONT, Plane::SIDE])
    {
        let Facing::Turned(turn) = text.facing else {
            panic!("{} is pinned over its plane, not set in it", text.content);
        };
        assert_eq!(
            turn.right,
            plane.x.as_vec3(),
            "{} runs the wrong way",
            text.content
        );
        assert_eq!(turn.normal, plane.normal().as_vec3());
        // **Centred on its own box**, with the whole offset in the lift. A box
        // hung off any other fraction is reflected through its point by the half
        // turn, so a name tucked into a corner from one side sticks out of it
        // from the other — which is what a look from behind showed. A centred
        // box is mapped onto itself and holds.
        assert_eq!(text.anchor, Vec2::splat(0.5));
        // Out to the square's top-left corner, in the plane's own axes: left
        // along +x and up along +y.
        assert!(
            turn.lift.x < 0.0 && turn.lift.y > 0.0,
            "{} is carried to the wrong corner",
            text.content
        );
    }

    // **Every one of them anchored on the origin**, which is where the three the
    // world comes with cross and the whole of what makes them read as an origin
    // at all. A square that followed whatever was sketched on its plane would
    // leave them crossing nowhere, which is not a thing a modeller draws.
    assert!(
        looked.texts.iter().all(|text| text.position == Vec3::ZERO),
        "a world plane's name left the origin its square is drawn at"
    );

    // What carries them apart is the lift, stated in each plane's own axes — so
    // three names anchored on one point still land in three places. No two share
    // a plane, which is what says the lifts differ.
    for (nth, text) in looked.texts.iter().enumerate() {
        let Facing::Turned(turn) = text.facing else {
            unreachable!("every name was matched as turned above");
        };
        for other in &looked.texts[nth + 1..] {
            let Facing::Turned(theirs) = other.facing else {
                unreachable!("every name was matched as turned above");
            };
            assert_ne!(
                turn.normal, theirs.normal,
                "{} and {} are set in one plane",
                text.content, other.content
            );
        }
    }

    // Opened on the first sketch: its plane alone shows a square, and the names
    // give the batch back to the marks.
    let mut drawn = Layout::default();
    let mut worked = Scene::default();
    let models = document.models(&build, Some(document.first_sketch()));
    redraw(models, &mut drawn, Showing::default(), &mut worked);
    gizmos::write(
        models,
        &mut drawn,
        Showing::default(),
        lens,
        &mut worked.gizmos,
    );
    // Two: the plane the drawing stands on, and the demo's one movable plane —
    // which shows a square whatever is open, that square being the only thing
    // there is to take hold of it by. The other two the world comes with are
    // neither, so they are not drawn.
    assert_eq!(
        squares(&worked, &drawn),
        2,
        "a plane stood in front of the model"
    );
    assert!(
        worked.texts.iter().all(|text| text.content != "XY"),
        "a plane named itself over the drawing"
    );
}
