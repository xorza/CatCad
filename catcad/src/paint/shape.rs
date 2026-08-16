//! The shapes a drawing cuts for itself, in coordinates of their own.
//!
//! Two kinds, and what tells them apart is whether they have volume. A **flat**
//! shape is a polygon in 2D: a caller maps its corners through
//! [`Plane::point`](silverpoint::Plane::point) and gets geometry lying in that
//! plane, foreshortening as it turns and vanishing edge-on, because it genuinely
//! lies there. A [`Solid`] is corners in a 3D frame, each carrying the way its
//! face looks, and a caller maps them through a frame of three axes.
//!
//! Nothing here knows where anything sits. Cut on the CPU rather than shaped by
//! the renderer, which is what lets a control be ordinary world geometry: it
//! costs a shader that does nothing but transform, and it costs nothing at all
//! when the camera moves, since a shape measured in its own frame has no
//! opinion about where the camera is.

use glam::{DVec2, DVec3};

/// How far a datum's axis arrows reach from its origin, in sketch units.
///
/// A fixed length rather than one fitted to what is drawn. A gizmo says where a
/// plane's origin is and which way its axes run, and neither is a fact about
/// what happens to have been sketched there — one that grew with the drawing
/// would move every time a point did, and would have its geometry cut again on
/// every solve for the privilege.
pub(super) const ARROW_REACH: f64 = 1.2;

/// How much of the reach the head takes, and how wide shaft and head are, as
/// fractions of it.
///
/// Wide, and deliberately wider than a drawn arrow would be: this is a handle
/// before it is a picture, and what a thin one costs is a click that misses.
const HEAD: f64 = 0.3;
const SHAFT_HALF: f64 = 0.09;
const HEAD_HALF: f64 = 0.22;

/// How far the corner square runs along both axes, as a fraction of the reach.
///
/// Where it *starts* is not a number of its own: a shaft reaches [`SHAFT_HALF`]
/// either side of its own axis, so the square begins exactly there and is
/// tucked into the corner the two make rather than floating in the quadrant
/// between them. That is what a corner square is, and it is also the one
/// placement that meets both shafts without overlapping either — every piece of
/// a gizmo is coplanar and goes through one pass at one depth bias, so two that
/// overlapped would settle their overlap in the last bits of a depth each
/// computed from its own vertices.
const CORNER_SIDE: f64 = 0.26;

/// The two triangles a square's four corners make, shared by [`hub`] and
/// [`corner`] — both are squares, and a square is a square.
pub(super) const SQUARE_TRIANGLES: [[u32; 3]; 2] = [[0, 1, 2], [0, 2, 3]];

/// The four corners of the block the two axes cross at.
///
/// A piece of its own rather than each shaft running back to the origin, which
/// is what the two arrows used to do — and where they crossed they overlapped,
/// so the origin came out mottled. Every piece of a gizmo is coplanar and goes
/// through one pass at one depth bias, so an overlap is two surfaces deciding
/// which is in front by the last bits of a depth each computed from its own
/// vertices. Nothing about drawing them as one mesh fixes that; not overlapping
/// is what fixes it.
pub(super) fn hub() -> [DVec2; 4] {
    let half = SHAFT_HALF * ARROW_REACH;
    square(-half, half)
}

/// The four corners of the square sitting in the quadrant the two axes shut in.
///
/// No direction to be given, unlike [`arrow`]: there is one quadrant that both
/// axes point into, so where the square goes follows from the plane's own basis
/// rather than from anything a caller chooses.
pub(super) fn corner() -> [DVec2; 4] {
    let near = SHAFT_HALF * ARROW_REACH;
    square(near, near + CORNER_SIDE * ARROW_REACH)
}

/// The four corners of the square running from `low` to `high` on both axes,
/// wound the way [`SQUARE_TRIANGLES`] reads them.
///
/// Both squares a gizmo is made of go through here, which is the point: the two
/// differ in where they sit and in nothing else, and written out twice they
/// would be two chances to wind one of them the other way about.
fn square(low: f64, high: f64) -> [DVec2; 4] {
    [
        DVec2::new(low, low),
        DVec2::new(high, low),
        DVec2::new(high, high),
        DVec2::new(low, high),
    ]
}

/// The triangles [`arrow`]'s corners make: the shaft's quad, then the head.
///
/// Wound either way without consequence — the pass that draws these is
/// two-sided, a flat shape having no outside to be culled from.
pub(super) const ARROW_TRIANGLES: [[u32; 3]; 3] = [[0, 1, 2], [0, 2, 3], [4, 5, 6]];

