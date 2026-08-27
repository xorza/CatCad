use super::*;
use crate::loops::Loops;
use crate::math::plane::Plane;
use crate::math::triangulate::Cutter;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::sphere::Sphere;

/// The frame every surface below is built on.
fn upright() -> Axis {
    Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X)
}

/// One face's parameters, ready to hand to [`Refining::refine`].
///
/// Held together because the three go together: the cutter works in cells,
/// the surface answers in its own parameters, and the boundary places are
/// what the walk left — see [`Mesher::patch`](crate::Mesher).
struct Patched {
    fill: Fill,
    boundary: Vec<DVec3>,
    lattice: Lattice,
}

impl Patched {
    /// Flatten `around`, cut it in the cells `surface` rules over.
    fn of(surface: &Surface, around: &[DVec2], sagitta: f64) -> Self {
        let lattice = Lattice::of(surface, around, sagitta);
        let celled: Vec<DVec2> = around.iter().map(|&uv| lattice.celled(uv)).collect();
        let mut fill = Fill::default();
        Cutter::default().polygon(&celled, &Loops::default(), &mut fill);
        let boundary = fill
            .corners
            .iter()
            .map(|&uv| surface.at(lattice.parameters(uv)))
            .collect();
        Self {
            fill,
            boundary,
            lattice,
        }
    }

    /// The corners of the fill in the surface's own parameters.
    fn params(&self) -> Vec<DVec2> {
        self.fill
            .corners
            .iter()
            .map(|&uv| self.lattice.parameters(uv))
            .collect()
    }

    /// Throw the triangles away and fan the whole boundary off its first
    /// corner, which is the worst triangulation of it there is and the one
    /// ear clipping fell into before a face was measured in cells.
    ///
    /// Corners of the fan that fall in a line leave triangles with no area
    /// in them, which is not a reason to leave them out: what is being
    /// asked here is whether the cutting holds up against *whatever* it is
    /// handed, and a triangulation with slivers in it is one of the things
    /// it may be handed.
    fn fanned(mut self) -> Self {
        let len = self.fill.corners.len() as u32;
        self.fill.triangles.clear();
        self.fill
            .triangles
            .extend((1..len - 1).map(|at| [0, at, at + 1]));
        self
    }

    /// Cut it down, and hand back the refining that did it.
    fn refined(&self, surface: &Surface, sagitta: f64) -> Refining {
        let mut refining = Refining::default();
        refining.refine(surface, &self.boundary, &self.fill, self.lattice, sagitta);
        refining
    }
}

/// Six cells of a unit cylinder two tall, its foot chorded every nine
/// tenths of a cell and its head every seven tenths.
///
/// Chorded in cells rather than in radians, so that asking for a finer
/// sagitta re-chords the boundary the way
/// [`chords`](crate::math::arc::chords) would rather than leaving it
/// coarser than the grid. A wall's cell *is* that chord, so a wall is never
/// handed over chorded coarser than one — a sphere is, and
/// [`a_face_chorded_coarser_than_its_cells_still_settles`] is that face.
fn wall(sagitta: f64) -> Vec<DVec2> {
    let cell = crate::math::arc::widest(1.0, sagitta);
    let (wide, tall) = (6.0 * cell, 2.0);
    let mut around = Vec::new();
    let mut at = 0.0;
    while at < wide {
        around.push(DVec2::new(at, 0.0));
        at += 0.9 * cell;
    }
    around.push(DVec2::new(wide, 0.0));
    let mut at = wide;
    while at > 0.0 {
        around.push(DVec2::new(at, tall));
        at -= 0.7 * cell;
    }
    around.push(DVec2::new(0.0, tall));
    around
}

