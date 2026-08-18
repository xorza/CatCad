//! What an overlay comes out as: how wide a stroke is drawn, and how round a
//! rim stays.

use crate::harness::{DEMO_FRAME, capture, edge_on, painted};
use crate::ink::strokes;
use aperture::{Projection, Ring, Styled, Viewport};
use catcad::CatCad;
use glam::{UVec2, Vec2, Vec3};

/// Read from the drawing rather than restated, so that the width these tests
/// hold a stroke to is the width the drawing was actually drawn with. The
/// harness renders at scale 1, so a logical pixel is a pixel and this is what a
/// fully drawn stroke deposits.
fn authored_width() -> f32 {
    CatCad::edge_width()
}

/// The finest a single crossing can be measured.
///
/// Overlays are drawn against four samples with nothing blending, so what a
/// stroke deposits lands on a multiple of a quarter pixel: one authored at 1.6
/// measures 1.50 or 1.75 and never 1.6. Nothing is wrong with the stroke — that
/// is the whole resolution the frame has to answer in, and the phase of the
/// stroke against the sample grid decides which side it lands. It is why
/// [`deposited`] averages many crossings instead of reading one.
const QUANTUM: f32 = 0.25;

/// Columns [`deposited`] measures across.
///
/// Narrow, and about the circle's own centre, because a scan measures a stroke
/// honestly only where it crosses one square on: a stroke crossed at an angle
/// deposits its width over `1 / sin` of it, and averaging that in would measure
/// the geometry's slope rather than its width. Over this band the circle's
/// tangent stays within a few degrees of horizontal and the rectangle's near
/// and far edges are horizontal outright.
const MEASURED_COLUMNS: std::ops::Range<u32> = 415..446;

/// How far from the authored width a mean may sit before it is a defect.
///
/// A shade under one [`QUANTUM`]: the mean of sixty-odd crossings has a
/// standard error near 0.03, so this is wide enough that the sampling cannot
/// trip it and narrow enough to catch the tenth of a width the two primitives
/// were out by.
const WIDTH_TOLERANCE: f32 = QUANTUM * 0.8;

/// What every overlay is allowed on top of that, and shouldn't be.
///
/// All three shade their own coverage and hand it to alpha-to-coverage, which
/// quantises it to a sample mask — and the quantisation is not symmetric, so a
/// stroke lands about an eighth of a pixel narrow whatever it asked for. It is
/// flat in the viewing angle and flat in the width, and it cannot be dialled
/// out: biasing the coverage to compensate only feeds the biased value to the
/// same quantiser, which returned about a third of what was added when it was
/// tried. What would move it is more samples.
///
/// Recorded here rather than folded quietly into the tolerance above, because
/// the two say different things: one is how precisely the frame can be
/// measured, the other is a defect with a known cause and a known size.
const MASK_SHORTFALL: f32 = 0.15;

/// How far two overlays may disagree with each other about one authored width.
///
/// The tightest claim in this file, and the point of them all answering
/// coverage the same way: whatever a stroke costs in absolute terms, a rim
/// beside it has to cost the same, or a drawing is not one weight of line. The
/// two ran 0.19 apart when a curve counted samples and a rim shaded itself;
/// they now run inside 0.11.
const AGREEMENT: f32 = 0.2;

/// How much a primitive's width may move between the steepest view and the
/// shallowest.
///
/// The grazing claim in one number. Wider than [`WIDTH_TOLERANCE`] because the
/// two ends are separate means and each carries its own quantum-driven wobble,
/// and still less than half of the 0.35 a ring drifted by when it took its
/// pixel scale from `fwidth`.
const GRAZING_SPAN: f32 = QUANTUM;