/// The seven corners of an arrow along `along`: four for the shaft, then three
/// for the head.
///
/// It starts at the [`hub`]'s edge rather than at the origin, so that the two
/// arrows of a gizmo meet the hub instead of each other. Running both back to
/// the origin is what made them overlap in the block they share.
///
/// `along` is a unit direction in the plane rather than an axis by name, so one
/// arrow is drawn twice rather than a second set of coordinates being written
/// out mirrored — which is where the sign errors live.
pub(super) fn arrow(along: DVec2) -> [DVec2; 7] {
    // The plane's own perpendicular, turned a quarter the way its two axes are
    // from each other. Taken from `along` rather than passed in, so the two
    // cannot be handed a pair that is not square.
    let across = DVec2::new(-along.y, along.x);
    let corner = |x: f64, y: f64| (along * x + across * y) * ARROW_REACH;
    let base = 1.0 - HEAD;
    [
        corner(SHAFT_HALF, -SHAFT_HALF),
        corner(base, -SHAFT_HALF),
        corner(base, SHAFT_HALF),
        corner(SHAFT_HALF, SHAFT_HALF),
        corner(base, -HEAD_HALF),
        corner(base, HEAD_HALF),
        corner(1.0, 0.0),
    ]
}

/// One corner of a solid shape, in the shape's own frame: `x` runs along the
/// axis it points down, `y` and `z` across it.
///
/// Carries the way its face looks as well as where it is, because a caller
/// cannot work that out from the position — a solid arrow is *faceted*, so two
/// corners in the same place on two faces look different ways, and which face a
/// corner belongs to is the thing that decides how bright it reads.
#[derive(Debug, Clone, Copy)]
pub(super) struct Corner {
    pub(super) at: DVec3,
    pub(super) facing: DVec3,
}

/// A shape with volume, as corners and the triangles over them.
#[derive(Debug, Default)]
pub(super) struct Solid {
    pub(super) corners: Vec<Corner>,
    pub(super) triangles: Vec<[u32; 3]>,
}

impl Solid {
    /// Add a face, which is however many corners wound in order.
    ///
    /// Fanned from the first, which is right for anything convex and every
    /// face here is: a quad, a triangle, or a cap's ring.
    fn face(&mut self, at: &[DVec3], facing: DVec3) {
        let base = self.corners.len() as u32;
        self.corners
            .extend(at.iter().map(|&at| Corner { at, facing }));
        self.triangles
            .extend((1..at.len() as u32 - 1).map(|step| [base, base + step, base + step + 1]));
    }
}

/// How many facets a solid arrow is turned out of.
///
/// Eight reads as round at the size a gizmo is drawn and costs forty-odd
/// triangles. It is faceted rather than smoothed on purpose: a smooth normal
/// would make the arrow read as a lit *object* among the drawing's geometry,
/// where what it is is a control.
const FACETS: usize = 8;

