//! The orientation cube: which way the camera is looking, and one click to a
//! named view.

use std::f32::consts::PI;

use aperture::Camera;
use glam::{Vec2, Vec3};
use palantir::{
    Align, AnimSlot, Animatable, Color, Configure, Drag, LineCap, LineJoin, Mesh, Panel,
    PolylineColors, Sense, Shape, Sizing, Ui, Vec2 as UiVec2, WidgetId,
};

use crate::hud::cube::facet::Facet;
use crate::intent::Intents;
use crate::intent::change::Change;
use crate::look::Theme;
use crate::scene_view::ORBIT_RATE;

mod facet;
mod letters;

/// How much of a face a name is set across, as a share of the face's own width.
///
/// **Sized against the widest of the six, so all six are set at one height.** A
/// cube whose lettering grew as it turned would read as six cubes rather than
/// as one being looked at from six sides. Short of the whole face, because a
/// word pressed against the bevel would read as one that had not fitted.
const NAMING: f32 = 0.80;

/// How far a face has to be turned toward the eye before its name is written on
/// it, as the cosine of the angle between them.
///
/// **A word is not worth the room until it can be read.** A face near enough to
/// edge-on projects its letters into a smear a stroke or two wide, which is
/// noise across the piece next to it. At this much the three faces of a corner
/// view all carry their names — a corner shows each at `0.577` — and a face
/// swinging away loses its name well before it loses its outline.
const READS: f32 = 0.3;

/// Where the cube's light comes from, in the world.
///
/// **Fixed in the world rather than to the screen**, which is what makes the
/// cube read as an object being turned rather than as three flat panels
/// swapping shades: the face that was bright stays the bright one as it comes
/// round.
///
/// Authored at unit length, so shading a piece is one normalize and one dot
/// rather than two normalizes.
const LIGHT: Vec3 = Vec3::new(0.3235, 0.8896, 0.3538);

/// The row the camera's bearing is eased in.
const TURNING: AnimSlot = AnimSlot::new("turning");

/// How near the target the turn has to get before it is over, in radians.
const ARRIVED: f32 = 1e-3;

/// Where the camera is pointed, as the two angles that say it.
///
/// The pair rather than a direction, because it is what the camera holds and
/// what an interpolation has to run in: eased as a direction, a turn would
/// cut through the middle of the cube instead of going round it.
#[derive(Debug, Clone, Copy, PartialEq, Animatable)]
struct Bearing {
    yaw: f32,
    pitch: f32,
}

impl Bearing {
    /// The same bearing, wound to within half a turn of `near`.
    ///
    /// **What keeps a turn going the short way round.** Yaw has no bounds, so a
    /// camera at 350° asked for 10° eases through 340° of model rather than
    /// through 20° — and that reads as a bug rather than as a turn.
    fn near(self, near: f32) -> Self {
        const TURN: f32 = 2.0 * PI;
        // Wound in one step rather than by subtracting turns until it lands: a
        // yaw that arrived as a NaN would never land, and a loop is the one
        // shape of that mistake that hangs rather than draws wrong.
        let round = (self.yaw - near + PI).rem_euclid(TURN) - PI;
        Self {
            yaw: near + round,
            ..self
        }
    }
}

impl From<Camera> for Bearing {
    fn from(camera: Camera) -> Self {
        Self {
            yaw: camera.yaw,
            pitch: camera.pitch,
        }
    }
}

impl From<Vec3> for Bearing {
    /// The bearing that looks from `direction`, which runs from what is being
    /// looked at toward the eye — the sense [`Camera::eye`] builds its offset
    /// in.
    fn from(direction: Vec3) -> Self {
        let direction = direction.normalize();
        Self {
            yaw: direction.x.atan2(direction.z),
            pitch: direction.y.asin(),
        }
    }
}