/// A square patch of a sphere, six tenths of a radian either way, each side
/// of it cut into `steps` even chords.
fn dome(steps: usize) -> Vec<DVec2> {
    let mut around = Vec::with_capacity(4 * steps);
    for side in 0..4 {
        for step in 0..steps {
            let along = -0.6 + 1.2 * step as f64 / steps as f64;
            around.push(match side {
                0 => DVec2::new(along, -0.6),
                1 => DVec2::new(0.6, along),
                2 => DVec2::new(-along, 0.6),
                _ => DVec2::new(-0.6, -along),
            });
        }
    }
    around
}

/// How far the worst triangle of a mesh leaves the surface.
fn worst(surface: &Surface, params: &[DVec2], triangles: &[[u32; 3]]) -> f64 {
    triangles.iter().fold(0.0_f64, |far, &[a, b, c]| {
        far.max(surface.straying([params[a as usize], params[b as usize], params[c as usize]]))
    })
}

/// Twice the area a mesh covers in the parameters, which no conforming cut
/// may change.
fn covered(params: &[DVec2], triangles: &[[u32; 3]]) -> f64 {
    triangles.iter().fold(0.0, |total, &[a, b, c]| {
        let (a, b, c) = (params[a as usize], params[b as usize], params[c as usize]);
        total + (b - a).perp_dot(c - a)
    })
}

/// **However badly a wall is triangulated, it is cut back to the
/// sagitta** — which is what makes the sagitta a promise rather than
/// something the clipper is trusted to arrive at.
///
/// Six cells of a unit cylinder, two tall, its foot chorded every nine
/// tenths of a cell and its head every seven tenths — each inside a cell,
/// as everything [`chords`](crate::math::arc::chords) hands over is, and
/// out of step with each other, as a circle and an ellipse over one turn
/// are. Measured in cells the clipper already lays a strip over this and
/// nothing needs cutting, which is the ordinary case and is why the mesher
/// pays almost nothing for any of it. So the strip is thrown away and the
/// whole boundary fanned off one corner instead, which is what ear clipping
/// did before the cells were there: a triangle reaching the whole six cells
/// across, standing `1 − cos(0.268)` off the wall, or thirty-five times
/// what was asked for.
///
/// Afterwards: nothing strays, the boundary is untouched corner for corner,
/// every corner put in is on the cylinder, and the mesh covers exactly what
/// it covered — the last two together saying the cut was conforming rather
/// than merely finer.
#[test]
fn however_badly_a_wall_is_tiled_it_is_cut_back_to_the_sagitta() {
    let sagitta = 1e-3;
    let surface = Surface::Cylinder(Cylinder {
        axis: upright(),
        radius: 1.0,
    });
    let given = Patched::of(&surface, &wall(sagitta), sagitta).fanned();
    let params = given.params();
    let coarse = worst(&surface, &params, &given.fill.triangles);
    assert!(
        coarse > 30.0 * sagitta,
        "the fan this is given already follows the wall: {coarse}",
    );

    let refining = given.refined(&surface, sagitta);
    // To within a rounding, a cell being allowed to come out that much
    // wide — see [`Lattice::over`].
    let fine = worst(&surface, refining.params(), refining.triangles());
    assert!(
        predicate::touching(fine, predicate::slack(sagitta)),
        "a triangle strays {fine} of {sagitta}",
    );
    assert!(
        refining.triangles().len() > given.fill.triangles.len(),
        "nothing was cut",
    );

    assert_eq!(&refining.params()[..params.len()], &params[..]);
    assert_eq!(
        &refining.places()[..given.boundary.len()],
        &given.boundary[..]
    );
    for &at in &refining.places()[given.boundary.len()..] {
        assert!(surface.off(at) < 1e-12, "{at:?} is not on the wall");
    }
    let (was, now) = (
        covered(&params, &given.fill.triangles),
        covered(refining.params(), refining.triangles()),
    );
    assert!(
        (now - was).abs() < 1e-12,
        "the mesh covered {was} and now covers {now}",
    );

    // **A tenth of the sagitta is a grid three times as fine**, so the same
    // wall comes back with more of everything — which is what says the
    // sagitta is read at all rather than one mesh being handed back.
    let finer = Patched::of(&surface, &wall(sagitta / 10.0), sagitta / 10.0).fanned();
    let refining = finer.refined(&surface, sagitta / 10.0);
    assert!(refining.triangles().len() > given.fill.triangles.len());
    let fine = worst(&surface, refining.params(), refining.triangles());
    assert!(
        predicate::touching(fine, predicate::slack(sagitta / 10.0)),
        "a triangle strays {fine}",
    );
}

