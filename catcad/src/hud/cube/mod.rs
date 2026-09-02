//! The orientation cube: which way the camera is looking, and one click to a
//! named view.
//!
//! **Drawn by the renderer and worked by a widget.** The solid and the six
//! names are a scene of their own in a pane of the viewport — see [`drawn`] —
//! so they are shaded, antialiased and lettered by the same code the drawing
//! is. What is here is the half a scene cannot do: sensing the box, resolving
//! which piece a press landed on, and easing the camera round to it.

use std::f32::consts::{PI, TAU};

use aperture::{Camera, Highlight, Lit, Viewport};
use glam::{Mat4, UVec2, Vec2, Vec3};
use palantir::{
    Align, AnimSlot, Animatable, Configure, Drag, Panel, Rect, Sense, Shape, Sizing, Ui,
    Vec2 as UiVec2, WidgetId,
};

use crate::hud::cube::facet::{Facet, SIDES};
use crate::intent::Intents;
use crate::intent::change::Change;
use crate::look;
use crate::look::Theme;
use crate::scene_view::{Travelled, orbited};

pub(crate) mod drawn;
mod facet;

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
        // Wound in one step rather than by subtracting turns until it lands: a
        // yaw that arrived as a NaN would never land, and a loop is the one
        // shape of that mistake that hangs rather than draws wrong.
        let round = (self.yaw - near + PI).rem_euclid(TAU) - PI;
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

/// Where the gizmo landed and what the pointer is on, as its pane needs them.
///
/// **What the widget knows and the renderer does not.** Where the box sits is
/// the overlay's layout, and which piece the pointer is over is answered
/// against that box — so both are carried out of the frame that drew the
/// overlay and handed to the picture that draws the pane.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Gizmo<'a> {
    /// The box, in the view's own logical pixels. Empty until the overlay has
    /// arranged once, and a pane with no room draws nothing.
    pub(crate) at: Rect,
    /// What is lit in the gizmo's own scene, which is at most the piece under
    /// the pointer and the word on it.
    pub(crate) lit: &'a [Lit],
}

impl Gizmo<'static> {
    /// Nowhere, and nothing lit on it.
    ///
    /// What a gizmo nothing has arranged or pointed at is, and what a caller
    /// driving the view without an overlay hands over. A pane with no room
    /// draws nothing, so it is a whole answer rather than a stand-in.
    pub(crate) const NOWHERE: Self = Self {
        at: Rect::new(0.0, 0.0, 0.0, 0.0),
        lit: &[],
    };
}

/// The gizmo, and the view it is on its way to.
///
/// **The target is the whole of its state.** Where the camera *is* belongs to
/// the document, and the cube reads it every frame; what is kept here is one
/// gesture's worth of intent — the view asked for, until the camera reaches it.
#[derive(Debug, Default)]
pub(super) struct Cube {
    /// One piece's outline, in the world and then in the box.
    ///
    /// Kept for their room rather than their contents, like everything else the
    /// overlay parks: twenty-six outlines are projected to answer a hover, and
    /// a pair of lists asked for at each of them is twenty-six trips to the
    /// heap a frame.
    ring: Vec<Vec3>,
    flat: Vec<Vec2>,
    /// What the pane is told to light, refilled every frame — see [`Gizmo`].
    lit: Vec<Lit>,
    /// Where the box was last arranged, which is where the pane goes.
    at: Rect,
    turning_to: Option<Bearing>,
    /// How far the drag under way has come — see [`Travelled`].
    ///
    /// Palantir reports a drag as its total, which is what makes it safe to
    /// re-read on a settling frame.
    travel: Travelled,
}

