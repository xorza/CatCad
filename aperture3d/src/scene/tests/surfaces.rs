//! What a surface hides, what it yields to, and what it is picked from.

use crate::camera::Camera;
use crate::curve::Curve;
use crate::hit::{HitAt, Precedence};
use crate::mesh::{Mesh, Vertex};
use crate::object::Object;
use crate::point::Point;
use crate::scene::tests::fixtures::*;
use crate::scene::*;
use crate::styled::Styled;
use crate::tag::Tag;
use crate::text::Text;
use crate::viewport::Viewport;
use glam::UVec2;
use glam::Vec2;
use glam::Vec3;

/// A face is picked anywhere over it, and loses to everything drawn on it.
///
/// The rule a surface needs that no other primitive does: a marker or a stroke
/// bounding a face lies *within* the face, so a face ranked with them would
/// take every click meant for its own boundary.
#[test]
fn a_surface_is_picked_anywhere_over_it_and_loses_to_what_is_drawn_on_it() {
    let viewport = Viewport::new(UVec2::new(200, 200));
    // Straight down the −Z axis at a square in the XY plane, two across and
    // centred, so screen centre is the middle of it.
    let camera = Camera {
        target: Vec3::ZERO,
        distance: 10.0,
        yaw: 0.0,
        pitch: 0.0,
        ..Camera::default()
    };
    let middle = Vec2::new(100.0, 100.0);

    let mut scene = Scene::default();
    let sheet = Tag::new(7);
    let mut mesh = Mesh::default();
    for corner in [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(-1.0, 1.0, 0.0),
    ] {
        mesh.vertices.push(Vertex {
            position: corner,
            normal: Vec3::Z,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    scene.faces.push(Object {
        mesh,
        tag: Some(sheet),
        ..Object::default()
    });

    // Anywhere over it answers, and the whole of it is one target.
    let hit = scene
        .nearest(Aim::new(&camera, middle, viewport, 6.0))
        .expect("the cursor is over the sheet");
    assert_eq!(hit.tag, sheet);
    assert_eq!(hit.at, HitAt::Surface);
    assert!(hit.world.abs_diff_eq(Vec3::ZERO, 1e-4), "{:?}", hit.world);
    // Not *near* it — over it. A distance would have it beat a nearer face for
    // no reason a user could see.
    assert_eq!(hit.screen, 0.0);

    // Well off it answers nothing, however wide the aim.
    assert!(
        scene
            .nearest(Aim::new(&camera, Vec2::new(4.0, 4.0), viewport, 6.0))
            .is_none()
    );

    // A marker in the middle of it takes the click instead, though the face is
    // just as much under the cursor.
    let marker = Tag::new(9);
    scene
        .points
        .push(Point::new(Vec3::ZERO).size(8.0).tagged(marker));
    let hit = scene
        .nearest(Aim::new(&camera, middle, viewport, 6.0))
        .expect("both are under the cursor");
    assert_eq!(
        hit.tag, marker,
        "the face swallowed a click meant for a point"
    );

    // And so does a stroke running across it.
    scene.points.clear();
    let edge = Tag::new(11);
    scene.curves.push(
        Curve::new(vec![Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)])
            .width(2.0)
            .tagged(edge),
    );
    let hit = scene
        .nearest(Aim::new(&camera, middle, viewport, 6.0))
        .expect("both are under the cursor");
    assert_eq!(
        hit.tag, edge,
        "the face swallowed a click meant for an edge"
    );

    // And no standing puts the face back in front. The edge is set aside and
    // then made furniture outright, while the face belongs to the sketch being
    // worked in — and the edge takes it both times, because a face beating what
    // stands on it is a face with no boundary anyone can click. This is the
    // strong form, and it is structural: the ordering is never asked, since a
    // surviving overlay is taken before the ground is looked at.
    for standing in [Precedence::Aside, Precedence::Frame] {
        scene.curves.clear();
        scene.curves.push(
            Curve::new(vec![Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)])
                .width(2.0)
                .tagged(edge)
                .precedence(standing),
        );
        let hit = scene
            .nearest(Aim::new(&camera, middle, viewport, 6.0))
            .expect("both are under the cursor");
        assert_eq!(
            hit.tag, edge,
            "a {standing:?} edge lost to the face under it"
        );
    }
}

/// A sheet answers from either side, because a sheet has no outside.
#[test]
fn a_surface_is_picked_from_behind_as_well_as_in_front() {
    let viewport = Viewport::new(UVec2::new(200, 200));
    let middle = Vec2::new(100.0, 100.0);
    let mut scene = Scene::default();
    let sheet = Tag::new(3);
    let mut mesh = Mesh::default();
    for corner in [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ] {
        mesh.vertices.push(Vertex {
            position: corner,
            normal: Vec3::Z,
        });
    }
    mesh.indices.extend([0, 1, 2]);
    scene.faces.push(Object {
        mesh,
        tag: Some(sheet),
        ..Object::default()
    });

    // From +Z, which is the side the winding faces, and from −Z, which is not.
    for yaw in [0.0, std::f32::consts::PI] {
        let camera = Camera {
            target: Vec3::ZERO,
            distance: 10.0,
            yaw,
            pitch: 0.0,
            ..Camera::default()
        };
        let hit = scene
            .nearest(Aim::new(&camera, middle, viewport, 6.0))
            .unwrap_or_else(|| panic!("the sheet vanished seen from yaw {yaw}"));
        assert_eq!(hit.tag, sheet);
    }
}

/// A surface hides what is behind it from the aim, and never what is level with
/// it.
///
/// The two halves are one rule and they pull opposite ways, which is why the
/// ordering alone could not say it. A face has to lose to the strokes bounding
/// it — they lie *within* it on screen, so a face that could beat them would
/// swallow every click meant for its own boundary, and that is what ranking a
/// backdrop last buys. But a label on some other plane, genuinely behind, was
/// answering through the face as well; a face you can see a drawing through is
/// not a face you should be able to click a drawing through.
///
/// It holds between two surfaces as much as between a surface and what is drawn
/// on it, and the last two sweeps are that half: depth settles them, and the
/// standing only speaks where they are level and neither hides the other.
#[test]
fn a_surface_hides_what_is_behind_it_and_not_what_is_level_with_it() {
    /// A quad facing the camera at `z`, as a two-sided sheet.
    fn sheet(z: f32) -> Object {
        let at = |x: f32, y: f32| Vertex {
            position: Vec3::new(x, y, z),
            normal: Vec3::Z,
        };
        Object::new(Mesh {
            vertices: vec![at(-2.0, -2.0), at(2.0, -2.0), at(2.0, 2.0), at(-2.0, 2.0)],
            indices: vec![0, 1, 2, 0, 2, 3],
        })
    }

    let mut scene = Scene::default();
    scene.faces.push(sheet(0.0).tagged(Tag::new(1)));
    // A label a whole unit behind the sheet — another sketch's, seen through it.
    scene.texts.push(
        Text::new(Vec3::new(0.0, 0.0, -1.0), "8.00", 12.0)
            .measured(Vec2::new(40.0, 12.0))
            .tagged(Tag::new(2)),
    );
    let aim = Aim::new(&Camera::head_on(), CENTRE, Viewport::hundred(), 1.0);
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "the aim reached a label through the surface in front of it"
    );

    // The same label brought level with the sheet — its own sketch's, now —
    // beats it, which is the half the ordering was already right about.
    scene.texts.clear();
    scene.texts.push(
        Text::new(Vec3::ZERO, "8.00", 12.0)
            .measured(Vec2::new(40.0, 12.0))
            .tagged(Tag::new(2)),
    );
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(2)),
        "a surface took the click meant for what is drawn on it"
    );

    // A surface hides other surfaces as readily as it hides what is drawn over
    // them. A sheet set aside sits in front of one being worked in, with a label
    // behind them both: the near sheet answers, because what a preference
    // between two surfaces would be choosing between is one the cursor is over
    // and one it is not. Being set aside makes a surface a worse answer than the
    // things drawn *on* it, not a worse answer than being visible at all.
    scene.texts.clear();
    scene.faces.clear();
    scene
        .faces
        .push(sheet(0.0).tagged(Tag::new(1)).precedence(Precedence::Aside));
    scene.faces.push(sheet(-1.0).tagged(Tag::new(3)));
    scene.texts.push(
        Text::new(Vec3::new(0.0, 0.0, -2.0), "8.00", 12.0)
            .measured(Vec2::new(40.0, 12.0))
            .tagged(Tag::new(2)),
    );
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "a sheet behind another took the click meant for the one in front"
    );

    // The same with nothing set aside, which is the answer either way — so the
    // sweep above turned on the standing and not on the depth alone.
    scene.faces.clear();
    scene.faces.push(sheet(0.0).tagged(Tag::new(1)));
    scene.faces.push(sheet(-1.0).tagged(Tag::new(3)));
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "between two sheets alike, the nearer one answers"
    );

    // Level with each other, and the standing decides after all — which is the
    // half depth has no opinion about. Two sketches on one plane overlap without
    // either hiding the other, and there the one being worked in is the one a
    // click was meant for.
    scene.faces.clear();
    scene
        .faces
        .push(sheet(0.0).tagged(Tag::new(1)).precedence(Precedence::Aside));
    scene.faces.push(sheet(0.0).tagged(Tag::new(3)));
    assert_eq!(
        scene.nearest(aim).map(|hit| hit.tag),
        Some(Tag::new(3)),
        "the coplanar sheet set aside took the click from the one being worked in"
    );

    // And the label behind stayed hidden throughout, whichever sheet answered.
    assert_ne!(scene.nearest(aim).map(|hit| hit.tag), Some(Tag::new(2)));

    // **And a sheet set aside hides as well as any other, both ways round.**
    // The one thing standing has no say in. It was given one for a while —
    // a dormant sketch's region floats over the open one and its numbers are
    // readable straight through it, which looked like a reason to let the open
    // sketch answer through it — and the exemption is what let a number a whole
    // plane back take the click from the sheet in front of it. Hiding is a fact
    // about the eye: what is in front is what the cursor is over, and standing
    // decides between what survives rather than what is visible.
    for (sheet_stands, label_stands) in [
        (Precedence::Aside, Precedence::Shaped),
        (Precedence::Shaped, Precedence::Aside),
    ] {
        scene.faces.clear();
        scene.texts.clear();
        scene
            .faces
            .push(sheet(0.0).tagged(Tag::new(1)).precedence(sheet_stands));
        scene.texts.push(
            Text::new(Vec3::new(0.0, 0.0, -1.0), "8.00", 12.0)
                .measured(Vec2::new(40.0, 12.0))
                .tagged(Tag::new(2))
                .precedence(label_stands),
        );
        assert_eq!(
            scene.nearest(aim).map(|hit| hit.tag),
            Some(Tag::new(1)),
            "a {sheet_stands:?} sheet let a {label_stands:?} label a plane behind it take \
             the click"
        );
    }
}