/// The gizmo, and the view it is on its way to.
///
/// **The target is the whole of its state.** Where the camera *is* belongs to
/// the document, and the cube reads it every frame; what is kept here is one
/// gesture's worth of intent — the view asked for, until the camera reaches it.
#[derive(Debug, Default)]
pub(super) struct Cube {
    /// The three lit faces and whatever wedge the pointer is over, as one
    /// shape.
    ///
    /// **One mesh rather than a triangle apiece**, for two reasons. A quad cut
    /// into two triangles antialiases along the cut, so the seam shows as a
    /// hairline across every face; a mesh has no seam because it has no second
    /// shape to blend against. And it is cleared and refilled rather than
    /// rebuilt, so the buffers it needs are asked for once and the record pass
    /// stays at zero allocations.
    mesh: Mesh,
    /// One piece's outline, in the world and then laid into the box.
    ///
    /// Kept for their room rather than their contents, like everything else the
    /// overlay parks: twenty-six outlines are built every frame, and a pair of
    /// lists asked for at each of them is twenty-six trips to the heap a frame.
    ring: Vec<Vec3>,
    flat: Vec<Vec2>,
    /// One stroke of one letter, laid into the box, for the same reason.
    stroke: Vec<UiVec2>,
    turning_to: Option<Bearing>,
    /// How far the drag under way has travelled, so the next frame can ask for
    /// the *step* rather than the whole of it again. Palantir reports a drag as
    /// its total, which is what makes it safe to re-read on a settling frame.
    travel: UiVec2,
}

impl Cube {
    /// Draw it, and ask for whatever a press on it named.
    ///
    /// **Shows and does not act**, like everything else on the overlay: the
    /// eased bearing goes out as a [`Change::Aim`] rather than being written to
    /// a camera. Landing twice is harmless because an aim is absolute.
    pub(super) fn show(
        &mut self,
        ui: &mut Ui,
        id: WidgetId,
        theme: &Theme,
        camera: Camera,
        intents: &mut Intents,
    ) {
        let state = ui.response_for(id);
        let seen = Seen::of(theme, camera);
        // Borrowed apiece, because the lettering runs inside the panel below
        // and the outlines are laid before it: they are different fields, and
        // nothing here reads one while writing another.
        let Self {
            mesh,
            ring,
            flat,
            stroke,
            ..
        } = self;
        // Read before the shapes, so the piece under the pointer is lit on the
        // frame it is under it rather than the frame after.
        let under = state
            .pointer_local
            .filter(|_| state.hovered)
            .and_then(|p| seen.facet_at(seen.local(p), ring, flat));
        solid(mesh, &seen, ring, flat, under);
        let mesh = &*mesh;
        // A canvas rather than a stack, because the lettering is *placed*: a
        // stack puts every child at its own top-left, so three names would land
        // on top of one another in the corner.
        Panel::canvas()
            .id(id)
            .size((
                Sizing::fixed(theme.chrome.cube),
                Sizing::fixed(theme.chrome.cube),
            ))
            .align(Align::CENTER)
            .sense(Sense::CLICK | Sense::DRAG)
            .show(ui, |ui| {
                ui.add_shape(Shape::mesh(mesh));
                edges(ui, &seen, ring, flat, stroke, under);
                // Away while the cube itself is under the pointer, so they do
                // not compete with the thing they sit beside.
                if under.is_none() {
                    arrows(ui, theme);
                }
                names(ui, &seen, stroke, under);
            });
        if let Some(facet) = under.filter(|_| state.left.clicked()) {
            self.turning_to = Some(Bearing::from(facet.out()).near(camera.yaw));
        }
        self.drag(state.left.drag, intents);
        self.turn(ui, id, theme, camera, intents);
    }

    /// Turn it by hand.
    ///
    /// The same gesture the drawing itself takes and so the same intent, at the
    /// same rate — see [`ORBIT_RATE`]. A drag also gives up whatever view was
    /// being turned to: taking hold of the cube is saying where to look more
    /// directly than a press did.
    fn drag(&mut self, drag: Drag, intents: &mut Intents) {
        let (Drag::Started { delta } | Drag::Active { delta }) = drag else {
            self.travel = UiVec2::ZERO;
            return;
        };
        let step = delta - self.travel;
        self.travel = delta;
        self.turning_to = None;
        // Dragging right turns the model right, which means orbiting the camera
        // the other way.
        intents.push(Change::Orbit {
            yaw: -step.x * ORBIT_RATE,
            pitch: step.y * ORBIT_RATE,
        });
    }