impl Cube {
    /// Sense it, and ask for whatever a press on it named.
    ///
    /// **Shows and does not act**, like everything else on the overlay: the
    /// eased bearing goes out as a [`Change::Aim`] rather than being written to
    /// a camera. Landing twice is harmless because an aim is absolute.
    ///
    /// What it draws is the two arrows and nothing else. The solid and the
    /// names are the renderer's — see [`Gizmo`], which is how what happens here
    /// reaches them.
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
        let under = state
            .pointer_local
            .filter(|_| state.hovered)
            .and_then(|at| seen.facet_at(Vec2::new(at.x, at.y), &mut self.ring, &mut self.flat));
        self.light(theme, under);
        // The rect the pane is drawn into, which is a frame behind: the overlay
        // is arranged after it is recorded, so what a widget knows of its own
        // box is where it was put last time. A resize therefore carries the
        // gizmo one frame after the window, and nothing else moves it.
        self.at = state.rect.unwrap_or(Gizmo::NOWHERE.at);
        // A canvas rather than a stack, because the arrows are *placed* at
        // either edge rather than laid out one after the other.
        Panel::canvas()
            .id(id)
            .size((
                Sizing::fixed(theme.chrome.cube),
                Sizing::fixed(theme.chrome.cube),
            ))
            .align(Align::CENTER)
            .sense(Sense::CLICK | Sense::DRAG)
            .show(ui, |ui| {
                // Away while the cube itself is under the pointer, so they do
                // not compete with the thing they sit beside.
                if under.is_none() {
                    arrows(ui, theme);
                }
            });
        if let Some(facet) = under.filter(|_| state.left.clicked()) {
            self.turning_to = Some(Bearing::from(facet.out()).near(camera.yaw));
        }
        self.drag(state.left.drag, intents);
        self.turn(ui, id, theme, camera, intents);
    }

    /// Where it is and what is lit on it, for the picture that draws its pane.
    pub(crate) fn gizmo(&self) -> Gizmo<'_> {
        Gizmo {
            at: self.at,
            lit: &self.lit,
        }
    }

    /// What the piece under the pointer, and the word on it, are lit in.
    ///
    /// Two looks rather than one, because they land on top of each other: a
    /// face gone bright with one colour on it would be a face whose name had
    /// disappeared into it.
    fn light(&mut self, theme: &Theme, under: Option<Facet>) {
        let chrome = &theme.chrome;
        self.lit.clear();
        let Some(facet) = under else {
            return;
        };
        self.lit.push(Lit {
            tag: facet.tag(),
            look: Highlight::new(look::ink(chrome.cube_high)),
        });
        // The pill's own dark, which is what reads on a face that has gone
        // light — the same pairing a held chip is inked with.
        if let Some(side) = SIDES.into_iter().find(|side| side.facet() == facet) {
            self.lit.push(Lit {
                tag: side.tag(),
                look: Highlight::new(look::ink(chrome.on_held)),
            });
        }
    }

    /// Turn it by hand.
    ///
    /// The same gesture the drawing itself takes and so the same intent, at the
    /// same rate — see [`ORBIT_RATE`](crate::scene_view::ORBIT_RATE). A drag
    /// also gives up whatever view was being turned to: taking hold of the
    /// cube is saying where to look more directly than a press did.
    fn drag(&mut self, drag: Drag, intents: &mut Intents) {
        let step = self.travel.step(drag);
        // A drag of no distance is still a drag and gives up the view being
        // turned to, where a frame with no drag at all leaves that turn
        // running — which is why the drag is read again rather than the step.
        let (Drag::Started { .. } | Drag::Active { .. }) = drag else {
            return;
        };
        self.turning_to = None;
        intents.push(orbited(step));
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
/// **The pane's own projection, and not a second one.** What a press lands on
/// has to agree with what was drawn, so both go through the very matrix the
/// pane is drawn by — see [`drawn::camera`]. A basis worked out again here from
/// the same yaw and pitch would agree with it until the day one of the two
/// changed.
///
/// Built once and handed down: a frame projects the corners of every piece in
/// view, and building the matrix at each of them would be doing once-a-frame
/// arithmetic a hundred times over.
#[derive(Debug)]
struct Seen {
    /// Where the eye stands, as a direction from what it is looking at.
    eye: Vec3,
    view_proj: Mat4,
    /// The gizmo's box, which is the pane's viewport.
    viewport: Viewport,
    /// How far the solid's edges are cut, so an outline asked for here is the
    /// outline the pane drew.
    chamfer: f32,
}

impl Seen {
    fn of(theme: &Theme, aim: Camera) -> Self {
        let camera = drawn::camera(theme, aim);
        let viewport = Viewport::new(UVec2::splat(theme.chrome.cube as u32));
        Self {
            eye: drawn::eye(&camera),
            view_proj: camera.view_proj(viewport.aspect()),
            viewport,
            chamfer: theme.chrome.cube_chamfer,
        }
    }

    /// Whether a piece of the solid is turned toward the eye at all.
    fn shows(&self, facet: Facet) -> bool {
        facet.out().dot(self.eye) > 0.0
    }

    /// Where a point of the solid lands in the box, counting down from its
    /// top-left corner — the way a pointer arrives.
    ///
    /// Never off the view: the projection is parallel and its slab reaches
    /// sixty-four times the standoff, so nothing an inch across is clipped.
    fn at(&self, position: Vec3) -> Vec2 {
        self.viewport
            .pixel_of(self.view_proj * position.extend(1.0))
            .expect("a parallel view of the gizmo draws the whole of it")
    }

    /// The outline of `facet`, laid into `flat`.
    fn outline(&self, facet: Facet, ring: &mut Vec<Vec3>, flat: &mut Vec<Vec2>) {
        facet.ring(self.chamfer, ring);
        flat.clear();
        flat.extend(ring.iter().map(|&at| self.at(at)));
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

/// The two arrows that step the view a quarter turn to either side.
///
/// **A step in yaw, not a roll.** A cube in a modeller that carries one usually
/// rolls the view about the axis it is looking down; this camera holds a yaw
/// and a pitch and no roll at all, and giving it one reaches the projection,
/// the ray cast and which way up a name is written. A quarter turn is the
/// useful nine tenths of what the arrows are for.
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

    /// The middle of the gizmo's box, which every press below is measured from.
    fn middle() -> Vec2 {
        Vec2::splat(Theme::default().chrome.cube * 0.5)
    }

    /// Where a press `off` the middle of the box would point the camera, or
    /// `None` where that point is off the solid.
    ///
    /// The whole of what these tests need: the drawing is judged by eye and the
    /// arithmetic is judged here, so a piece that stopped resolving to its own
    /// view fails without a frame being rendered.
    fn aimed(camera: Camera, off: Vec2) -> Option<Bearing> {
        let (mut ring, mut flat) = (Vec::new(), Vec::new());
        let facet =
            Seen::of(&Theme::default(), camera).facet_at(middle() + off, &mut ring, &mut flat)?;
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
        let at = flat.iter().sum::<Vec2>() / flat.len() as f32;
        let aim = aimed(camera, at - middle()).expect("the bevel is in view");
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
