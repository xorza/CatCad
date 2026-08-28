//! What the orientation gizmo is a solid of: a cube with every edge and every
//! corner cut away.

use glam::{IVec3, Vec3};

/// One flat piece of the chamfered cube — a face, an edge bevel or a corner
/// bevel.
///
/// **The direction it looks out along is the whole of it.** Which kind of piece
/// it is follows from how many of the three components are not zero: one is a
/// face, two is an edge, three is a corner. The ring of points bounding it
/// follows from that, and the view a press on it asks for *is* that direction.
/// Nothing else is stored because nothing else is decided.
///
/// **Twenty-six of them, and that is the point of the shape.** A plain cube
/// offers six faces and eight corners you can aim at, and the twelve half-way
/// views only by cutting each face into bands nobody can see. Cut the edges and
/// the corners off and every one of the twenty-six is a piece of the solid with
/// an outline of its own — so what you can press is what you can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Facet(IVec3);

/// Every piece, once each.
///
/// Built rather than written out, because the set *is* the twenty-six whole
/// directions with each component in `-1..=1` and not all three zero — and a
/// hand-written table of them is twenty-six chances to leave one out.
pub(super) const EVERY: [Facet; 26] = {
    let mut every = [Facet(IVec3::ZERO); 26];
    let mut at = 0;
    let mut x = -1;
    while x <= 1 {
        let mut y = -1;
        while y <= 1 {
            let mut z = -1;
            while z <= 1 {
                if x != 0 || y != 0 || z != 0 {
                    every[at] = Facet(IVec3::new(x, y, z));
                    at += 1;
                }
                z += 1;
            }
            y += 1;
        }
        x += 1;
    }
    every
};

impl Facet {
    /// The view a press on it asks for, as the whole numbers that say it.
    pub(super) fn out(self) -> Vec3 {
        self.0.as_vec3()
    }

    /// The same at unit length, which is what a light is dotted against.
    pub(super) fn normal(self) -> Vec3 {
        self.out().normalize()
    }

    /// The points bounding it, in ring order, written into `into`.
    ///
    /// **Ring order and not a winding.** Nothing downstream asks which way
    /// round it runs: a fan fills the same triangles either way, palantir culls
    /// no faces, and the test for a point inside asks whether the crossings
    /// agree rather than which sign they agree on. What it does owe is that
    /// consecutive points are neighbours, so the outline does not cross itself.
    ///
    /// **Every point of the solid reaches the full half-cube on one axis, stops
    /// short of the edge on the next and short of the corner on the third** —
    /// forty-eight of them, three axes by two partners by eight quarters. That
    /// one set is what makes the three kinds fall out, and what makes each of
    /// them the shape it is: a face is an octagon, an edge bevel a rectangle,
    /// and a corner a hexagon that runs face, bevel, face, bevel, face, bevel
    /// round its six sides.
    ///
    /// **The corner is cut back by the same amount the edge is**, which is why
    /// that hexagon is regular: three of its sides are the width of an edge cut
    /// and the other three the width of a corner cut, and one number sets both.
    /// Cut the corner *less* and it collapses to the triangle a plain chamfer
    /// leaves; cut it not at all and the twelve edges survive as edges, which
    /// is a solid with fourteen pieces and no bevel to press.
    ///
    /// `chamfer` is how far each cut reaches in, as a share of the half-cube —
    /// see [`Chrome::cube_chamfer`](crate::look::chrome::Chrome::cube_chamfer).
    pub(super) fn ring(self, chamfer: f32, into: &mut Vec<Vec3>) {
        into.clear();
        // How far a face reaches toward the edge it shares with a bevel, and
        // toward the corner beyond it — a corner being cut three times over,
        // once by each edge that runs into it and once by itself.
        let edged = 1.0 - chamfer;
        let cornered = 1.0 - 2.0 * chamfer;
        let sign = self.0.as_vec3();
        match self.0.abs().element_sum() {
            // An octagon: the face, cut where each of its four corners was.
            1 => {
                let axis = (0..3).find(|&at| self.0[at] != 0).expect("a face has one");
                let [u, v] = [(axis + 1) % 3, (axis + 2) % 3];
                for (across, along) in [
                    (edged, cornered),
                    (cornered, edged),
                    (-cornered, edged),
                    (-edged, cornered),
                    (-edged, -cornered),
                    (-cornered, -edged),
                    (cornered, -edged),
                    (edged, -cornered),
                ] {
                    let mut at = Vec3::ZERO;
                    at[axis] = sign[axis];
                    at[u] = across;
                    at[v] = along;
                    into.push(at);
                }
            }
            // A rectangle, running the length of the edge it was cut from. The
            // axis it does not span is the one it was cut *along*, so its two
            // ends are where the corners took over.
            2 => {
                let free = (0..3).find(|&at| self.0[at] == 0).expect("an edge has one");
                let [a, b] = [(free + 1) % 3, (free + 2) % 3];
                for (toward, ends) in [(b, 1.0), (a, 1.0), (a, -1.0), (b, -1.0)] {
                    let mut at = sign;
                    at[toward] *= edged;
                    at[free] = ends * cornered;
                    into.push(at);
                }
            }
            // A hexagon, alternating between the three faces and the three
            // bevels that run into the corner. Regular, because the corner is
            // cut back by the same amount the edges are — three of its sides
            // are the width of an edge cut and three the width of a corner cut.
            _ => {
                for [toward, away] in [[1, 2], [0, 2], [2, 0], [1, 0], [0, 1], [2, 1]] {
                    let mut at = sign;
                    at[toward] *= edged;
                    at[away] *= cornered;
                    into.push(at);
                }
            }
        }
    }
}