    /// Carry the camera toward whatever view was asked for.
    ///
    /// **Asked for every frame, not only while a turn is running.** Palantir
    /// keeps the eased value against the widget's id and starts a row at the
    /// first target it is shown — so a frame that skipped this would seed the
    /// row at the *destination* and the next turn would cut rather than run.
    fn turn(
        &mut self,
        ui: &mut Ui,
        id: WidgetId,
        theme: &Theme,
        camera: Camera,
        intents: &mut Intents,
    ) {
        let want = self.turning_to.unwrap_or_else(|| camera.into());
        let now = ui.animate(id, TURNING, want, Some(theme.motion.turn));
        if self.turning_to.is_none() {
            return;
        }
        intents.push(Change::Aim {
            yaw: now.yaw,
            pitch: now.pitch,
        });
        if (now.yaw - want.yaw).abs() < ARRIVED && (now.pitch - want.pitch).abs() < ARRIVED {
            self.turning_to = None;
        }
    }
}

/// How the cube is being looked at this frame.
///
/// **Built once and handed down.** Every projection below wants the same two
/// facts — where the eye stands and how big the box is — and the screen basis
/// they imply costs two normalizes. A frame projects forty-odd corners and label
/// centres, so working it out at each of them would be doing once-a-frame
/// arithmetic forty times over.
#[derive(Debug)]
struct Seen<'a> {
    theme: &'a Theme,
    /// Where the eye stands, as a direction from what it is looking at.
    ///
    /// **Taken off the camera's own answer rather than worked out again.** Which
    /// way the offset runs from a yaw and a pitch is aperture's convention, and a
    /// second copy of it here would be a cube that went on drawing the old one
    /// the day it changed. What is dropped is the distance, which says nothing
    /// about which way round the cube reads.
    eye: Vec3,
    /// The screen right and up the camera implies, with the world's own up.
    ///
    /// The same basis the renderer builds, restated here rather than borrowed:
    /// the cube is drawn flat and orthographic whatever the view is doing, so it
    /// wants the camera's *orientation* and nothing else of it — no projection,
    /// no distance, no viewport.
    right: Vec3,
    up: Vec3,
}

impl<'a> Seen<'a> {
    fn of(theme: &'a Theme, camera: Camera) -> Self {
        let eye = (camera.eye() - camera.target).normalize_or(Vec3::Y);
        let right = Vec3::new(eye.z, 0.0, -eye.x).normalize_or(Vec3::X);
        Self {
            theme,
            eye,
            right,
            up: right.cross(-eye).normalize_or(Vec3::Y),
        }
    }

    /// Whether a piece of the solid is turned toward the eye at all.
    fn shows(&self, facet: Facet) -> bool {
        facet.out().dot(self.eye) > 0.0
    }

    /// Where a world direction lands in the box, measured from its middle.
    fn flat(&self, direction: Vec3) -> Vec2 {
        // Negated down the screen, because a box counts its rows from the top
        // and the world counts its height from the ground.
        Vec2::new(direction.dot(self.right), -direction.dot(self.up))
            * self.theme.chrome.cube_scale()
    }

    /// The same, in the box's own coordinates — what a shape is placed in and
    /// where the pointer arrives.
    fn at(&self, direction: Vec3) -> UiVec2 {
        self.boxed(self.flat(direction))
    }

    /// A point already laid flat, moved into the box's own corner.
    fn boxed(&self, flat: Vec2) -> UiVec2 {
        let middle = self.theme.chrome.cube * 0.5;
        UiVec2::new(flat.x + middle, flat.y + middle)
    }

    /// The pointer, measured from the middle of the box like everything else.
    fn local(&self, pointer: UiVec2) -> Vec2 {
        let middle = self.theme.chrome.cube * 0.5;
        Vec2::new(pointer.x - middle, pointer.y - middle)
    }

