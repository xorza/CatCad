use super::*;
use crate::build::Build;
use crate::demo;
use crate::paint::growing::Growing;
use crate::paint::{MARK_FONT, redraw};
use crate::preview::Preview;
use aperture::Scene;
use silverpoint::{Along, Dimension, Sketch};

/// A plane that can be moved is drawn as a gizmo at its origin, and one that
/// cannot is not drawn at all.
///
/// The gizmo is what makes a datum a thing to aim at, and every piece of it
/// reports the plane: an axis is not a part of the drawing in its own right
/// yet, so a cursor over any of them is over the datum.
///
/// The ground draws nothing. It is what everything else is measured *from*
/// rather than something anybody put anywhere, and axes standing for it would
/// be axes standing for the world.
///
/// What this pins beyond the count is that the gizmo is *not* fitted to the
/// sketch: it starts at the plane's own origin and reaches a fixed distance,
/// so nothing about where the drawing happens to lie can move it. A gizmo
/// sized from the drawing would pass every count below and fail the last two.
#[test]
fn a_movable_plane_is_drawn_as_a_gizmo_at_its_origin() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    redraw(
        document.models(&build, Some(document.first_sketch())),
        &mut layout,
        Showing::default(),
        &mut scene,
    );
    write(
        document.models(&build, Some(document.first_sketch())),
        &mut layout,
        Showing::default(),
        Lens::new(
            document.camera(),
            aperture::Viewport::new(glam::UVec2::new(800, 600)),
        ),
        &mut scene.gizmos,
    );

    let named: Vec<_> = scene
        .gizmos
        .iter()
        .filter(|gizmo| {
            matches!(
                gizmo.tag.and_then(|tag| layout.names().get(tag)),
                Some(Part::Step(_))
            )
        })
        .collect();
    // Two squares, each naming its own plane: the one the drawing is standing
    // on, and the demo's one movable plane — which shows a square whatever is
    // open, because that square is the only thing there is to take hold of it
    // by. The other two the world comes with are neither, so while a sketch is
    // open they are not drawn.
    assert_eq!(named.len(), 2);
    let models = document.models(&build, Some(document.first_sketch()));
    let movable = models
        .planes()
        .find(|sheeted| sheeted.movable)
        .expect("the demo draws a datum that can be moved");
    // That plane's square, not whichever came first: the demo sketches on the
    // ground as well, and measuring against that one would be asking whether a
    // square lies in a plane it was never on.
    let square = named
        .iter()
        .find(|piece| {
            piece.tag.and_then(|tag| layout.names().get(tag)) == Some(Part::Step(movable.at))
        })
        .expect("the movable plane shows a square");
    let origin = movable.plane.point(DVec2::ZERO).as_vec3();
    let normal = movable.plane.normal().as_vec3();
    assert!(square.closed, "a plane's square does not close");
    // Every corner lies in the plane, which is the whole of what makes it flat
    // *in* the datum rather than a shape hung in front of it.
    for &corner in &square.points {
        let off = (corner - origin).dot(normal);
        assert!(off.abs() < 1e-5, "a corner stands {off} off its own plane");
    }

    // **Measured in pixels, not in the drawing.** Pulling the camera back makes
    // the gizmo bigger in the world by exactly as much as the projection then
    // shrinks it, so what reaches the screen is the same size — which is the
    // whole of what a control is: one that shrank with the zoom would stop
    // being grabbable exactly when you had zoomed out to find it.
    //
    // Asked on screen rather than in the world, because on screen is where the
    // claim is. The world span does *not* simply double with the distance: a
    // pixel covers world in proportion to depth along the view, and the shelf's
    // origin does not sit at the orbit target, so doubling the distance moves
    // it from `d + off` to `2d + off`.
    let viewport = aperture::Viewport::new(glam::UVec2::new(800, 600));
    let spans = |camera: aperture::Camera| {
        let lens = Lens::new(camera, viewport);
        let mut scene = Scene::default();
        let mut layout = Layout::default();
        let models = document.models(&build, Some(document.first_sketch()));
        redraw(models, &mut layout, Showing::default(), &mut scene);
        write(
            models,
            &mut layout,
            Showing::default(),
            lens,
            &mut scene.gizmos,
        );
        let middle = lens
            .screen_of(origin)
            .expect("the datum the gizmo stands on is behind the camera");
        // *That* plane's square, found by its tag: two are drawn while a sketch
        // is open, and one measured against the other plane's origin would be
        // measuring how far apart the two stand — a world length, which is
        // exactly what a control does not have.
        scene
            .gizmos
            .iter()
            .find(|gizmo| {
                gizmo.tag.and_then(|tag| layout.names().get(tag)) == Some(Part::Step(movable.at))
            })
            .expect("the movable plane shows a square")
            .points
            .iter()
            .map(|&at| {
                lens.screen_of(at)
                    .expect("a corner of the gizmo is behind the camera")
                    .distance(middle)
            })
            .fold(0.0f32, f32::max)
    };
    let near = document.camera();
    let far = aperture::Camera {
        distance: near.distance * 2.0,
        ..near
    };
    let (near, far) = (spans(near), spans(far));
    // To a pixel, against a span of about fifty. The shape is built in the
    // world from a scale taken at one point on it, and every corner is then
    // divided by its *own* depth — so a gizmo lying in a tilted plane comes out
    // a shade off what a screen-space layout would have drawn, by more of a
    // shade the nearer the camera is. That is the residue this allows for, and
    // a control that scaled with the zoom at all would miss by tens.
    assert!(
        (far - near).abs() < 1.0,
        "twice as far away the gizmo spans {far}px against {near}px, where it \
         should hold its size on screen"
    );
}