/// **A face wide both ways is given corners in the middle of it**, which
/// is the case no triangulation of a boundary alone can reach.
///
/// A square patch of a unit sphere, six tenths of a radian either way,
/// nineteen cells across each way at a sagitta of a thousandth. Its middle
/// stands a tenth of a radius proud of any triangle laid across it and
/// there is no corner out there to hang one on, so the corners have to be
/// put there — on the lines of the grid, one axis after the other.
///
/// Nothing the kernel builds is shaped like this: a cylinder and a cone
/// stand one cell tall however far they run, and a plane has no line at
/// all. It is written down because the *rule* does not know that, and a
/// rule that only ever runs on strips is a rule nobody has watched work.
#[test]
fn a_face_wide_in_both_directions_is_given_corners_in_the_middle() {
    let sagitta = 1e-3;
    let surface = Surface::Sphere(Sphere {
        axis: upright(),
        radius: 1.0,
    });
    let around = dome(20);
    let given = Patched::of(&surface, &around, sagitta);
    let params = given.params();
    let coarse = worst(&surface, &params, &given.fill.triangles);
    assert!(
        coarse > 0.09,
        "the patch this is given already follows the ball: {coarse}",
    );

    let refining = given.refined(&surface, sagitta);
    let fine = worst(&surface, refining.params(), refining.triangles());
    assert!(
        predicate::touching(fine, predicate::slack(sagitta)),
        "a triangle strays {fine} of {sagitta}",
    );

    assert_eq!(&refining.params()[..params.len()], &params[..]);
    assert_eq!(
        &refining.places()[..given.boundary.len()],
        &given.boundary[..]
    );
    for &at in &refining.places()[given.boundary.len()..] {
        assert!(surface.off(at) < 1e-12, "{at:?} is not on the ball");
    }
    let (was, now) = (
        covered(&params, &given.fill.triangles),
        covered(refining.params(), refining.triangles()),
    );
    assert!(
        (now - was).abs() < 1e-12,
        "the mesh covered {was} and now covers {now}",
    );
}

/// **A face whose boundary arrives chorded more coarsely than its own cells
/// still settles**, which every face on a sphere does.
///
/// The patch above, chorded at the widest chord the sagitta allows rather
/// than inside a cell. That is what [`chords`](crate::math::arc::chords)
/// hands over, and a sphere's cell is that chord over the square root of
/// two, so a run of the boundary reaches over more than one cell. It may
/// not be cut, and the triangle carrying it has no corner the grid can
/// offer to bring it inside one: the window such a corner would have to
/// fall in is narrower than a cell. Read as a thing to cut down, the rounds
/// step that corner from the line below the window to the line above it and
/// back, for ever.
///
/// So what is asserted is the promise rather than the counting that usually
/// stands in for it. No triangle strays further than a chord of the
/// boundary does, the boundary is untouched corner for corner, and the mesh
/// covers what it covered.
#[test]
fn a_face_chorded_coarser_than_its_cells_still_settles() {
    let sagitta = 1e-3;
    let surface = Surface::Sphere(Sphere {
        axis: upright(),
        radius: 1.0,
    });
    // Divided evenly into chords no wider than the sagitta allows, which is
    // what a walk hands over. Each comes out a hair under the widest, and
    // the cell is that widest over the square root of two.
    let widest = crate::math::arc::widest(1.0, sagitta);
    let steps = (1.2 / widest).ceil() as usize;
    let around = dome(steps);
    let given = Patched::of(&surface, &around, sagitta);
    let params = given.params();
    let reach = given.lattice.celled(DVec2::new(1.2 / steps as f64, 0.0)).x;
    assert!(
        reach > 1.0 && reach < 1.5,
        "the boundary is not chorded coarser than a cell: {reach}",
    );

    let refining = given.refined(&surface, sagitta);
    // A triangle carrying a chord of the boundary stands at least as far
    // off as that chord does, whatever is done to its other two sides, and
    // a chord this wide stands off by the sagitta itself. Twice it is what
    // a corner one cell away from both ends of such a chord costs.
    let fine = worst(&surface, refining.params(), refining.triangles());
    assert!(
        fine < 2.0 * sagitta,
        "a triangle strays {fine} of {sagitta}",
    );

    assert_eq!(&refining.params()[..params.len()], &params[..]);
    assert_eq!(
        &refining.places()[..given.boundary.len()],
        &given.boundary[..]
    );
    let (was, now) = (
        covered(&params, &given.fill.triangles),
        covered(refining.params(), refining.triangles()),
    );
    assert!(
        (now - was).abs() < 1e-12,
        "the mesh covered {was} and now covers {now}",
    );
}