    /// The outline of `facet`, laid flat into `flat`.
    fn outline(&self, facet: Facet, ring: &mut Vec<Vec3>, flat: &mut Vec<Vec2>) {
        facet.ring(self.theme.chrome.cube_chamfer, ring);
        flat.clear();
        flat.extend(ring.iter().map(|&at| self.flat(at)));
    }

    /// What the point `at` in the box is over, if anything.
    ///
    /// **Asked of the outlines that were drawn, and of nothing else.** The
    /// pieces turned toward the eye tile what you can see of a convex solid
    /// exactly — they cannot overlap and they leave no gap — so the first one
    /// that contains the point is the only one that does, and a point in none
    /// of them is a point beside the gizmo.
    ///
    /// That is the whole of what cutting the edges and corners off bought. A
    /// plain cube has six outlines and fourteen views, so the other eight had
    /// to be read off bands inside a face that nothing on screen drew; here
    /// every view you can ask for is a piece you can see.
    fn facet_at(&self, at: Vec2, ring: &mut Vec<Vec3>, flat: &mut Vec<Vec2>) -> Option<Facet> {
        facet::EVERY.into_iter().find(|&facet| {
            self.shows(facet) && {
                self.outline(facet, ring, flat);
                inside(flat, at)
            }
        })
    }
}

/// Whether `at` is inside the convex outline `ring`.
///
/// By whether every edge turns the same way about it, which is the test a
/// *convex* outline admits and a general one does not — and every piece of this
/// solid is convex by construction. An edge the point sits exactly on counts
/// for either, so a press on the join between two pieces lands on one of them
/// rather than on neither.
fn inside(ring: &[Vec2], at: Vec2) -> bool {
    let mut side = 0.0;
    for from in 0..ring.len() {
        let edge = ring[(from + 1) % ring.len()] - ring[from];
        let reach = at - ring[from];
        let turn = edge.x * reach.y - edge.y * reach.x;
        if turn * side < 0.0 {
            return false;
        }
        if turn != 0.0 {
            side = turn;
        }
    }
    true
}

/// Every piece of the solid turned toward the eye, filled and lit, into `mesh`.
///
/// Nothing is sorted, because pieces pointing at the eye cannot overlap each
/// other — the same fact the picking above rests on.
fn solid(
    mesh: &mut Mesh,
    seen: &Seen<'_>,
    ring: &mut Vec<Vec3>,
    flat: &mut Vec<Vec2>,
    under: Option<Facet>,
) {
    mesh.clear();
    for facet in facet::EVERY {
        if !seen.shows(facet) {
            continue;
        }
        seen.outline(facet, ring, flat);
        fan(
            mesh,
            seen,
            flat,
            lit(seen.theme, facet, under == Some(facet)),
        );
    }
}

/// Every piece run round with a stroke of its own colour.
///
/// **What makes the solid's edges smooth.** A mesh is rasterized by whether a
/// pixel's centre falls inside a triangle and by nothing else, so every
/// boundary it draws is a staircase — which on a gizmo of two dozen small
/// facets is most of what you see. A stroke is drawn with analytic coverage.
/// Run round each piece in the very shade that piece is filled with, it changes
/// no colour at all and feathers every boundary: the silhouette against the
/// drawing, and the join between one facet and the next.
///
/// The ring is closed by repeating where it began, because a stroke has two
/// ends and an outline has none.
fn edges(
    ui: &mut Ui,
    seen: &Seen<'_>,
    ring: &mut Vec<Vec3>,
    flat: &mut Vec<Vec2>,
    stroke: &mut Vec<UiVec2>,
    under: Option<Facet>,
) {
    for facet in facet::EVERY {
        if !seen.shows(facet) {
            continue;
        }
        seen.outline(facet, ring, flat);
        stroke.clear();
        stroke.extend(flat.iter().map(|&at| seen.boxed(at)));
        stroke.push(stroke[0]);
        ui.add_shape(
            Shape::polyline(
                stroke,
                PolylineColors::Single(lit(seen.theme, facet, under == Some(facet))),
                1.0,
            )
            .join(LineJoin::Miter),
        );
    }
}

