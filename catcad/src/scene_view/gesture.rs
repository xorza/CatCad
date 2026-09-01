//! What the pointer is doing to the view: what a press took hold of, and what
//! its travel writes.

use aperture::{HitAt, Motion};
use glam::{DVec3, Vec3};
use silverpoint::{ConstraintId, Entity, Grown};

use crate::document::Document;
use crate::drawing::{Drawing, Grip};
use crate::intent::change::Change;
use crate::intent::{Choice, Intent};
use crate::lens::Lens;
use crate::part::Part;
use crate::prompt::Prompt;
use crate::scene_view::Travelled;
use crate::scene_view::aimed::Aimed;
use crate::scene_view::picture::{Picture, Under};
use crate::session::Session;
use crate::timeline::along::Along;
use crate::timeline::{Axle, FeatureId, Movable, Spindle};
use crate::tool::Tool;
use std::f64::consts::{PI, TAU};

/// What the pointer is doing to the view, settled when the button goes down.
///
/// Decided once at the press rather than asked again each frame: what is under
/// the cursor moves as the drag proceeds, and a gesture that re-decided would
/// let go of a point the moment the drag outran it.
#[derive(Debug, Clone, Copy, Default)]
pub(super) enum Gesture {
    #[default]
    None,
    /// Turning the camera.
    Orbit {
        travel: Travelled,
    },
    Move(Held),
}

impl Gesture {
    /// What a press that took hold of nothing comes to.
    ///
    /// Every way of failing to grab falls back to turning the view — no tool
    /// free to grab, nothing under the cursor, nothing there worth taking hold
    /// of, a cursor the motion cannot be resolved against — so it is one answer
    /// rather than a spelling of a zero at each of them.
    const TURNS: Self = Gesture::Orbit {
        travel: Travelled::NONE,
    };

    /// Decide what this press is the start of.
    ///
    /// Something that will move takes precedence, and everything else — empty
    /// space, a solid, a point the drawing pins — turns the camera. Grabbing
    /// nothing has to stay the way the view is orbited, or the pointer would
    /// lose its only way to look around.
    ///
    /// The whole session rather than the three facts of it a press reads. They
    /// arrive together and are read together — what is in hand decides whether
    /// a press grabs at all, which sketch is open decides what it may grab, and
    /// a depth being decided is one of the things it can be — so a caller
    /// picking them apart would be a caller free to hand over three that never
    /// stood at one moment.
    ///
    /// `aimed` and `lens` are the frame's own readings rather than second ones
    /// taken off the response, and the first carries the `hovered` filter with
    /// it — see [`SceneView::poll`](crate::scene_view::SceneView). Either being
    /// absent is a press with nothing under it, which orbits like a press on
    /// empty space.
    pub(super) fn grab(
        aimed: Option<Aimed>,
        lens: Option<Lens>,
        picture: &Picture,
        document: &Document,
        session: &Session,
    ) -> Self {
        if session.tool() != Tool::Pointer {
            // A tool with something to put down has no business picking up what
            // is already there. The view is still turned under it, so a point
            // can be aimed at from wherever it needs to be seen from.
            //
            // Before anything is read of the session, because nothing below is
            // worth working out for a press that cannot grab.
            return Self::TURNS;
        }
        aimed
            .zip(lens)
            .and_then(|(aimed, lens)| Held::taken(aimed, lens, picture, document, session))
            .map_or(Self::TURNS, Gesture::Move)
    }
}