/// The solid arrow, pointing down `+x` of its own frame and reaching
/// [`ARROW_REACH`].
///
/// Built once and handed out, because none of it depends on where the arrow
/// stands: the shape is constants, and only the frame it is mapped through
/// moves. A drag rewrites the gizmo every frame and this costs it nothing.
pub(super) fn solid_arrow() -> &'static Solid {
    static SHAPE: std::sync::OnceLock<Solid> = std::sync::OnceLock::new();
    SHAPE.get_or_init(|| {
        let shaft = SHAFT_HALF * ARROW_REACH;
        let head = HEAD_HALF * ARROW_REACH;
        let base = (1.0 - HEAD) * ARROW_REACH;
        let around = |step: usize, radius: f64, along: f64| {
            let angle = step as f64 / FACETS as f64 * std::f64::consts::TAU;
            DVec3::new(along, angle.cos() * radius, angle.sin() * radius)
        };
        let mut solid = Solid::default();
        for step in 0..FACETS {
            let (near, far) = (step, step + 1);
            // The shaft's side, and the head's, each facing out of the axis
            // rather than out of one of its two edges — a facet's own middle is
            // what it looks along.
            let out = |radius: f64| {
                let (a, b) = (around(near, radius, 0.0), around(far, radius, 0.0));
                DVec3::new(0.0, a.y + b.y, a.z + b.z).normalize_or_zero()
            };
            solid.face(
                &[
                    around(near, shaft, 0.0),
                    around(near, shaft, base),
                    around(far, shaft, base),
                    around(far, shaft, 0.0),
                ],
                out(shaft),
            );
            // The head, leaning in towards the tip: its facet looks part way
            // between straight out and straight along.
            // How far the head leans in towards the tip, which is how much of
            // its facet looks along the axis rather than out of it.
            let lean = head / (ARROW_REACH - base);
            let side = out(head);
            solid.face(
                &[
                    around(near, head, base),
                    DVec3::new(ARROW_REACH, 0.0, 0.0),
                    around(far, head, base),
                ],
                (side + DVec3::X * lean).normalize_or_zero(),
            );
            // The ring the head overhangs the shaft by, seen from below.
            solid.face(
                &[
                    around(near, shaft, base),
                    around(near, head, base),
                    around(far, head, base),
                    around(far, shaft, base),
                ],
                DVec3::NEG_X,
            );
        }
        // The two ends. The tail is a cap; the head has none, being a point.
        let tail: Vec<DVec3> = (0..FACETS)
            .rev()
            .map(|step| around(step, shaft, 0.0))
            .collect();
        solid.face(&tail, DVec3::NEG_X);
        solid
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An arrow is the length it says, widest at the head, and square to the
    /// direction it was given.
    ///
    /// The proportions are written out here as literals rather than read from
    /// the constants above, which is the whole of what makes this a test: the
    /// head's base at `1 - 0.3` of the reach, the shaft `0.09` either side and
    /// the head `0.22`. Against the reach itself, so that resizing the gizmo is
    /// one number and reshaping it is a failure here.
    #[test]
    fn an_arrow_reaches_its_full_length_and_is_widest_at_the_head() {
        // To a scaled product rather than exactly: `0.09 * 2.5` lands at
        // 0.22499999999999998, and a test that demanded the decimal would be
        // asserting how the multiply rounded rather than where the corner is.
        let at = |corner: DVec2, want: DVec2| {
            assert!(
                corner.abs_diff_eq(want, 1e-12),
                "expected {want:?}, got {corner:?}"
            );
        };
        let reach = ARROW_REACH;
        let arrow = arrow(DVec2::X);
        at(arrow[6], DVec2::new(reach, 0.0));
        // Tail on the hub's edge rather than on the origin: both arrows run
        // back to the same place, so one that reached the origin would overlap
        // the other in the block they share.
        at(arrow[0], DVec2::new(0.09 * reach, -0.09 * reach));
        at(arrow[3], DVec2::new(0.09 * reach, 0.09 * reach));
        // Head base: shaft corners and head corners share it, so the two pieces
        // meet rather than overlapping or leaving a gap.
        at(arrow[1], DVec2::new(0.7 * reach, -0.09 * reach));
        at(arrow[4], DVec2::new(0.7 * reach, -0.22 * reach));
        at(arrow[5], DVec2::new(0.7 * reach, 0.22 * reach));
        assert!(
            arrow[5].y > arrow[2].y,
            "a head no wider than its shaft is a bar, not an arrow"
        );
    }

    /// The corner square meets both shafts exactly, overlapping neither, and
    /// stops short of both heads.
    ///
    /// Two failures in one claim, and they pull opposite ways. Started past the
    /// shafts it floats in the middle of the quadrant and stops reading as the
    /// corner of anything; started inside them it overlaps, and an overlap is
    /// not a wrong picture but a speckled one — every piece of a gizmo is
    /// coplanar and goes through one pass at one depth bias, so two that
    /// overlapped would decide their overlap in the last bits of a depth each
    /// computed from its own vertices. Meeting exactly is the only placement
    /// that is neither.
    #[test]
    fn the_corner_square_meets_both_shafts_without_overlapping_them() {
        let square = corner();
        let (near, far) = (square[0], square[2]);
        // Square, and the same square either way about — so the gizmo reads the
        // same whichever axis you come at it from.
        let side = far - near;
        assert!((side.x - side.y).abs() < 1e-12, "{side:?} is not square");
        assert_eq!(near.x, near.y);

        // Exactly where the shaft's edge is: a shaft reaches `SHAFT_HALF`
        // either side of its own axis, and the square starts there.
        assert_eq!(
            near.y,
            SHAFT_HALF * ARROW_REACH,
            "the square starts {} from the x axis, where the shaft it should \
             meet ends at {}",
            near.y,
            SHAFT_HALF * ARROW_REACH,
        );
        // And short of the heads, which begin at `1 - HEAD` along.
        assert!(
            far.x < (1.0 - HEAD) * ARROW_REACH,
            "the square runs to {} along, into a head that begins at {}",
            far.x,
            (1.0 - HEAD) * ARROW_REACH,
        );
    }

    /// No two pieces of a gizmo overlap.
    ///
    /// The defect this exists for was visible and ugly: both arrows ran back to
    /// the origin, so they overlapped in the block they shared and it came out
    /// mottled. Coplanar surfaces at one depth bias have nothing to settle an
    /// overlap with but the last bits of a depth each computed from its own
    /// vertices, and drawing them as one mesh does not change that — only not
    /// overlapping does.
    ///
    /// Checked as axis-aligned boxes, and an arrow as *two* of them — a box
    /// round a whole arrow takes in the width of its head all the way down its
    /// shaft, which is empty plane the other arrow legitimately runs through.
    /// So each is split where [`arrow`] already splits it, at the corner its
    /// shaft ends and its head begins. The head is still only contained by its
    /// box rather than filling it, which can fail a layout that was fine but
    /// never pass one that was not.
    #[test]
    fn no_two_pieces_of_a_gizmo_overlap() {
        let box_of = |corners: &[DVec2]| {
            corners.iter().fold(
                (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
                |(low, high), &at| (low.min(at), high.max(at)),
            )
        };
        let (across, up) = (arrow(DVec2::X), arrow(DVec2::Y));
        let pieces = [
            ("hub", box_of(&hub())),
            ("x shaft", box_of(&across[..4])),
            ("x head", box_of(&across[4..])),
            ("y shaft", box_of(&up[..4])),
            ("y head", box_of(&up[4..])),
            ("corner", box_of(&corner())),
        ];
        for (i, (name, (low, high))) in pieces.iter().enumerate() {
            for (other, (their_low, their_high)) in &pieces[i + 1..] {
                // Touching is fine and wanted — the pieces are meant to meet —
                // so the overlap has to have *area* before it counts.
                let across = high.x.min(their_high.x) - low.x.max(their_low.x);
                let up = high.y.min(their_high.y) - low.y.max(their_low.y);
                assert!(
                    across <= 0.0 || up <= 0.0,
                    "{name} and {other} share {across} by {up} of the plane"
                );
            }
        }
    }

    /// **The solid arrow is a closed shell, and every number of it is a
    /// number.**
    ///
    /// A shape with volume is drawn through a two-sided pass, so a face left out
    /// would let the eye through into the shape's own inside — and the pass is
    /// unlit, so what came back would be flat colour rather than anything that
    /// read as a mistake.
    ///
    /// Checked by counting how many faces each edge belongs to: every edge of a
    /// closed shell is shared by exactly two. The disc from the axis out to the
    /// shaft, where the head sits on it, is *not* missing and does not need to
    /// be — the ring carries the surface from the shaft's rim out to the head's
    /// and the cone takes it to the tip, so the inside is enclosed without it.
    #[test]
    fn the_solid_arrow_is_a_closed_shell() {
        let solid = solid_arrow();
        assert!(!solid.triangles.is_empty());
        for corner in &solid.corners {
            assert!(corner.at.is_finite(), "a corner at {:?}", corner.at);
            assert!(
                (corner.facing.length() - 1.0).abs() < 1e-9,
                "a face looking {:?}, which is no direction",
                corner.facing
            );
        }

        // Reaches exactly as far as it says, and no further.
        let along = solid
            .corners
            .iter()
            .map(|corner| corner.at.x)
            .fold(f64::MIN, f64::max);
        assert!(
            (along - ARROW_REACH).abs() < 1e-9,
            "the arrow reaches {along}, where it says {ARROW_REACH}"
        );

        // Edges counted by where their two ends *are*, not by which corner
        // index they used: the shape is faceted, so a shared edge is two
        // corners in one place rather than one corner in two faces.
        /// A corner as something two faces can agree about — rounded, because
        /// the two sides of a shared edge reach it by different arithmetic.
        type At = (i64, i64, i64);
        let key = |at: DVec3| -> At {
            let round = |value: f64| (value * 1e6).round() as i64;
            (round(at.x), round(at.y), round(at.z))
        };
        let mut shared: std::collections::HashMap<(At, At), usize> =
            std::collections::HashMap::new();
        for &[a, b, c] in &solid.triangles {
            let at = |corner: u32| key(solid.corners[corner as usize].at);
            for (from, to) in [(a, b), (b, c), (c, a)] {
                let (from, to) = (at(from), at(to));
                let edge = if from < to { (from, to) } else { (to, from) };
                *shared.entry(edge).or_default() += 1;
            }
        }
        let open: Vec<_> = shared
            .iter()
            .filter(|(_, faces)| **faces != 2)
            .map(|(edge, faces)| (*edge, *faces))
            .collect();
        assert!(
            open.is_empty(),
            "{} edges of the shell are not shared by exactly two faces: {open:?}",
            open.len()
        );
    }

    /// The +y arrow is the +x arrow turned a quarter, corner for corner.
    ///
    /// The whole reason `along` is a direction and not an axis: the two arrows
    /// of a gizmo are one shape, so there is no second set of coordinates to
    /// get a sign wrong in. A quarter turn takes `(x, y)` to `(-y, x)`, and
    /// asserting that of every corner is what says the basis is right-handed in
    /// the plane rather than mirrored — a mirror would pass a test that only
    /// looked at the tip.
    #[test]
    fn the_second_axis_is_the_first_turned_a_quarter() {
        for (along, turned) in arrow(DVec2::X).into_iter().zip(arrow(DVec2::Y)) {
            assert_eq!(
                turned,
                DVec2::new(-along.y, along.x),
                "the +y arrow is not the +x arrow turned",
            );
        }
    }
}