/// One of the six named faces: which way it looks, and the frame a word on it
/// is set in.
///
/// The axes travel with the direction rather than being worked out from it,
/// because a word has a way up: `u` runs along the line of the letters and `v`
/// rises through them, and which of the four quarter turns of the face that is
/// is a decision rather than a derivation. The top of a cube reads front-to-back
/// and the bottom reads the other way, so that a turn from one to the other does
/// not pass through a word standing on its head.
#[derive(Debug, Clone, Copy)]
pub(super) struct Side {
    out: IVec3,
    pub(super) u: Vec3,
    pub(super) v: Vec3,
    pub(super) name: &'static str,
}

/// Every face that carries a name, the way a drawing is read.
///
/// `TOP`, `FRONT` and `RIGHT` are what every CAD program writes on a cube, and
/// they stay that vocabulary here even though the recipe two corners away calls
/// the world's three planes `Ground`, `Front` and `Side`. The two are different
/// things: one is a direction you look from, the other is a sheet you draw on.
pub(super) const SIDES: [Side; 6] = [
    Side {
        out: IVec3::Y,
        u: Vec3::X,
        v: Vec3::NEG_Z,
        name: "TOP",
    },
    Side {
        out: IVec3::NEG_Y,
        u: Vec3::X,
        v: Vec3::Z,
        name: "BOTTOM",
    },
    Side {
        out: IVec3::Z,
        u: Vec3::X,
        v: Vec3::Y,
        name: "FRONT",
    },
    Side {
        out: IVec3::NEG_Z,
        u: Vec3::NEG_X,
        v: Vec3::Y,
        name: "BACK",
    },
    Side {
        out: IVec3::X,
        u: Vec3::NEG_Z,
        v: Vec3::Y,
        name: "RIGHT",
    },
    Side {
        out: IVec3::NEG_X,
        u: Vec3::Z,
        v: Vec3::Y,
        name: "LEFT",
    },
];

impl Side {
    /// The facet this face is.
    pub(super) fn facet(self) -> Facet {
        Facet(self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The solid is a cube with all twenty of its edges and corners cut**,
    /// and every piece of it is bounded by the right number of points.
    ///
    /// Six faces, twelve edges and eight corners is what a chamfered cube is;
    /// eight, four and six points is what each of those is bounded by. Both
    /// halves are asserted because the ring is *built* from the direction
    /// rather than looked up, so a construction that lost a point would draw a
    /// heptagon nobody would notice was one.
    #[test]
    fn every_piece_of_the_solid_is_bounded_by_the_points_its_kind_has() {
        let mut ring = Vec::new();
        let mut counted = [0; 4];
        for facet in EVERY {
            let kind = facet.0.abs().element_sum() as usize;
            counted[kind] += 1;
            facet.ring(0.25, &mut ring);
            let want = match kind {
                1 => 8,
                2 => 4,
                _ => 6,
            };
            assert_eq!(ring.len(), want, "{facet:?} is bounded by {}", ring.len());
            // Every point sits on the facet's own plane, which is what says the
            // ring bounds *it* rather than some neighbouring piece: the far
            // corner of the cut is the same distance out along the direction
            // for all of them.
            let reach = ring[0].dot(facet.out());
            for at in &ring {
                let off = at.dot(facet.out()) - reach;
                assert!(off.abs() < 1e-5, "{at:?} is {off} off {facet:?}'s plane");
            }
            // **A corner is a regular hexagon**, which is the whole of what
            // says the corner is cut back by the same amount its edges are:
            // three of its sides are the width of an edge cut and three the
            // width of a corner cut. A quarter cut leaves each of them the
            // diagonal of a quarter square — `√2 × 0.25`, or `0.3536`.
            if kind == 3 {
                for at in 0..ring.len() {
                    let side = (ring[(at + 1) % ring.len()] - ring[at]).length();
                    let want = std::f32::consts::SQRT_2 * 0.25;
                    assert!(
                        (side - want).abs() < 1e-5,
                        "a corner's side runs {side} rather than {want}",
                    );
                }
            }
        }
        assert_eq!(
            counted[1..],
            [6, 12, 8],
            "the solid is not a chamfered cube"
        );
    }

    /// Every named face is one of the six, no two share a direction, and each
    /// writes its word in its own plane.
    ///
    /// What keeps the naming honest in the two places it could rot quietly. A
    /// [`Side`] whose direction is not one of the six would put a word on a
    /// bevel, which has no room for one. And the frame a word is set in is
    /// *stated* rather than derived from the direction — which is what lets the
    /// top read front-to-back and the bottom read the other way — so nothing
    /// but this says the pair still lies in the face they belong to.
    #[test]
    fn every_named_face_is_one_of_the_six_and_writes_in_its_own_plane() {
        for side in SIDES {
            let facet = side.facet();
            assert_eq!(
                facet.0.abs().element_sum(),
                1,
                "{} is not a face",
                side.name
            );
            assert!(EVERY.contains(&facet), "{} is not on the solid", side.name);
            assert!(side.u.dot(side.v).abs() < 1e-6, "{}'s axes lean", side.name);
            assert!(
                side.u.cross(side.v).dot(facet.normal()).abs() > 0.999,
                "{}'s word is not written in its own plane",
                side.name,
            );
        }
        for (at, one) in SIDES.iter().enumerate() {
            for two in &SIDES[at + 1..] {
                assert_ne!(
                    one.out, two.out,
                    "{} and {} look one way",
                    one.name, two.name
                );
                assert_ne!(one.name, two.name, "two faces answer to {}", one.name);
            }
        }
    }
}