/// The regression a fixed segment count cannot pass.
///
/// A polyline of `n` chords sits `r(1 − cos(π/n))` inside the arc at its
/// worst, so the 96 segments this used to be tessellated into cross a pixel of
/// error once the radius reaches about 1900 px on screen — and a sketch is
/// zoomed into, so it gets there. The ring is resolved in the fragment stage
/// instead, and the check is simply that the rim keeps one distance from the
/// centre all the way round.
#[test]
fn a_ring_stays_round_at_a_radius_that_would_facet_a_polyline() {
    /// Where the rim is put, in pixels from the centre. Past the ~1900 px at
    /// which 96 chords cross a pixel of error.
    const RIM_PX: f32 = 2400.0;
    /// Well clear of the pitch at which a Y-up view has no side to stand on.
    const PITCH: f32 = 1.0;

    let size = DEMO_FRAME;
    let mut app = CatCad::build();
    app.enter_first_sketch();
    {
        let mut view = app.renderer().borrow_mut();
        // Nothing else in the frame, so every lit pixel is the rim. The faces
        // among them: the demo's outlines enclose a filled sheet, and a sheet
        // is as much "something else" as the slab under it.
        let scene = view.scene_mut();
        scene.solids.clear();
        scene.faces.clear();
        scene.curves.clear();
        scene.points.clear();
        // Square to the eye, so the circle projects to a circle and roundness
        // is what the distances measure. Straight down would do it too, but
        // that is the one pitch where a Y-up view has no side to stand on.
        let (sin, cos) = PITCH.sin_cos();
        let rings = &mut scene.rings;
        rings.clear();
        rings.push(
            Ring::new(Vec3::ZERO, 1.0, Vec3::new(0.0, sin, cos))
                .colored(Vec3::new(0.35, 0.55, 0.80))
                .width(2.0),
        );
    }
    // The substituted ring survives the frame below because nothing in it
    // touches the drawing: the view lays the drawing out again only when the
    // document says it has moved on, and recording a frame that edits nothing
    // leaves the overlays exactly as they were set above.
    //
    // Parallel, so no foreshortening enters the measurement. Zoomed until a
    // world radius of 1 spans `RIM_PX`, and aimed at the rim rather than the
    // centre — at that magnification the centre is far off the frame and only a
    // shallow arc crosses it, which is exactly the arc a chord would visibly cut
    // across. Set on the document rather than the renderer, because the frame
    // below is recorded through the app and the app aims its renderer from the
    // document as it records.
    let camera = app.camera_mut();
    camera.projection = Projection::Orthographic;
    camera.target = Vec3::X;
    camera.yaw = 0.0;
    camera.pitch = PITCH;
    camera.distance = 4.0;
    camera.fov_y = 2.0 * (size.y as f32 / 2.0 / RIM_PX / camera.distance).atan();

    let frame = capture(size, &mut app);

    let viewport = Viewport::new(frame.size);
    let centre = viewport
        .pixel_of(frame.camera.view_proj(viewport.aspect()) * Vec3::ZERO.extend(1.0))
        .expect("the camera is aimed at the origin");

    // Every pixel of the rim, by how far it sits from where the centre
    // projected. Picked out by its blue rather than by brightness, because the
    // app's own status line is drawn over the viewport and is neither.
    let mut reach: Vec<f32> = Vec::new();
    for y in 0..frame.size.y {
        for x in 0..frame.size.x {
            let [r, _, b, _] = frame.pixel(UVec2::new(x, y));
            if f32::from(b) - f32::from(r) > 30.0 {
                reach.push(centre.distance(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)));
            }
        }
    }
    assert!(
        reach.len() > 500,
        "expected a long arc of rim to measure, got {} px",
        reach.len()
    );

    let near = reach.iter().copied().fold(f32::MAX, f32::min);
    let far = reach.iter().copied().fold(0.0f32, f32::max);
    // The stroke is two logical pixels wide and fades over one either side, so
    // four pixels of spread is the stroke itself and nothing more. Ninety-six
    // chords at this radius would wander a further 1.3 px as each one dips
    // inside the arc and climbs back out.
    assert!(
        far - near < 4.0,
        "the rim wandered {:.2} px, between {near:.1} and {far:.1} from the centre",
        far - near
    );
    // And it is the rim of the circle that was asked for, not some other
    // curve that happens to be smooth.
    assert!(
        (near - RIM_PX).abs() < 4.0 && (far - RIM_PX).abs() < 4.0,
        "the arc sits at {near:.1}..{far:.1} px, not the {RIM_PX} asked for"
    );
}