/// What is being dragged, and where the pointer may take it.
///
/// What was grabbed is held apart from the motion on purpose. They agree while
/// a sketch entity is dragged by its own geometry, and they part where a datum
/// is: there the whole outline is the handle, and the line it travels along is
/// the base plane's rather than anything the outline says.
#[derive(Debug, Clone, Copy)]
pub(super) struct Held {
    /// What was taken hold of, as something that can be picked out.
    ///
    /// Kept beside [`Grabbed`] rather than derived from it: that one says what
    /// the drag *writes*, in the vocabulary the change needs, and this says what
    /// the user grabbed, in the vocabulary the selection needs. A datum is a
    /// `Movable` to one and a `Part::Step` to the other, and a solid's far end
    /// is the same `Movable` to one and a `Part::Solid` to the other — which is
    /// exactly why neither can be worked out from the other.
    pub(super) part: Part,
    grabbed: Grabbed,
    /// Where the pointer may take it, which is what this frame's cursor is
    /// resolved against — see [`landing`](crate::scene_view::aimed::landing).
    pub(super) motion: Motion,
    /// Where what was grabbed sits relative to where the press landed on the
    /// motion, so a grab three pixels off centre does not snap it to the cursor.
    ///
    /// Three pixels either way. Geometry is grabbed by the very thing that
    /// moves, and a datum by an outline whose travel line is taken through the
    /// grab — see [`Along::travel`](crate::timeline::along::Along::travel) — so
    /// what is left over in both cases is only how far the pick landed from the
    /// cursor, which is the width of a stroke and no more.
    ///
    /// Kept even so, because a plane has nowhere across its line to go and this
    /// carries a little of *across*: a pick on an outline answers with the
    /// nearest point of the stroke rather than the one under the cursor. The
    /// part that cannot be used is dropped where the travel becomes a number —
    /// see [`Along::offset_at`](crate::timeline::along::Along::offset_at).
    offset: Vec3,
}

impl Held {
    /// What a press aiming at `aimed` takes hold of, or `None` where it takes
    /// hold of nothing.
    ///
    /// **The whole of what a press decides**, in the order the answers lean on
    /// each other: what is under the cursor, what that is to take hold of, where
    /// the pointer may then take it, and how far what was grabbed stands off
    /// where the press landed. The last three are [`Grabbed`]'s own — see the
    /// methods there — so what is left here is asking them in order and putting
    /// the four together.
    ///
    /// Its own call rather than the body of [`Gesture::grab`], which is what
    /// leaves that one the two things it decides: whether a press may grab at
    /// all, and what a press that grabbed nothing comes to. Every `?` below
    /// means the one thing — nothing to take hold of — where threaded through
    /// there it stood beside a `return` and a `match` arm meaning the same.
    fn taken(
        aimed: Aimed,
        lens: Lens,
        picture: &Picture,
        document: &Document,
        session: &Session,
    ) -> Option<Self> {
        let Under { aim, hit, part } = picture.under(aimed, lens)?;
        // `None` where no sketch is open, which is a press that can still grab:
        // a datum, a solid's far end and a depth being decided are none of them
        // the open sketch's — see [`Grabbed`]. Only the two arms that *are* read
        // a drawing, and neither is reachable without one.
        let drawing = session.editing().and_then(|at| document.drawing_at(at));
        let grabbed = Grabbed::under(part, hit.at, hit.world, drawing, document, session)?;
        let motion = grabbed.motion(drawing, hit.world);
        let carried = grabbed.carried(hit.world, drawing, picture, lens);
        Some(Held {
            part,
            grabbed,
            motion,
            // Where the press landed on the motion, against where what was
            // grabbed actually is: a grab is not a teleport.
            offset: hit.world - motion.resolve(&aim)? + carried,
        })
    }

    /// What this frame's travel writes, with the cursor landed at `at` on the
    /// motion.
    ///
    /// The offset and nothing else: where the cursor landed plus however far off
    /// centre the press was, which is the one thing a [`Held`] keeps that a
    /// [`Grabbed`] does not. What that place then comes out *as* is
    /// [`Grabbed::writes`]'s, which is where the five part company — including
    /// the `None`, which is the label arm wanting a mark the drawing has placed.
    pub(super) fn writes(
        self,
        at: Vec3,
        sketch: Option<FeatureId>,
        drawing: Option<Drawing<'_>>,
        picture: &Picture,
        lens: Option<Lens>,
    ) -> Option<Intent> {
        self.grabbed
            .writes(at + self.offset, sketch, drawing, picture, lens)
    }
}

