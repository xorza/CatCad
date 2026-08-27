//! The orientation cube: which way the camera is looking, and one click to a
//! named view.

use std::f32::consts::PI;

use aperture::Camera;
use glam::{Vec2, Vec3};
use palantir::{
    Align, AnimSlot, Animatable, Color, Configure, Drag, Mesh, Panel, Sense, Shape, Sizing, Text,
    TextStyle, Ui, Vec2 as UiVec2, WidgetId,
};

use crate::intent::Intents;
use crate::intent::change::Change;
use crate::look::Theme;
use crate::scene_view::ORBIT_RATE;

/// Where a click inside a face stops being about the face and starts being
/// about a corner, as a share of the half-face.
///
/// Half, so the middle of a face is a face and the outer quarters are corners.
const CORNER_BAND: f32 = 0.5;

/// Where the cube's light comes from, in the world.
///
/// **Fixed in the world rather than to the screen**, which is what makes the
/// cube read as an object being turned rather than as three flat panels
/// swapping shades: the face that was bright stays the bright one as it comes
/// round.
///
/// Authored at unit length, so shading a face is one dot product rather than a
/// normalize per face per frame.
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

/// One face of the cube, and the two axes that span it.
///
/// The axes travel with the normal rather than being worked out from it,
/// because they are what a click is read in: a point inside a face resolves to
/// a share along each, and which corner that names follows from their signs.
#[derive(Debug, Clone, Copy)]
struct Side {
    normal: Vec3,
    u: Vec3,
    v: Vec3,
    name: &'static str,
}

/// Every face, named the way a drawing is read.
///
/// `TOP`, `FRONT` and `RIGHT` are what every CAD program writes on a cube, and
/// they stay that vocabulary here even though the recipe two corners away calls
/// the world's three planes `Ground`, `Front` and `Side`. The two are different
/// things: one is a direction you look from, the other is a sheet you draw on.
const SIDES: [Side; 6] = [
    Side {
        normal: Vec3::Y,
        u: Vec3::X,
        v: Vec3::NEG_Z,
        name: "TOP",
    },
    Side {
        normal: Vec3::NEG_Y,
        u: Vec3::X,
        v: Vec3::Z,
        name: "BOTTOM",
    },
    Side {
        normal: Vec3::Z,
        u: Vec3::X,
        v: Vec3::Y,
        name: "FRONT",
    },
    Side {
        normal: Vec3::NEG_Z,
        u: Vec3::NEG_X,
        v: Vec3::Y,
        name: "BACK",
    },
    Side {
        normal: Vec3::X,
        u: Vec3::NEG_Z,
        v: Vec3::Y,
        name: "RIGHT",
    },
    Side {
        normal: Vec3::NEG_X,
        u: Vec3::Z,
        v: Vec3::Y,
        name: "LEFT",
    },
];

/// What a press on the cube would take you to.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Zone {
    /// Straight on to one face.
    Face(Vec3),
    /// The three-quarter view from one corner.
    Corner(Vec3),
}