/// Which of the drawing's overlays a measurement is of.
///
/// One at a time, because a column crosses more than one of them and they do
/// not answer alike: reading them together averages a defect in one into the
/// health of the other, which is what hid a ring losing a fifth of its width
/// behind curves that had lost none.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Overlay {
    Curves,
    Rings,
}

/// What `overlay` deposits at `pitch`, in pixels, averaged over every crossing
/// in [`MEASURED_COLUMNS`].
///
/// Averaged rather than read once because a single crossing carries a whole
/// [`QUANTUM`] of noise — see there. Sixty-odd of them bring the standard error
/// to about 0.03, which is what makes a tenth of a width worth asserting on.
///
/// Everything else is emptied rather than filtered out afterwards: what a
/// column crosses is the demo's business and changes whenever the demo does,
/// where what is *drawn* is this test's to decide.
fn deposited(pitch: f32, overlay: Overlay) -> f32 {
    let frame = painted(DEMO_FRAME, |renderer| {
        edge_on(pitch)(renderer.camera_mut());
        // The markers, the constraint marks, the faces and the solids go in
        // both cases: none is either of the two overlays being weighed, and each
        // lands in the columns measured below — the first two on the ends of
        // every edge the column crosses and in the middle of what each relation
        // names, the third as a change of ground behind the very stroke being
        // counted, and the last standing in front of the drawing outright. The
        // solid is the one that *hides* rather than decorates: the demo grows a
        // cylinder off the hub, and seen from overhead it covers the rim it was
        // grown from — so a run left in would weigh nothing at all.
        //
        // The controls go too, and with them the lines a dimension is drawn
        // with. Neither is one of the two overlays either, and a dimension's is
        // the worse of the two to leave: an arrowhead is a filled triangle, so a
        // column crossing one measures a run several times the width being
        // weighed and drags the average with it.
        let scene = renderer.scene_mut();
        scene.points.clear();
        scene.texts.clear();
        scene.faces.clear();
        scene.solids.clear();
        scene.gizmos.clear();
        match overlay {
            Overlay::Curves => scene.rings.clear(),
            Overlay::Rings => scene.curves.clear(),
        }
    });
    let widths: Vec<f32> = MEASURED_COLUMNS
        .flat_map(|column| strokes(&frame, column).into_iter().map(|drawn| drawn.width))
        .collect();
    assert!(
        widths.len() >= 20,
        "{overlay:?} at pitch {pitch} crossed {} columns, too few to average",
        widths.len()
    );
    widths.iter().sum::<f32>() / widths.len() as f32
}

/// Every overlay holds the width it was authored at, whatever the view does.
///
/// The angles run from overhead to under 9°, which is where a surface's depth
/// changes fastest across a stroke lying on it — and where a ribbon widened in
/// screen space has no depth of its own to follow it down.
///
/// Each primitive separately, and then against the other, because those are two
/// different claims and only the second one is about them being one drawing.
/// Before the rim took its pixel scale from the length of the radius gradient it
/// read 1.75 overhead and 1.41 at the last of these angles, and no assertion
/// over the two of them together saw it.
#[test]
fn overlays_keep_their_authored_width_at_grazing_angles() {
    const PITCHES: [f32; 4] = [1.5, 0.6, 0.3, 0.15];
    let allowed = WIDTH_TOLERANCE + MASK_SHORTFALL;
    let mut by_overlay = Vec::new();

    for overlay in [Overlay::Curves, Overlay::Rings] {
        let mut measured = Vec::new();
        for pitch in PITCHES {
            let width = deposited(pitch, overlay);
            assert!(
                (width - authored_width()).abs() < allowed,
                "{overlay:?} at pitch {pitch} deposits {width:.3} px, not the {} it \
                 was authored at",
                authored_width()
            );
            measured.push(width);
        }

        // The spread across the angles, which is the grazing claim itself: a
        // primitive that answers the same overhead and edge-on has nothing left
        // to lose as the view tips, whatever it is worth in absolute terms.
        let (lo, hi) = measured
            .iter()
            .fold((f32::MAX, 0.0f32), |(lo, hi), &w| (lo.min(w), hi.max(w)));
        assert!(
            hi - lo < GRAZING_SPAN,
            "{overlay:?} runs {lo:.3}..{hi:.3} px across the angles, so it thins as the view grazes"
        );
        by_overlay.push(measured);
    }

    // And the claim the shared coverage rule exists to make: whatever a stroke
    // is worth, a rim at the same authored width is worth the same. This is
    // what a split between counting samples and shading coverage broke, and
    // what no per-primitive assertion above can see.
    for (index, pitch) in PITCHES.iter().enumerate() {
        let (curve, ring) = (by_overlay[0][index], by_overlay[1][index]);
        assert!(
            (curve - ring).abs() < AGREEMENT,
            "at pitch {pitch} a curve deposits {curve:.3} px and a ring {ring:.3}, \
             so the two are not one weight of line"
        );
    }
}