/// What a drag took hold of, and so which change its travel is written as.
///
/// Five kinds rather than one, because they are five different things to write:
/// geometry moves within a sketch and is solved for afterwards, a datum moves
/// the sketches drawn on it without any of them saying anything different, a
/// solid's far end restates how far it was carried, a depth still being decided
/// writes a form's draft rather than the document at all, and a number moves
/// where the drawing *says* something and no geometry with it. Which of the five
/// a press found is settled once, at the press, exactly as everything else about
/// a gesture is.
///
/// [`Grabbed::Datum`] and [`Grabbed::Cap`] carry the same [`Movable`] and stay
/// separate arms even so, as do [`Grabbed::Cap`] and [`Grabbed::Growing`] with
/// their shared normal. What each pair shares is the arithmetic — one number
/// along a line — and what they differ in is the change they come out as, which
/// is the whole of what this enum is read for.
#[derive(Debug, Clone, Copy)]
enum Grabbed {
    /// Geometry of the sketch being worked in, at the grip the press settled on.
    Sketch(Grip),
    /// A datum plane, which travels along the line it is offset on.
    ///
    /// Whichever sketch is open, unlike the arm above: a plane belongs to none
    /// of them — it is what they are drawn *on* — so moving one is not an edit
    /// that has to land where you are.
    Datum(Movable),
    /// The depth of a solid still being decided, which travels along the same
    /// normal as the arm below and writes a form's draft rather than the
    /// document — see [`Part::Growing`].
    ///
    /// No handle, because there is no step to name: what this carries is a
    /// reading the form holds, and the form is what turns it into a change when
    /// it is committed.
    ///
    /// The depth rides along with the line it travels because the arrow stands
    /// *off* the face it carries — see [`Grabbed::carried`]. Read from the form
    /// once, here, rather than again where the standoff is worked out: the
    /// second reading was a second `?` on an answer the first had already
    /// settled, and a parse of the draft on every press that grabbed anything at
    /// all.
    Growing { along: Along, depth: f64 },
    /// How much of a turn a solid still being decided sweeps, which travels
    /// round the line it spins about and writes a form's draft — see
    /// [`Part::Turning`].
    ///
    /// **The angle travelled, not the angle reached.** Where a depth is a
    /// number the line itself measures, an angle is measured from a direction
    /// somebody has to choose — and the two that would have to agree on one are
    /// the handle and the drag. So neither is told: the press records where it
    /// landed, every frame reads how far round from there the pointer has gone,
    /// and a difference of two angles is the same in any frame.
    ///
    /// The turn at the press rides along for the same reason a depth does: what
    /// a drag hands back is that turn plus the travel.
    Turning {
        spindle: Spindle,
        /// The direction the two angles are measured from, which says nothing
        /// beyond being the same one twice.
        reference: DVec3,
        /// Where the press landed, and how much of a turn the form read then.
        angle: f64,
        sweep: f64,
    },
    /// The far end of a solid, which travels along the normal of the plane its
    /// region was drawn on.
    ///
    /// The same [`Movable`] the datum carries, because it is the same
    /// arithmetic: one number measured along a normal, and a line to read it
    /// off. What differs is the change it comes out as, and that is what these
    /// two arms are for.
    Cap(Movable),
    /// A dimension's number, which moves where the drawing *says* something and
    /// no geometry at all.
    ///
    /// The one grab that is not a handle on a thing: what is taken hold of is
    /// the figure itself, which is why it travels across the whole sketch plane
    /// like geometry rather than along a line like the two above. What it leaves
    /// behind is a placement — see [`Change::Place`].
    Label(ConstraintId),
}