/// One outline, as the fan of triangles it is — sharing their vertices, so no
/// cut inside it is a boundary anything antialiases against.
fn fan(mesh: &mut Mesh, seen: &Seen<'_>, ring: &[Vec2], color: Color) {
    let [first, second] = [0, 1].map(|at| mesh.vertex(seen.boxed(ring[at]), color));
    let mut last = second;
    for &at in &ring[2..] {
        let next = mesh.vertex(seen.boxed(at), color);
        mesh.triangle(first, last, next);
        last = next;
    }
}

/// What a piece turned this way is filled with.
///
/// Lit in the world rather than picked off a table, so a piece keeps its shade
/// as the cube turns — see [`LIGHT`]. It is also the whole of what makes the
/// chamfers read: a bevel faces halfway between its two neighbours, so the
/// light gives it a shade of its own and the edge between them becomes a facet
/// rather than a line.
fn lit(theme: &Theme, facet: Facet, under: bool) -> Color {
    let chrome = &theme.chrome;
    if under {
        return chrome.chip_held;
    }
    let light = facet.normal().dot(LIGHT).max(0.0);
    chrome.cube_low.lerp(chrome.cube_high, light)
}

/// The name of every face turned far enough toward the eye to read, written in
/// the plane of the face itself.
///
/// **In the face and not on the screen**, which is the one thing about the
/// lettering worth saying. A run of shaped text is a rectangle of pixels the
/// compositor sets square to the screen, and a word set square on a face that
/// is not reads as a sticker on a photograph. These are strokes — see
/// [`letters`] — so every point of every letter goes through the same
/// projection the outline under it did, and the word leans with the face.
fn names(ui: &mut Ui, seen: &Seen<'_>, stroke: &mut Vec<UiVec2>, under: Option<Facet>) {
    let chrome = &seen.theme.chrome;
    // One height for all six — see [`NAMING`].
    let widest = facet::SIDES
        .into_iter()
        .map(|side| letters::width(side.name))
        .fold(0.0, f32::max);
    // The face is what is left of the cube's own after the cut, and the word
    // takes a share of that.
    let em = (1.0 - chrome.cube_chamfer) * 2.0 * NAMING / widest;
    for side in facet::SIDES {
        let facet = side.facet();
        if facet.normal().dot(seen.eye) < READS {
            continue;
        }
        // The pill's own dark where the face under it has gone light, so the
        // word survives being pointed at rather than disappearing into it.
        let ink = match under == Some(facet) {
            true => chrome.on_held,
            false => chrome.ink_lit,
        };
        let mut pen = em * letters::width(side.name) * -0.5;
        for letter in side.name.bytes() {
            for run in letters::strokes(letter) {
                stroke.clear();
                stroke.extend(run.iter().map(|point| {
                    let across = pen + point.x * letters::NARROW * em;
                    let up = (point.y - 0.5) * em;
                    seen.at(facet.out() + side.u * across + side.v * up)
                }));
                ui.add_shape(
                    Shape::polyline(stroke, PolylineColors::Single(ink), chrome.cube_letter)
                        .cap(LineCap::Round)
                        .join(LineJoin::Round),
                );
            }
            pen += (letters::NARROW + letters::TRACKING) * em;
        }
    }
}