/// A frame hides what is behind it and yields to what is level with it.
///
/// The same rule as the surfaces above, one kind further out, and it exists for
/// the same reason theirs does: [`Hit::aim_order`] settles what a thing is *for*
/// before how far off it is, so a frame — which ranks below every kind of
/// geometry there is — was losing to any edge of any sketch however far behind
/// it that sketch lay. Aimed at a datum plane with some other sketch's edge
/// lining up behind it, the edge took the click.
///
/// Both halves matter and they pull opposite ways, exactly as the surfaces do.
/// A frame has to keep losing to the geometry it frames, because a datum's
/// rectangle is drawn around a sketch in that sketch's own plane and the two are
/// level by construction — that is what ranking it last buys, and it is the
/// whole reason a datum is not merely ordinary geometry. What it must not lose
/// to is a drawing a plane away.
///
/// Two of the five below turn on the rule and three hold it in place. The first
/// is the bug, and the fourth is the boundary the ordering would have settled
/// the other way; the rest answer the same either way by design — they say the
/// filter did not take the frame's own reason for existing with it, that it
/// reaches backwards only, and that it stopped at frames rather than swallowing
/// [`Precedence::Aside`] along the way.
#[test]
fn a_frame_keeps_the_click_against_what_is_behind_it_and_yields_to_what_is_level() {
    /// A stroke straight across the view at `z`, `off` above the middle of it.
    fn across(z: f32, off: f32) -> Curve {
        Curve::segment(Vec3::new(-2.0, off, z), Vec3::new(2.0, off, z))
    }

    /// The same through the middle, which is where the cursor is.
    fn under(z: f32) -> Curve {
        across(z, 0.0)
    }

    // A datum plane's outline a unit in front of the target, and some other
    // sketch's edge four units behind it — a whole plane away, and lined up
    // under the same pixel.
    let mut scene = Scene::default();
    scene
        .curves
        .push(under(1.0).tagged(Tag::new(1)).precedence(Precedence::Frame));
    scene.curves.push(under(-3.0).tagged(Tag::new(2)));

    // Ten pixels, which is what `nearer_the_cursor_beats_nearer_the_eye` asks
    // for: the stroke set four pixels off the cursor below has to be in reach,
    // or the case that turns on it would be testing an empty scene.
    let aim = || Aim::new(&Camera::head_on(), CENTRE, Viewport::hundred(), 10.0);
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "an edge a plane behind the datum took the click meant for it"
    );

    // Level with it — the sketch the datum is drawn around — and the geometry
    // wins, which is the half the ordering was already right about and the
    // reason the datum is a frame at all.
    scene.curves.clear();
    scene
        .curves
        .push(under(1.0).tagged(Tag::new(1)).precedence(Precedence::Frame));
    scene.curves.push(under(1.0).tagged(Tag::new(2)));
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(2)),
        "the datum took the click meant for the sketch drawn on it"
    );

    // And geometry in *front* of the frame keeps winning, which is neither half
    // of the rule but is what says the filter reaches backwards only.
    scene.curves.clear();
    scene.curves.push(
        under(-1.0)
            .tagged(Tag::new(1))
            .precedence(Precedence::Frame),
    );
    scene.curves.push(under(1.0).tagged(Tag::new(2)));
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(2)),
        "a frame behind an edge stopped the edge from answering"
    );

    // Between two datums the nearer answers, and this is where the rule earns
    // its place rather than agreeing with what was there: the pair is the very
    // arrangement `nearer_the_cursor_beats_nearer_the_eye` is built on — the
    // near one four pixels off the cursor, the far one dead under it — where
    // the ordering hands the answer to the *further* stroke. Two frames instead
    // of two edges, and the nearer takes it.
    scene.curves.clear();
    scene.curves.push(
        across(1.0, 0.4)
            .tagged(Tag::new(1))
            .precedence(Precedence::Frame),
    );
    scene
        .curves
        .push(under(0.0).tagged(Tag::new(2)).precedence(Precedence::Frame));
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(1)),
        "the further datum answered, so the pair fell through to the cursor"
    );

    // A sketch set aside is not a frame and keeps its old standing: the one
    // being worked in wins from behind, because being somewhere else in the
    // document is not a fact about depth. This is the case the wider rule —
    // nearest overlay wins outright — would have got wrong.
    scene.curves.clear();
    scene
        .curves
        .push(under(1.0).tagged(Tag::new(1)).precedence(Precedence::Aside));
    scene.curves.push(under(-3.0).tagged(Tag::new(2)));
    assert_eq!(
        scene.nearest(aim()).map(|hit| hit.tag),
        Some(Tag::new(2)),
        "a dormant sketch in front took the click meant for the one being edited"
    );
}