/// The arrow carrying a solid's depth turns its flat side to the camera.
///
/// It is an outline, so it has a side to turn, and laid out in a plane of the
/// sketch's own it would fold to a line the moment the camera came round to
/// look along that plane — a handle you cannot see being a handle you cannot
/// take hold of. The axis arrows of a datum do not do this and must not: lying
/// in the plane is what they say about it.
///
/// Both halves are the test. That the width is square to the view from one
/// angle could be the angle; that it is square from two and *different* at the
/// two is the camera being read rather than ignored.
#[test]
fn the_depth_arrow_turns_its_face_to_the_camera() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let showing = Showing {
        growing: Some(Growing {
            sketch: document.first_sketch(),
            region: 0,
            distance: 0.5,
        }),
        ..Showing::default()
    };
    let viewport = aperture::Viewport::new(glam::UVec2::new(800, 600));
    let widths = [0.0f32, 1.1].map(|yaw| {
        let camera = aperture::Camera {
            yaw,
            ..document.camera()
        };
        let mut scene = Scene::default();
        let mut layout = Layout::default();
        let models = document.models(&build, Some(document.first_sketch()));
        write(
            models,
            &mut layout,
            showing,
            Lens::new(camera, viewport),
            &mut scene.gizmos,
        );
        let arrow = scene
            .gizmos
            .iter()
            .find(|gizmo| {
                matches!(
                    gizmo.tag.and_then(|tag| layout.names().get(tag)),
                    Some(Part::Growing)
                )
            })
            .expect("a solid being grown has no arrow to carry it");
        // The arrow is what the gesture is *for* while a form is open, so it
        // takes the click over the geometry it stands over — where a plane's
        // square stands aside and yields to everything drawn on it. Ranking the
        // arrow aside would lose it the click it exists for.
        assert_eq!(arrow.precedence, Precedence::Shaped);
        assert!(
            scene
                .gizmos
                .iter()
                .filter(|piece| matches!(
                    piece.tag.and_then(|tag| layout.names().get(tag)),
                    Some(Part::Step(_))
                ))
                .all(|piece| piece.precedence == Precedence::Aside),
            "a plane's square stopped yielding to what is drawn on it"
        );
        // The two corners the head is widest between. Across the arrow rather
        // than along it, because the tip and the tail sit on its axis whichever
        // way the shape is turned, and the width is the whole of what an
        // edge-on outline loses.
        let across = arrow.points[2] - arrow.points[4];
        let out = across.dot(camera.facing());
        assert!(
            out.abs() < 1e-4,
            "the head reaches {out} out of the screen's own plane, so it reads \
             narrower than it is"
        );
        across
    });
    assert!(
        widths[0].angle_between(widths[1]) > 0.5,
        "the arrow lay the same way from both sides, so it is not turning at all"
    );
}

/// Moving the camera renames the controls rather than naming more of them.
///
/// The whole of what lets them be written on their own schedule, and a failure
/// nothing else would notice: a tag is a position in a list, so a second set
/// appended rather than written over leaves every tag still resolving and every
/// other assertion here still passing — while the list grows by a gizmo's worth
/// on every frame of an orbit. See
/// [`Names::truncate_to_drawn`](crate::paint::names::Names::truncate_to_drawn).
///
/// Turned between the writes rather than written twice from one place, because
/// what is being claimed is about a camera that *moved*: two passes from the
/// same place could come out equal for having been skipped.
#[test]
fn moving_the_camera_alone_renames_the_controls_rather_than_naming_more() {
    let mut build = Build::default();
    let document = demo::document(&mut build);
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    let models = document.models(&build, Some(document.first_sketch()));
    redraw(models, &mut layout, Showing::default(), &mut scene);
    let drawing = layout.names().iter().count();

    let viewport = aperture::Viewport::new(glam::UVec2::new(800, 600));
    let counted: Vec<_> = (0..4)
        .map(|step| {
            let camera = aperture::Camera {
                yaw: step as f32 * 0.3,
                ..document.camera()
            };
            write(
                models,
                &mut layout,
                Showing::default(),
                Lens::new(camera, viewport),
                &mut scene.gizmos,
            );
            layout.names().iter().count()
        })
        .collect();
    assert_eq!(
        counted, [counted[0]; 4],
        "the name list grew as the camera turned"
    );
    // And the controls did name something, so holding the count is a claim
    // about a list that was appended to rather than about an empty one.
    assert!(
        counted[0] > drawing,
        "{} names before the controls and {} after",
        drawing,
        counted[0]
    );

    // Every tag a control carries still reports the datum it was drawn for,
    // which is the half truncating has to keep true: a list written back over
    // could as easily have left the tags pointing into the drawing.
    for gizmo in scene.gizmos.iter().filter(|gizmo| gizmo.tag.is_some()) {
        let tag = gizmo.tag.expect("this one was just filtered for");
        assert!(
            matches!(layout.names().get(tag), Some(Part::Step(_))),
            "{tag:?} stopped naming the datum it belongs to"
        );
    }
    // And the batch does hold strokes with no name, which is what makes the
    // filter above a statement rather than a way of passing: a dimension's lines
    // are drawn here and deliberately unnamed, because what a dimension offers a
    // click is its number — see [`write`].
    assert!(
        scene.gizmos.iter().any(|gizmo| gizmo.tag.is_none()),
        "nothing in the batch was left unnamed, so the demo drew no dimension"
    );
}