impl Grabbed {
    /// What a press on `part` takes hold of, or `None` where it takes hold of
    /// nothing.
    ///
    /// The order of the arms is the order of precedence, and two of them are
    /// guards rather than shapes: a dimension's number is tested before the
    /// grip below it, because a grip is about what the *solver* would move and a
    /// number moves nothing it knows about.
    ///
    /// `drawing` is the sketch being worked in, which is what every edit lands
    /// on. The document is here for the two steps that are in no sketch, and the
    /// session for the form a handle is drawn from. `world` is where the press
    /// landed, which one arm records — see [`Grabbed::Turning`], on why an
    /// angle is measured from where a drag began.
    fn under(
        part: Part,
        on: HitAt,
        world: Vec3,
        drawing: Option<Drawing<'_>>,
        document: &Document,
        session: &Session,
    ) -> Option<Self> {
        let editing = session.editing();
        Some(match part {
            // A step, whatever sketch is open. The one kind that goes anywhere
            // is a plane somebody put there: what it moves is where the sketches
            // on it land, and none of them is being edited by it. Everything
            // else answers `None` and so orbits, which is right — a world plane
            // is not somewhere anybody put one, and a sketch is drawn on
            // whatever it is drawn on. See
            // [`Timeline::movable`](crate::timeline::Timeline).
            Part::Step(at) => Grabbed::Datum(document.movable(at)?),
            // The far end alone. The base lies in the plane the region was drawn
            // on and has nowhere of its own to go, and a wall is carried by both
            // ends at once — so neither says how far, and a press on either
            // turns the view like a press on anything else that does not move.
            Part::Solid {
                of,
                face: Grown::Far,
            } => Grabbed::Cap(document.stretching(of)),
            // The arrow standing off a region whose depth is being decided. It
            // travels along that region's own plane, which is the same line the
            // far end of a built solid runs on — the difference is only that
            // there is no step yet to name.
            //
            // The plane the *form* named rather than the open sketch's. The two
            // agree whenever the form was opened the ordinary way, since picking
            // a region opens the sketch it came from — and where they came apart
            // the drag would travel along a different normal and nothing would
            // look wrong. No form is the arrow that was never drawn.
            Part::Growing => {
                let carrying = session.prompt().and_then(Prompt::carrying)?;
                Grabbed::Growing {
                    along: Along::on(document.drawing_at(carrying.sketch)?.plane()),
                    depth: carrying.depth,
                }
            }
            // The arrow riding the circle a region's own middle sweeps. It
            // travels on the plane square to the line, which is the one surface
            // every angle about that line is reached on.
            Part::Turning => {
                let turning = session.prompt().and_then(Prompt::turning)?;
                let drawing = document.drawing_at(turning.sketch)?;
                let spindle = Axle::of(drawing.sketch(), turning.axis)?.borne(drawing.plane())?;
                let reference = spindle.direction.any_orthonormal_vector();
                Grabbed::Turning {
                    spindle,
                    reference,
                    angle: spindle.reads(reference, world.as_dvec3()).angle,
                    sweep: turning.sector.sweep,
                }
            }
            // A dimension's number, which is the one thing a press can find that
            // has a place of its own without being geometry. See
            // [`Drawing::grip`], which goes on answering `None` for every
            // relation including this one.
            _ if let Some(id) = label(part, drawing, editing) => Grabbed::Label(id),
            // Only the sketch being worked in can be taken hold of. A drag of
            // geometry is an edit and an edit lands where you are — and the
            // handles would not even tell the two apart: two sketches are two
            // arenas and mint the same ones, so a grip that read the entity
            // alone would take hold of whatever sat at that slot in the open
            // sketch. See [`Part`](crate::part::Part).
            _ if editing.is_some_and(|editing| part.sketch() == Some(editing)) => {
                Grabbed::Sketch(drawing?.grip(part.entity()?, on)?)
            }
            // Anything else is a press on a sketch nobody is in, which turns the
            // view like a press on empty space.
            _ => return None,
        })
    }