impl Zone {
    fn direction(self) -> Vec3 {
        match self {
            Self::Face(d) | Self::Corner(d) => d,
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
        // Read before the shapes, so the zone under the pointer is lit on the
        // frame it is under it rather than the frame after.
        let under = state
            .pointer_local
            .filter(|_| state.hovered)
            .and_then(|p| seen.zone(seen.local(p)));
        faces(&mut self.mesh, &seen, under);
        let mesh = &self.mesh;
        // A canvas rather than a stack, because the face names are *placed*: a
        // stack puts every child at its own top-left, so three labels would
        // land on top of one another in the corner.
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
                // Away while the cube itself is under the pointer, so they do
                // not compete with the thing they sit beside.
                if under.is_none() {
                    arrows(ui, theme);
                }
                names(ui, &seen);
            });
        if let Some(zone) = under.filter(|_| state.left.clicked()) {
            self.turning_to = Some(Bearing::from(zone.direction()).near(camera.yaw));
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

    /// Whether a face is turned toward the eye at all.
    fn shows(&self, side: Side) -> bool {
        side.normal.dot(self.eye) > 0.0
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
        let flat = self.flat(direction);
        let middle = self.theme.chrome.cube * 0.5;
        UiVec2::new(flat.x + middle, flat.y + middle)
    }

    /// The pointer, measured from the middle of the box like everything else.
    fn local(&self, pointer: UiVec2) -> Vec2 {
        let middle = self.theme.chrome.cube * 0.5;
        Vec2::new(pointer.x - middle, pointer.y - middle)
    }

    /// What the point `at` in the box is over, if anything.
    ///
    /// Read against the same three faces that were drawn, in the same
    /// projection: a point inside one resolves to a share along each of its
    /// axes, and how far out those shares are is what tells a face from a
    /// corner.
    ///
    /// **An edge counts as its face.** The twelve half-way views are the least
    /// reached for and the fiddliest to hit, and adding them is one more band in
    /// this test rather than a second test — the case where exactly one of the
    /// two shares is out past [`CORNER_BAND`].
    fn zone(&self, at: Vec2) -> Option<Zone> {
        for side in SIDES {
            if !self.shows(side) {
                continue;
            }
            let middle = self.flat(side.normal);
            let (u, v) = (self.flat(side.u), self.flat(side.v));
            let det = u.x * v.y - u.y * v.x;
            // Edge-on, so it has no inside to be within.
            if det.abs() < f32::EPSILON {
                continue;
            }
            let from = at - middle;
            let s = (from.x * v.y - from.y * v.x) / det;
            let t = (u.x * from.y - u.y * from.x) / det;
            if s.abs() > 1.0 || t.abs() > 1.0 {
                continue;
            }
            return Some(if s.abs() >= CORNER_BAND && t.abs() >= CORNER_BAND {
                Zone::Corner(side.normal + side.u * s.signum() + side.v * t.signum())
            } else {
                Zone::Face(side.normal)
            });
        }
        None
    }
}

/// Every face turned toward the eye, filled and lit, into `mesh`.
///
/// Three of them, always: a cube shows three faces from a corner and two from
/// an edge, and the third is then a sliver of no width. Nothing is sorted,
/// because faces pointing at the eye cannot overlap each other.
fn faces(mesh: &mut Mesh, seen: &Seen<'_>, under: Option<Zone>) {
    mesh.clear();
    for side in SIDES {
        if !seen.shows(side) {
            continue;
        }
        let shade = lit(
            seen.theme,
            side.normal,
            under == Some(Zone::Face(side.normal)),
        );
        let at = |s: f32, t: f32| seen.at(side.normal + side.u * s + side.v * t);
        quad(
            mesh,
            [at(-1.0, -1.0), at(1.0, -1.0), at(1.0, 1.0), at(-1.0, 1.0)],
            shade,
        );
        if let Some(Zone::Corner(corner)) = under {
            highlight(mesh, seen.theme, side, corner, at);
        }
    }
}

/// One quad, as the two triangles a mesh is made of — sharing their vertices,
/// so the cut between them is not a boundary anything antialiases against.
fn quad(mesh: &mut Mesh, [a, b, c, d]: [UiVec2; 4], color: Color) {
    let [a, b, c, d] = [a, b, c, d].map(|at| mesh.vertex(at, color));
    mesh.triangle(a, b, c);
    mesh.triangle(a, c, d);
}

/// What a face turned this way is filled with.
///
/// Lit in the world rather than picked off a table of three, so a face keeps
/// its shade as the cube turns — see [`LIGHT`].
fn lit(theme: &Theme, normal: Vec3, under: bool) -> Color {
    let chrome = &theme.chrome;
    if under {
        return chrome.chip_held;
    }
    let light = normal.dot(LIGHT).max(0.0);
    chrome.cube_low.lerp(chrome.cube_high, light)
}

/// The wedge of one face that belongs to `corner`, where the pointer is on one.
///
/// Drawn per face rather than as one shape, because a corner is where three
/// faces meet and what the viewer can see of it is however many of them are
/// turned this way.
fn highlight(
    mesh: &mut Mesh,
    theme: &Theme,
    side: Side,
    corner: Vec3,
    at: impl Fn(f32, f32) -> UiVec2,
) {
    let (s, t) = (corner.dot(side.u), corner.dot(side.v));
    // A corner off this face has no wedge on it: two of its three axes agree
    // with the face's, and the third is the face's own normal.
    if corner.dot(side.normal) <= 0.0 || s == 0.0 || t == 0.0 {
        return;
    }
    let [a, b, c] =
        [at(s, t), at(s, 0.0), at(0.0, t)].map(|p| mesh.vertex(p, theme.chrome.chip_held));
    mesh.triangle(a, b, c);
}

/// The name of every face with room on screen to carry one.
///
/// **Measured against the face's projection, not against how square-on it is.**
/// A name is set horizontally, so what decides whether it fits is how wide and
/// tall the face comes out — and those are different questions. The top of a
/// cube tipped a little toward the viewer is a shallow band: barely any of it
/// by area, and still wider than the word `TOP`.
fn names(ui: &mut Ui, seen: &Seen<'_>) {
    // The box the longest name needs, and no more: it is the bound a face is
    // measured against, so a box wider than the word it holds drops names off
    // faces that had room for them.
    const RUN: f32 = 30.0;
    const RISE: f32 = 11.0;
    // Light, because a face is dark however the light falls on it: the two
    // shades it lerps between are both under half.
    let style = TextStyle {
        color: seen.theme.chrome.ink_lit,
        font_size_px: 8.0,
        ..TextStyle::default()
    };
    for side in SIDES {
        if !seen.shows(side) {
            continue;
        }
        let (u, v) = (seen.flat(side.u), seen.flat(side.v));
        // The projected face is a parallelogram spanned by these two, so its
        // reach along each screen axis is the sum of what they contribute to it.
        let across = (u.x.abs() + v.x.abs()) * 2.0;
        let down = (u.y.abs() + v.y.abs()) * 2.0;
        if across < RUN || down < RISE {
            continue;
        }
        let middle = seen.at(side.normal);
        Panel::zstack()
            .id_salt(side.name)
            .position(UiVec2::new(middle.x - RUN * 0.5, middle.y - RISE * 0.5))
            .size((Sizing::fixed(RUN), Sizing::fixed(RISE)))
            .show(ui, |ui| {
                Text::new(side.name)
                    .id_salt(side.name)
                    .style(&style)
                    .align(Align::CENTER)
                    .show(ui);
            });
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
    /// `None` where that point is off the cube.
    ///
    /// The whole of what these tests need: the drawing is judged by eye and the
    /// arithmetic is judged here, so a face that stopped resolving to its own
    /// view fails without a frame being rendered.
    fn aimed(camera: Camera, at: Vec2) -> Option<Bearing> {
        Some(Bearing::from(
            Seen::of(&Theme::default(), camera).zone(at)?.direction(),
        ))
    }

    /// The bearing that looks square on at the face called `name`.
    fn facing(name: &str) -> Bearing {
        let side = SIDES
            .iter()
            .find(|side| side.name == name)
            .expect("the cube has a face by that name");
        Bearing::from(side.normal)
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

    /// A corner of a face is the three-quarter view, and not the face's own.
    ///
    /// Aimed at the far corner of the front face as it projects from a corner
    /// view, which is inside the corner band on both axes by construction.
    #[test]
    fn a_corner_of_a_face_aims_at_the_corner_rather_than_the_face() {
        let camera = looking(Vec3::new(1.0, 1.0, 1.0));
        let at = Seen::of(&Theme::default(), camera).flat(Vec3::new(0.85, 0.85, 1.0));
        let aim = aimed(camera, at).expect("a corner of the front face is on the cube");
        let square_on = facing("FRONT");
        assert!(
            (aim.pitch - square_on.pitch).abs() > 0.1,
            "the corner resolved to the face's own view: {aim:?}"
        );
        // Up and to one side, which is what a three-quarter view is.
        assert!(aim.pitch > 0.4 && aim.pitch < 0.8, "{aim:?}");
    }

    /// Off the cube is nothing at all, rather than the nearest face.
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