/// **A dimension being placed is drawn twice over, out of one placement.**
///
/// The figure goes among the marks and the rule carrying it among the controls
/// — two halves of a frame on two schedules — so where the proposal stands is
/// the one thing they both have to agree about. They each worked it out for
/// themselves, which is the drift [`Placed`] keeps the *stated* marks out of and
/// the proposal went round; now both read one [`Proposed`], and what this pins
/// is that both still read it.
///
/// Ghost and untagged on both sides, which is the whole of what a proposal is:
/// the constraints have not been asked about it, so it has no state to report,
/// and nothing yet holds it, so there is nothing for a pick to land on. A number
/// the click has not stated that could be hovered would be a number you could
/// select and delete.
#[test]
fn a_dimension_being_placed_is_drawn_as_a_ghost_figure_and_a_ghost_rule() {
    // Four apart along the sketch's own x, so the number reads `4.00` and the
    // rule runs along that span.
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(4.0, 0.0));
    let placing = Constraint::Distance {
        a,
        b,
        along: Along::Shortest,
        dimension: Dimension::new(0.0),
    };
    let one = crate::paint::tests::fixtures::drawn(sketch);
    // Fitted by the drawing, which is what puts the measurement in it — a
    // proposal is written with a placeholder and the sketch fills it in.
    let placing = one
        .models()
        .open()
        .expect("a fixture opens the sketch it names")
        .sketch()
        .fitted(placing)
        .expect("four apart is a distance to state");

    let lens = Lens::new(
        aperture::Camera::default(),
        aperture::Viewport::new(glam::UVec2::new(800, 600)),
    );
    let showing = Showing {
        band: Some(Preview::Dimension(placing)),
        ..Showing::default()
    };
    let mut layout = Layout::default();
    let mut scene = Scene::default();
    redraw(one.models(), &mut layout, showing, &mut scene);
    write(one.models(), &mut layout, showing, lens, &mut scene.gizmos);

    // The sketch states nothing, so every mark and every control on screen
    // belongs to the proposal.
    let [figure] = &scene.texts[..] else {
        panic!("the proposal drew {} figures", scene.texts.len());
    };
    assert_eq!(figure.content, "4.00", "the figure is not the measurement");
    assert_eq!(figure.color, GHOST, "a proposal reads as a rubber band");
    assert_eq!(figure.tag, None, "a proposal can be picked out");

    // The rule under it — an extension line from each foot, the dimension line
    // itself, and a head at each end, see [`dimension::strokes`] — and the
    // square standing for the plane the whole of it is drawn on.
    assert_eq!(
        scene.gizmos.len(),
        6,
        "the rule and the square do not both stand"
    );
    // The rule alone. A plane's square is in this batch too, and unlike the
    // rule it is both tagged and drawn in its plane's own ink — so what tells
    // the two apart is the width each is stroked at.
    for stroke in scene
        .gizmos
        .iter()
        .filter(|stroke| stroke.width != SHEET_WIDTH)
    {
        assert_eq!(stroke.color, GHOST, "the rule and the figure disagree");
        assert_eq!(stroke.tag, None, "a proposal's rule can be picked out");
    }

    // And the two are placed from one answer: the figure is anchored on the span
    // it measures, and the rule runs through that same anchor dropped by the
    // clearance a number keeps off its own line. Half a line-height of it — the
    // mark's own `MARK_CLEAR` of 1.1 less the `RULE_DROP` of 0.6 — written out
    // rather than read off the constants, which is what makes this a test.
    //
    // The dimension line is the one stroke that spans the whole four, and its
    // middle is that anchor: it runs the same overshoot past each foot, and the
    // two feet sit either side of the number.
    let rule = scene
        .gizmos
        .iter()
        .find(|stroke| stroke.points[0].distance(stroke.points[1]) > 4.0)
        .expect("the dimension line runs past both feet");
    let apart = figure
        .position
        .distance(rule.points[0].midpoint(rule.points[1]));
    let drop = 0.5 * MARK_FONT.line_height_px * lens.world_per_pixel(figure.position);
    assert!(
        (apart - drop).abs() < 1e-5,
        "the figure sits {apart} from its own rule rather than {drop}, so the \
         two were placed apart"
    );
}