    /// Where the pointer may take what this has hold of.
    ///
    /// Taken through the grab rather than through what is being moved — see
    /// [`Along::travel`](crate::timeline::along::Along::travel), which is where that
    /// matters and why.
    fn motion(self, drawing: Option<Drawing<'_>>, at: Vec3) -> Motion {
        match self {
            // A number travels across the drawing exactly as geometry does: it
            // is placed on the sketch's own plane, and where on it is the whole
            // of what a placement says.
            //
            // The one place here that insists on a drawing, and it may: these
            // two are the only arms [`Grabbed::under`] builds *through* one, so
            // holding either is holding a sketch open.
            Grabbed::Sketch(_) | Grabbed::Label(_) => drawing
                .expect("a grip and a label are found through the open sketch")
                .motion(),
            Grabbed::Datum(movable) | Grabbed::Cap(movable) => movable.along.travel(at),
            Grabbed::Growing { along, .. } => along.travel(at),
            // The plane square to the line, taken through the grab for the
            // reason a line is — see
            // [`Along::travel`](crate::timeline::along::Along::travel).
            Grabbed::Turning { spindle, .. } => Motion::Plane {
                origin: at,
                normal: spindle.direction.as_vec3(),
            },
        }
    }

    /// How far what was grabbed stands off where the press landed on it.
    ///
    /// **Nothing, for everything grabbed by the very thing that moves** — a
    /// point by the point, a solid's far end by that face — where the press and
    /// the value are the same place to within the width of a stroke.
    ///
    /// The two that stand *off* what they carry are the two that need it. An
    /// arrow stands off the face whose depth it sets, so a grab near its head is
    /// a grab a whole arrow-length past that depth, and without this the solid
    /// leaps that far the moment it is touched. A number stands off the point it
    /// names for the same reason — the box floats clear of the geometry so it
    /// can be read — and what a placement says is where the point under it goes,
    /// so the press records where the *box* sat relative to the cursor and every
    /// frame of the drag puts the box back there. Given back as it stands then
    /// rather than as it stood here, because it moves: see
    /// [`Mark::standoff`](crate::paint::marks::mark::Mark).
    fn carried(
        self,
        at: Vec3,
        drawing: Option<Drawing<'_>>,
        picture: &Picture,
        lens: Lens,
    ) -> Vec3 {
        match self {
            Grabbed::Growing { along, depth } => {
                along.normal() * (depth - along.offset_at(at)) as f32
            }
            // Nothing to carry where the mark has not been placed — and the
            // same answer where no sketch is open, which the drawing being
            // absent says. Neither is reachable holding a label; both are
            // spelled as the zero this already had rather than as a claim.
            Grabbed::Label(id) => {
                drawing
                    .zip(picture.placed(id))
                    .map_or(Vec3::ZERO, |(drawing, placed)| {
                        placed.mark.world(drawing) - at + placed.mark.standoff(drawing, lens)
                    })
            }
            // Nothing, and for the reason the three below carry nothing: the
            // press and the value are one place. What the press landed on is
            // the very angle the drag measures its travel from — see
            // [`Grabbed::Turning`] — so a grab a stroke off centre is a start
            // a stroke off centre, which cancels.
            Grabbed::Sketch(_) | Grabbed::Datum(_) | Grabbed::Cap(_) | Grabbed::Turning { .. } => {
                Vec3::ZERO
            }
        }
    }