/// **Nothing a pick answers with lies behind a surface the aim crosses.**
///
/// The other rule [`Scene::nearest`] states, and the one that was made
/// conditional for a day: a surface set aside was let off hiding the drawing
/// being worked in, and a number a whole plane back started taking the click
/// from the sheet in front of it. Hiding is a fact about the eye — what is in
/// front is what the cursor is over — and standing decides between what survives
/// that, not what is visible.
///
/// Swept over every combination of standings the two can be in, because the
/// exemption was expressible only as a difference between them and would have
/// passed a sweep that held both alike.
#[test]
fn nothing_answers_from_behind_a_surface_the_aim_crosses() {
    for overlay in [Precedence::Shaped, Precedence::Aside] {
        for surface in [Precedence::Shaped, Precedence::Aside] {
            let mut scene = one_of_each(overlay);
            // A sheet across the whole view, in front of everything above and
            // behind the cube — so some cursors are covered by it and some are
            // covered by something nearer still.
            scene.faces.push(
                Object::new(Mesh::cube(6.0))
                    .at(Vec3::new(0.0, 0.0, 2.6))
                    .tagged(Tag::new(9))
                    .precedence(surface),
            );
            let camera = Camera::head_on();
            for cursor in over_the_view() {
                let aim = Aim::new(&camera, cursor, Viewport::hundred(), 8.0);
                let Some(hit) = scene.nearest(aim) else {
                    continue;
                };
                if hit.at == HitAt::Surface {
                    continue;
                }
                let front = scene
                    .faces
                    .iter()
                    .chain(scene.solids.iter())
                    .filter_map(|mesh| mesh.pick(&aim, HitAt::Surface))
                    .fold(f32::INFINITY, |front, surface| front.min(surface.distance));
                assert!(
                    shows(front, hit.distance),
                    "a {overlay:?} {:?} answered from {} along the ray with a {surface:?} \
                     surface {front} away at {cursor:?}",
                    hit.at,
                    hit.distance,
                );
            }
        }
    }
}