/// A rim seen along its own plane thins away instead of fanning out across the
/// screen.
///
/// A ring is widened *in its own plane* rather than in screen space, which is
/// what keeps every vertex on the plane and its depth exact. The width of that
/// band is worked out backwards, from how many pixels a world unit is worth at
/// the rim — and as the plane turns edge-on that rate collapses, so covering one
/// pixel of stroke asks for a band hundreds of world units across. What the
/// projection then leaves of a circle a centimetre wide is a wedge fanning out
/// over half the viewport.
///
/// Measured as the ink a free rim puts down, in its own colour. Nothing else in
/// the demo wears it: the free edges are the same orange, so the rim is measured
/// against the frame that has every one of them and no rim.
#[test]
fn a_rim_seen_edge_on_thins_rather_than_fanning_out() {
    /// Pixels wearing the colour a free rim is drawn in, with the rim either
    /// drawn or taken away.
    fn orange(pitch: f32, rims: bool) -> u32 {
        let frame = painted(UVec2::new(920, 520), |renderer| {
            edge_on(pitch)(renderer.camera_mut());
            // Close enough that a band running away in world units fills the
            // frame rather than passing off the side of it.
            renderer.camera_mut().distance = 5.0;
            if !rims {
                renderer.scene_mut().rings.clear();
            }
        });
        let mut ink = 0;
        for y in 0..frame.size.y {
            for x in 0..frame.size.x {
                let [r, g, b, _] = frame.pixel(UVec2::new(x, y));
                // The free-geometry orange. Tight enough to leave out the
                // demo's orange cube, which carries far more blue.
                if r > 200 && (150..=220).contains(&g) && b < 110 {
                    ink += 1;
                }
            }
        }
        ink
    }

    // Down to half a thousandth of a radian, which puts the eye all but exactly
    // in the sketch plane. The demo's hole is the rim that shows it: no radius
    // constraint, so it is drawn free and wears the colour counted above.
    // Nowhere near a tuned number: the runaway band deposited 34,387 px of
    // orange at the first of these, and a rim held to its collar deposits 15.
    // Anything between says the ceiling is there and is not merely large.
    for pitch in [0.0005f32, 0.002] {
        let (drawn, without) = (orange(pitch, true), orange(pitch, false));
        let rim = drawn as i64 - without as i64;
        assert!(
            rim < 400,
            "at pitch {pitch} the rim deposits {rim} px of orange over the {without} the \
             free edges leave, so it fanned out rather than thinning away"
        );
    }

    // And it is still a rim at an angle it can be seen at, so the ceiling above
    // is not simply switching the ring off — which is the way this test would
    // otherwise pass on a renderer that had stopped drawing circles. A rim of
    // the demo's radius seen from here is hundreds of pixels of stroke, so the
    // floor is as far from tuned as the ceiling.
    let (drawn, without) = (orange(0.5, true), orange(0.5, false));
    assert!(
        drawn as i64 - without as i64 > 100,
        "at a pitch of 0.5 the rim deposits only {} px, so it is not being drawn at all",
        drawn as i64 - without as i64
    );
}