    /// What a drag that has carried this to `to` writes.
    ///
    /// **What the press decided, spent.** Which of the five a drag is was
    /// settled when the button went down; all that is left is turning one place
    /// on the motion into the thing the document — or the form — is asked for.
    ///
    /// A plane and a solid's far end name a distance where geometry names a
    /// place, because that is what each of them *is*: either has one number, and
    /// asking it to be somewhere would be asking a question with two answers it
    /// does not have.
    ///
    /// An [`Intent`] rather than a [`Change`], because one of the five is not an
    /// edit: a depth still being decided lives in a form, so what the drag
    /// writes is a draft.
    fn writes(
        self,
        to: Vec3,
        sketch: Option<FeatureId>,
        drawing: Option<Drawing<'_>>,
        picture: &Picture,
        lens: Option<Lens>,
    ) -> Option<Intent> {
        match self {
            // The two arms that name the open sketch, and the only two that can
            // fail to find one — which they never do, being the two a press
            // only ever grabs *through* a drawing. See [`Grabbed::under`].
            Grabbed::Sketch(grip) => Some(Intent::from(Change::Drag {
                sketch: sketch?,
                grip,
                to,
            })),
            Grabbed::Datum(movable) => Some(
                Change::MovePlane {
                    plane: movable.at,
                    to: movable.along.offset_at(to),
                }
                .into(),
            ),
            Grabbed::Cap(movable) => Some(
                Change::Carry {
                    extrude: movable.at,
                    to: movable.along.offset_at(to),
                }
                .into(),
            ),
            // Where the *box* should land, less where the box stands off the
            // point a placement names — read as it is now, because a radius's
            // standoff turns with the number and a stacked mark's drops a lane
            // the moment it moves. A frame behind, and that is what makes it
            // converge: each frame corrects with the last one's answer, and a
            // drag is many frames.
            //
            // `to` is where the *box* should land and a placement names the
            // point under it, so the clearance comes off by
            // [`Mark::anchor`](crate::paint::marks::mark::Mark) — which inverts
            // it rather than taking off the last frame's. A frame with no mark
            // to invert says nothing at all: the number stays put, which is a
            // stutter, where placing it against no clearance would move it by
            // the whole of one.
            Grabbed::Label(constraint) => {
                let (sketch, drawing) = (sketch?, drawing?);
                picture.placed(constraint).zip(lens).map(|(placed, lens)| {
                    Change::Place {
                        sketch,
                        constraint,
                        at: placed.mark.anchor(
                            drawing.sketch().constraint(constraint),
                            drawing,
                            lens,
                            to,
                        ),
                    }
                    .into()
                })
            }
            Grabbed::Growing { along, .. } => Some(
                Choice::Set {
                    nth: 0,
                    to: along.offset_at(to),
                }
                .into(),
            ),
            Grabbed::Turning {
                spindle,
                reference,
                angle,
                sweep,
            } => {
                // Wrapped to the half turn either way, which is the whole of
                // what one drag can say: a pointer at some angle is at that
                // angle and at every whole turn from it, and the nearest is the
                // one it travelled to. Further than half a turn in a single
                // gesture is a release and a second grab.
                let reached = spindle.reads(reference, to.as_dvec3()).angle;
                let travelled = (reached - angle + PI).rem_euclid(TAU) - PI;
                Some(
                    Choice::Set {
                        nth: 1,
                        // Held inside a whole turn, past which a revolve sweeps
                        // the same space twice and the kernel raises nothing —
                        // so a handle that could reach there would be a handle
                        // that could rub the solid out.
                        to: (sweep + travelled).clamp(-TAU, TAU).to_degrees(),
                    }
                    .into(),
                )
            }
        }
    }
}

/// `part` as a number a press could take hold of, or `None` where it is not one.
///
/// A dimension, and one of the sketch being worked in. Every other relation has
/// no number and so nowhere of its own to be — where a symbol goes is worked out
/// from the geometry it is about, and dragging one would be arguing with that
/// rather than saying anything. A part of another sketch is refused for the
/// reason every press is: moving a number is an edit, and an edit lands where
/// you are.
///
/// A free fn rather than a method, because nothing about it is a gesture's: it
/// reads the drawing, and the press is only what happened to be asking.
pub(super) fn label(
    part: Part,
    drawing: Option<Drawing<'_>>,
    editing: Option<FeatureId>,
) -> Option<ConstraintId> {
    let Some(Entity::Constraint(id)) = part.entity().filter(|_| part.sketch() == editing) else {
        return None;
    };
    drawing?.sketch().constraint(id).value().map(|_| id)
}