/// The two arrows that step the view a quarter turn to either side.
///
/// **A step in yaw, not a roll.** A cube in a modeller that carries one usually
/// rolls the view about the axis it is looking down; this camera holds a yaw
/// and a pitch and no roll at all, and giving it one reaches the projection,
/// the ray cast and which way up a name is written. A quarter turn is the
/// useful nine tenths of what the arrows are for.
///
fn arrows(ui: &mut Ui, theme: &Theme) {
    const RISE: f32 = 4.5;
    const RUN: f32 = 5.0;
    let middle = theme.chrome.cube * 0.5;
    let reach = middle - 1.0;
    for side in [-1.0f32, 1.0] {
        let tip = middle + side * reach;
        let base = tip - side * RUN;
        ui.add_shape(
            Shape::triangle(
                UiVec2::new(tip, middle),
                UiVec2::new(base, middle - RISE),
                UiVec2::new(base, middle + RISE),
            )
            .fill(theme.chrome.ink),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::FRAC_PI_2;

    use super::*;
    use crate::look::Theme;
    use aperture::Camera;
    use glam::Vec2;

    /// Where a press at `at` in the cube's box would point the camera, or
    /// `None` where that point is off the solid.
    ///
    /// The whole of what these tests need: the drawing is judged by eye and the
    /// arithmetic is judged here, so a piece that stopped resolving to its own
    /// view fails without a frame being rendered.
    fn aimed(camera: Camera, at: Vec2) -> Option<Bearing> {
        let (mut ring, mut flat) = (Vec::new(), Vec::new());
        let facet = Seen::of(&Theme::default(), camera).facet_at(at, &mut ring, &mut flat)?;
        Some(Bearing::from(facet.out()))
    }

    /// The bearing that looks square on at the face called `name`.
    fn facing(name: &str) -> Bearing {
        let side = facet::SIDES
            .iter()
            .find(|side| side.name == name)
            .expect("the cube has a face by that name");
        Bearing::from(side.facet().out())
    }

    /// A camera looking from `direction`, and nothing else about it that
    /// matters here.
    fn looking(direction: Vec3) -> Camera {
        let Bearing { yaw, pitch } = direction.into();
        Camera {
            yaw,
            pitch,
            ..Camera::default()
        }
    }

    /// The piece looking out along `out`.
    fn piece(out: Vec3) -> Facet {
        facet::EVERY
            .into_iter()
            .find(|facet| facet.out() == out)
            .expect("the solid has a piece looking that way")
    }

    /// Each named view is the direction its name says, worked out by hand: the
    /// eye offset runs `(sin yaw · cos pitch, sin pitch, cos yaw · cos pitch)`,
    /// so looking at the front is `yaw = 0`, at the right is a quarter turn,
    /// and at the top is a quarter turn of pitch.
    #[test]
    fn every_named_view_is_the_direction_its_name_says() {
        for (name, yaw, pitch) in [
            ("FRONT", 0.0, 0.0),
            ("BACK", PI, 0.0),
            ("RIGHT", FRAC_PI_2, 0.0),
            ("LEFT", -FRAC_PI_2, 0.0),
            ("TOP", 0.0, FRAC_PI_2),
            ("BOTTOM", 0.0, -FRAC_PI_2),
        ] {
            let got = facing(name);
            assert!(
                (got.pitch - pitch).abs() < 1e-5,
                "{name} pitches to {} rather than {pitch}",
                got.pitch
            );
            // Yaw says nothing at the poles, where every heading looks straight
            // down the same axis — so it is only checked where it means
            // something.
            if pitch == 0.0 {
                assert!(
                    (got.yaw - yaw).abs() < 1e-5,
                    "{name} yaws to {} rather than {yaw}",
                    got.yaw
                );
            }
        }
    }

    /// The middle of the box is the face nearest the eye, whichever way the
    /// cube has been turned — so a click there goes straight on to it.
    #[test]
    fn the_middle_of_the_cube_aims_straight_at_the_face_under_it() {
        let front = looking(Vec3::Z);
        let aim = aimed(front, Vec2::ZERO).expect("the middle of the box is on the cube");
        assert!((aim.yaw).abs() < 1e-5 && aim.pitch.abs() < 1e-5);

        // Turned to look down on it, the middle is the top face instead. Same
        // point in the box, a different view, which is the whole claim.
        let above = looking(Vec3::Y);
        let aim = aimed(above, Vec2::ZERO).expect("the middle of the box is on the cube");
        assert!(
            (aim.pitch - FRAC_PI_2).abs() < 1e-5,
            "looking down, the middle of the cube is not the top: {aim:?}"
        );
    }

    /// **Every piece the eye can see answers for its own outline, and for no
    /// other.**
    ///
    /// The one claim the whole gizmo rests on, and three things have to hold
    /// together for it: the outline a piece is built with has to be the piece,
    /// the pieces turned toward the eye have to tile what you can see of the
    /// solid without overlapping, and the test for a point inside one has to
    /// agree with both. A press at the middle of a piece landing on a
    /// *neighbour* is what any of the three getting it wrong looks like.
    ///
    /// From a bearing square to nothing, so no piece is edge-on and every one
    /// that shows has an outline with room in it.
    #[test]
    fn every_piece_in_view_answers_for_its_own_outline() {
        let theme = Theme::default();
        let camera = looking(Vec3::new(0.9, 0.55, 1.3));
        let seen = Seen::of(&theme, camera);
        let (mut ring, mut flat) = (Vec::new(), Vec::new());
        let (mut probing, mut probed) = (Vec::new(), Vec::new());
        let mut seen_count = 0;
        for facet in facet::EVERY {
            if !seen.shows(facet) {
                continue;
            }
            seen_count += 1;
            seen.outline(facet, &mut probing, &mut probed);
            let middle = probed.iter().sum::<Vec2>() / probed.len() as f32;
            let hit = seen.facet_at(middle, &mut ring, &mut flat);
            assert_eq!(hit, Some(facet), "the middle of {facet:?} landed elsewhere");
        }
        // Three faces, six bevels and one corner is what a corner-ish view of a
        // chamfered cube shows: every piece whose direction has a positive
        // component in common with the eye and none against it.
        assert_eq!(seen_count, 13, "a general view shows {seen_count} pieces");
    }

    /// A press on the bevel between two faces aims half-way between them.
    ///
    /// **The view the old shape could not offer.** A plain cube draws six
    /// outlines, so the twelve half-way views had to be read off a band inside
    /// a face — invisible, and the fiddliest thing on the gizmo to hit. Cut,
    /// the bevel is a piece you can see, and its middle is its own view.
    #[test]
    fn a_press_on_a_bevel_aims_half_way_between_the_two_faces_it_joins() {
        let theme = Theme::default();
        let camera = looking(Vec3::new(0.9, 0.55, 1.3));
        let seen = Seen::of(&theme, camera);
        let (mut ring, mut flat) = (Vec::new(), Vec::new());
        // The bevel joining FRONT and RIGHT, at the middle of its own outline.
        let bevel = piece(Vec3::new(1.0, 0.0, 1.0));
        seen.outline(bevel, &mut ring, &mut flat);
        let middle = flat.iter().sum::<Vec2>() / flat.len() as f32;
        let aim = aimed(camera, middle).expect("the bevel is in view");
        // Half-way between a yaw of zero and a quarter turn, and level: the
        // eye offset runs `(sin yaw, ·, cos yaw)`, so `(1, 0, 1)` is an eighth
        // of a turn round and no pitch at all.
        assert!(
            (aim.yaw - FRAC_PI_2 * 0.5).abs() < 1e-5 && aim.pitch.abs() < 1e-5,
            "the bevel between FRONT and RIGHT aims at {aim:?}",
        );
    }

    /// Off the cube is nothing at all, rather than the nearest piece.
    #[test]
    fn a_press_beside_the_cube_names_no_view() {
        let camera = looking(Vec3::Z);
        let off = Theme::default().chrome.cube;
        assert!(aimed(camera, Vec2::new(off, off)).is_none());
    }

    /// A turn takes the short way round, whichever side of the wrap the two
    /// ends fall.
    #[test]
    fn a_turn_winds_to_the_near_side_of_the_wrap() {
        let target = Bearing {
            yaw: 0.1,
            pitch: 0.0,
        };
        // From just under a full turn, the near reading of 0.1 is 0.1 + 2π —
        // twenty-odd degrees on, rather than three hundred and forty back.
        let wound = target.near(6.2);
        assert!(
            (wound.yaw - (0.1 + 2.0 * PI)).abs() < 1e-5,
            "{} is the long way round from 6.2",
            wound.yaw
        );
        // And it leaves a target already near alone.
        assert!((target.near(0.0).yaw - 0.1).abs() < 1e-5);
    }
}