/// **A face comes back the size its own cells are**, and stays that size as
/// the sagitta falls.
///
/// The one thing a cut that put in a single corner did not buy. A triangle
/// cut by one line comes back as three, so every halving of the face cost
/// three triangles where its cells ask for two, and the mesh settled at `w`
/// to the power of 1.585 for a face `w` cells across. Cut by every line it
/// crosses at once it comes back as `2w + 1` — see [`Lattice::crossings`].
///
/// **Asserted as a ratio that does not climb**, rather than as a count: the
/// count is a fact about one patch and the ratio is the property. Here the
/// ratio falls, the patch being bounded by eighty corners however fine the
/// sagitta — a face six cells across is mostly its own boundary and one
/// sixty cells across is not. Cut a line at a time it climbed instead, and
/// climbed without bound.
#[test]
fn a_face_comes_back_the_size_its_own_cells_are() {
    let surface = Surface::Sphere(Sphere {
        axis: upright(),
        radius: 1.0,
    });
    let around = dome(20);
    let mut coarsest = 0.0;
    let mut last = 0.0;
    for sagitta in [1e-2, 1e-3, 1e-4] {
        let given = Patched::of(&surface, &around, sagitta);
        let refining = given.refined(&surface, sagitta);
        let wide = given.lattice.celled(DVec2::new(1.2, 1.2));
        let apiece = refining.triangles().len() as f64 / (wide.x * wide.y);
        if coarsest == 0.0 {
            coarsest = apiece;
        }
        assert!(
            apiece <= coarsest,
            "{sagitta} came back {apiece} triangles a cell against {coarsest}",
        );
        last = apiece;
    }
    // Two is what a grid of cells holds outright. Twice that is the room a
    // triangulation cut across the grid rather than laid along it takes,
    // and it is measured rather than reasoned: 4.64 at the finest above.
    assert!(last < 5.0, "a face came back {last} triangles a cell");
}

/// A face on a plane is handed back exactly as it came, which is what keeps
/// every flat face in a drawing costing the mesher nothing.
#[test]
fn a_flat_face_is_handed_back_as_it_came() {
    let surface = Surface::Plane(Plane::GROUND);
    let around = [
        DVec2::new(-3.0, -3.0),
        DVec2::new(3.0, -3.0),
        DVec2::new(3.0, 3.0),
        DVec2::new(0.0, 1.0),
        DVec2::new(-3.0, 3.0),
    ];
    let given = Patched::of(&surface, &around, 1e-6);
    let refining = given.refined(&surface, 1e-6);
    assert_eq!(refining.params(), &given.params()[..]);
    assert_eq!(refining.triangles(), &given.fill.triangles[..]);
}
