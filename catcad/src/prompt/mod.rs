//! A value asked for against the drawing: where the form stands, and what
//! committing it asks the document for.
//!
//! **Session state, and scratch.** Nothing here is written down by saving, none
//! of it is a step to take back, and a form abandoned is a form that never
//! happened — the document is not touched until the value is committed. That
//! puts it on the same footing as the rubber band a half-drawn line follows.
//!
//! **What is left after palantir.** Where the caret is, what is picked out,
//! what a keystroke does to either, and every rule about clicking into a line
//! belong to the [`TextEdit`] a field is shown through — and *fitting the form
//! on screen* belongs to [`Popup`], which resolves a body against an anchor and
//! flips or shifts it to stay on the surface. What is left is the two things
//! neither can know: where in the world the form is about, and what pressing
//! Enter means.

use glam::Vec2;
use palantir::{
    Align, Button, ButtonTheme, ClickOutside, Configure, HAlign, Panel, Popup, Rect, Sizing, Text,
    TextEdit, TextEditTheme, TextRun, TextWrap, Ui, VAlign, WidgetId,
};
use silverpoint::{CircleId, Constraint, Dimension, Entity};
use std::fmt::Write;

use crate::drawing::anchor::Anchor;
use crate::intent::{Change, Choice, Intents, Opening, Step};
use crate::model::{Model, Models};
use crate::paint::growing::Growing;
use crate::paint::{DECIMALS, mark_font};
use crate::part::Part;
use crate::profile::Profile;
use crate::timeline::FeatureId;
use crate::tool::Tool;

pub(crate) mod look;

/// What a form is about, and so what committing it asks for.
///
/// One arm per operation, which is the same shape [`Tool`](crate::tool::Tool)
/// and [`Change`] already take. It is what turns "the user pressed Enter" into
/// something the document understands, and there is no way to know that without
/// knowing which operation is in hand.
/// Not [`Copy`], and that is [`Profile`]'s doing rather than an oversight — see
/// [`Asking::Extrude`].
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Asking {
    /// A sketch dimension being restated.
    Dimension { part: Part },
    /// A circle being drawn about a centre already placed.
    ///
    /// The one form that stands where there is nothing yet to name — see
    /// [`Opening::Circle`](crate::intent::Opening). Committing it *makes* the
    /// circle, where every other arm here restates something already drawn, and
    /// cancelling puts the tool back to its first click.
    Circle { sketch: FeatureId, center: Anchor },
    /// A circle being held to a radius it does not yet have.
    ///
    /// A [`CircleId`] and not a [`Profile`], unlike the arm below, and the
    /// difference is what each of them names: a circle is one of the sketch's
    /// own handles, which survives its geometry moving and being cut into
    /// pieces, where a region is a face of an arrangement and survives nothing.
    Radius { sketch: FeatureId, circle: CircleId },
    /// A solid being grown off a region, *before* it reaches the timeline.
    ///
    /// Held here rather than created at zero and carried, because a
    /// [`Prism`](silverpoint::Prism) is a reading rather than a thing the
    /// document holds — an arrangement, a region, a plane and a distance, all
    /// four of which this has without a step existing. So the solid can be
    /// drawn while it is still being decided, the document is not touched until
    /// confirm, and cancelling is dropping this rather than unpicking a step
    /// that was already taken.
    ///
    /// **A [`Profile`] and not a position**, which is the whole reason this
    /// enum is not [`Copy`]. A position names a face by where it fell in the
    /// arrangement's walk and holds only while the drawing's topology does —
    /// long enough for the intent that opens this, which lands the frame it is
    /// raised, and nowhere near long enough for a form. The viewport stays live
    /// under an open form, so an undo or an edge dragged across another rebuilds
    /// the arrangement while someone is still typing; a position would then name
    /// a different region and the solid would quietly become one, or the commit
    /// would grow it.
    Extrude { profile: Profile },
}

impl Asking {
    /// The region a solid is being grown from, where growing one is what this
    /// form is about.
    ///
    /// **The one arm two readings share.** What a *press* on the depth arrow
    /// needs and what the *drawing* shows are different answers — see
    /// [`Prompt::carrying`] and [`Prompt::growing`], which part company over
    /// whether they can afford to resolve the profile against an arrangement —
    /// but both begin here, and they began here in two different spellings.
    fn extruding(&self) -> Option<&Profile> {
        match self {
            Asking::Extrude { profile } => Some(profile),
            Asking::Dimension { .. } | Asking::Radius { .. } | Asking::Circle { .. } => None,
        }
    }
}

/// Where a form stands relative to what it is about.
///
/// **The distinction the whole module turns on.** A dimension's field stands
/// *over* its mark — the drawing leaves the mark out and the field takes its
/// place, and a number that shifted as it became editable would be exactly the
/// jump the alignment below exists to prevent. Everything else stands *beside*
/// what it is about, because the point of an extrude's form is that you can
/// still see the face.
///
/// Collapsing the two would either cover the geometry a form is asking about or
/// make an editable number jump, so the two placements are named rather than
/// approximated by one.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Stands {
    /// The middle of the box the drawing would have put what the form replaces
    /// in — see [`Mark::centre`](crate::paint::marks::mark::Mark).
    ///
    /// The middle rather than the mark's anchor, and worked out before it gets
    /// here, because where a mark's box sits relative to the geometry it names
    /// is the *drawing's* business: it rises up the run's own frame by however
    /// many lanes its stack is deep, and that frame is the sketch plane's rather
    /// than the screen's. What is left for a form is to centre itself on a
    /// point.
    Over(Vec2),
    /// Clear of a footprint, so what the form is about stays visible under it —
    /// see [`Lens::footprint`](crate::lens::Lens), which measures one.
    ///
    /// A shape the projection draws none of has no footprint, and that is a
    /// frame the form is not shown for rather than a form that *closes*: a
    /// camera turning back brings it round again, and geometry swinging out of
    /// view is nobody asking to stop typing.
    Beside(Rect),
}

/// How far a form standing beside something keeps off it, in logical pixels.
///
/// Enough that the box does not read as part of the drawing, and enough that
/// the outline it is about — a circle's rim while it is being drawn — is not
/// underneath it.
const STANDS_CLEAR: f32 = 14.0;

/// How far apart a form sets the things it is made of, in logical pixels.
///
/// One number for both directions and every row, so a label, a field and two
/// answers read as one control rather than as four things that happen to be
/// near each other.
const GAP: f32 = 4.0;

/// What a field opens showing, and so which of the two inputs starts with it.
///
/// The one rule that keeps every form consistent, and it follows from what the
/// form is *for*: something already drawn has a value, and something being made
/// does not. Getting it wrong is not a crash but a field that will not hand
/// control back — a draft seeded with a number reads as one somebody typed, so
/// the pointer never drives and the placeholder is never seen.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Seed {
    /// A value already decided, in the **draft**. What restating something
    /// already drawn opens on: the keyboard has it from the first frame,
    /// because somebody already said it.
    Stated(f64),
    /// A value nobody has decided, in the **placeholder**. What making
    /// something new opens on: the pointer has it, the draft is empty, and the
    /// first keystroke lands in a field with nothing to fight.
    ///
    /// Still a number rather than nothing, so what the drawing shows of the
    /// half-made thing is decided from the moment the form opens — a solid at
    /// no depth rather than no solid.
    Offered(f64),
}

/// A depth being decided: the sketch whose plane it is measured along, and how
/// deep it currently reads.
///
/// What a press on the arrow that carries it needs, and both halves are needed
/// together. The plane says which line the drag travels on; the depth says how
/// far up that line the *handle* was, which is not the same as how far up the
/// solid is — an arrow stands off the face it carries, so a grab near its head
/// is a grab a whole arrow-length past the depth it sets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Carrying {
    pub(crate) sketch: FeatureId,
    pub(crate) depth: f64,
}

/// One value being typed, and what to call it where two are asked for at once.
#[derive(Debug)]
struct Field {
    /// Empty where the form asks for one thing and standing over it says which
    /// — a dimension's field is the number, and a word beside it would be a
    /// word the drawing never had.
    label: &'static str,
    /// The buffer [`TextEdit`] borrows, so the widget writes it and this reads
    /// it. Never re-seeded from the drawing: a draft is what the *user* has
    /// made of the value, and geometry that moved under it says nothing about
    /// what they meant to type.
    draft: String,
    /// What the field shows while the draft is empty — the value the *pointer*
    /// is describing, which is a suggestion rather than an answer.
    ///
    /// The placeholder rather than the draft, and that is the whole trick. A
    /// pointer writing the draft of a focused field destroys the selection that
    /// makes the first keystroke *replace* rather than insert, so typing `3`
    /// into a field the pointer had carried to `1.47` would give `1.473`.
    /// Written here, the draft stays empty until somebody types, and the first
    /// keystroke lands in an empty field with nothing to fight.
    ///
    /// It also means the two states need no flag between them: **the keyboard
    /// is driving exactly when the draft is not empty.** Backspacing the last
    /// character hands the pointer back, which is the behaviour anyone would
    /// expect and costs nothing to have.
    suggested: String,
}

/// A form open against the drawing: what it is about, and what has been typed.
#[derive(Debug)]
pub(crate) struct Prompt {
    about: Asking,
    /// One per value asked for. A rectangle wants two and everything built so
    /// far wants one, which is why this is a list rather than a field.
    fields: Vec<Field>,
    /// Whether the form has been drawn yet.
    ///
    /// What decides the one frame it asks for focus. A form opens *between* the
    /// asking half of a frame and the next one, so the frame it is shown on is
    /// never the frame it was opened on, and nothing outside it is in a
    /// position to say which one that was.
    shown: bool,
    /// What its fields are set in. Built once per opening rather than per frame
    /// — it is the stock theme with one axis rewritten, and rebuilding it sixty
    /// times a second to say the same thing would be sixty copies of a bundle
    /// that never changes.
    look: TextEditTheme,
    /// The two answers, on the same terms. Built for every form rather than
    /// only the ones that show them: two bundles per *opening* is nothing, and
    /// an `Option` here would be a second thing saying what
    /// [`Prompt::answered`] already says.
    goes: ButtonTheme,
    stops: ButtonTheme,
}

/// What a field made of the frame.
///
/// Named rather than three bools in a row, which is what it was: they are all
/// one type, so any two could change places and still compile — and what the
/// three come to is a *cancel* or a *commit*, which is exactly the kind of
/// mistake nothing downstream would look wrong for.
///
/// Accumulated across a row with [`Said::and`], because Enter in *any* field
/// commits the form: a second value is another thing to say about one
/// operation, not a second operation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Said {
    cancelled: bool,
    lost_focus: bool,
    submitted: bool,
}

impl Said {
    /// What a [`TextEdit`] reported.
    fn of(shown: &palantir::TextEditResponse) -> Self {
        Self {
            cancelled: shown.cancelled,
            lost_focus: shown.lost_focus,
            submitted: shown.submitted,
        }
    }

    /// This and `other` together, which is what a row of fields says.
    fn and(self, other: Self) -> Self {
        Self {
            cancelled: self.cancelled || other.cancelled,
            lost_focus: self.lost_focus || other.lost_focus,
            submitted: self.submitted || other.submitted,
        }
    }
}

/// What a frame of the open form asked of the session, beyond editing a draft.
///
/// Private, and a measure of how little of a form is this crate's: the widgets
/// report `submitted`, `cancelled` and `lost_focus`, and turning those into an
/// answer about a *dimension* or an *extrude* is the whole of what is left.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Done {
    /// Put what the form says on the document.
    Commit,
    /// Leave the document as it was.
    Cancel,
}

impl Prompt {
    /// The form `opening` asks for, or `None` where what it names has gone.
    ///
    /// **Where a request becomes a form**, which is what [`Opening`] is a
    /// separate enum for: an intent is [`Copy`] and carries what the form is to
    /// start out saying, and a form owns its drafts and a name meant to outlive
    /// the arrangement it was read from. Here rather than where the request
    /// lands, because which fields a form shows and what each begins with is the
    /// form's business and not the session's. A fifth kind of form is still
    /// three arms — one in [`Opening`], one in [`Asking`], one here — but two of
    /// the three now sit in the file that owns what they build.
    ///
    /// `models` is the drawing the asking was read against, and one arm wants
    /// it: a position among the faces is only good for the arrangement it came
    /// from, so this is where that position becomes a name — see
    /// [`Asking::Extrude`] and [`Model::profile`].
    pub(crate) fn opening(opening: Opening, models: Models<'_>) -> Option<Self> {
        Some(match opening {
            Opening::Dimension { part, from } => {
                Self::on(Asking::Dimension { part }, &[("", Seed::Stated(from))])
            }
            // Offered rather than stated, like every form that *makes*
            // something: the pointer has the value until somebody types one,
            // and the field shows whichever is speaking. See [`Seed`].
            Opening::Circle { sketch, center } => Self::on(
                Asking::Circle { sketch, center },
                &[("Radius", Seed::Offered(0.0))],
            ),
            Opening::Radius {
                sketch,
                circle,
                from,
            } => Self::on(
                Asking::Radius { sketch, circle },
                &[("Radius", Seed::Stated(from))],
            ),
            // At no depth at all, which is where the ask starts: the solid is on
            // screen from the moment the form opens, and a zero-depth prism is a
            // well-formed one.
            Opening::Extrude { sketch, region } => Self::on(
                Asking::Extrude {
                    profile: models.at(sketch)?.profile(region),
                },
                &[("Depth", Seed::Offered(0.0))],
            ),
        })
    }

    /// Open a form for `about`, seeded with `values`.
    ///
    /// Which of the two inputs each field starts with is [`Seed`]'s to say.
    /// Reached through [`Prompt::opening`], which is what decides both for each
    /// kind of form; each arm stands up its own rather than answering with a
    /// pair the match then builds from, because the seeds are arrays and two
    /// arms of different *lengths* would not agree on a type.
    fn on(about: Asking, values: &[(&'static str, Seed)]) -> Self {
        Self {
            about,
            fields: values
                .iter()
                .map(|&(label, seed)| {
                    let said = |value: f64| format!("{value:.*}", DECIMALS);
                    match seed {
                        Seed::Stated(value) => Field {
                            label,
                            draft: said(value),
                            suggested: String::new(),
                        },
                        Seed::Offered(value) => Field {
                            label,
                            draft: String::new(),
                            suggested: said(value),
                        },
                    }
                })
                .collect(),
            shown: false,
            look: look::field(),
            goes: look::confirm(),
            stops: look::cancel(),
        }
    }

    /// What the form is about.
    pub(crate) fn about(&self) -> &Asking {
        &self.about
    }

    /// The sketch whose region this form is growing a solid off, where that is
    /// what it is about.
    ///
    /// The sketch rather than the region, because it is the half of a
    /// [`Profile`] that needs no arrangement to resolve against — which is what
    /// lets the press that grabs the depth arrow ask for the plane it travels
    /// on without a solve in hand.
    ///
    /// The depth the form says and never one of its own, on the same terms
    /// [`Prompt::growing`] reads one. The two are read on different schedules,
    /// so a fallback here would be the arrow travelling against a depth the
    /// solid was never drawn at.
    pub(crate) fn carrying(&self) -> Option<Carrying> {
        Some(Carrying {
            sketch: self.about.extruding()?.sketch(),
            depth: self.says(0)?,
        })
    }

    /// The part of the drawing this form is about, where it is about one the
    /// document holds.
    ///
    /// What a session prunes on: a form open over geometry an undo took away
    /// would commit onto a handle naming nothing. Apart from [`Prompt::marks`],
    /// which answers a different question — a dimension's form *stands where its
    /// mark would be* and the drawing leaves it out, and a radius form stands
    /// beside a circle whose mark does not exist yet.
    pub(crate) fn holds(&self) -> Option<Part> {
        match &self.about {
            Asking::Dimension { part } => Some(*part),
            Asking::Radius { sketch, circle } => Some(Part::Entity {
                sketch: *sketch,
                entity: (*circle).into(),
            }),
            // Named by a `Profile` rather than a `Part`, so what says it is
            // still there is the name failing to resolve — see
            // [`Prompt::growing`]. A circle being drawn names nothing the
            // document holds at all, which is the point of it.
            Asking::Extrude { .. } | Asking::Circle { .. } => None,
        }
    }

    /// The dimension being restated, where that is what this is about.
    ///
    /// What the drawing skips when it lays its marks out — the field is drawn
    /// where the mark would be, and both at once would be one on top of the
    /// other.
    pub(crate) fn marks(&self) -> Option<Part> {
        match &self.about {
            Asking::Dimension { part } => Some(*part),
            Asking::Radius { .. } | Asking::Extrude { .. } | Asking::Circle { .. } => None,
        }
    }

    /// The solid this form is deciding, as the drawing should show it now.
    ///
    /// `None` for a form about anything else, for one whose depth does not read
    /// as a number yet — a half-typed "1." is not a depth, and a solid that
    /// flickered away between the digits of one would be worse than a solid
    /// that waits — and for one whose region the drawing no longer holds.
    ///
    /// Resolved against `models` every time rather than remembered, which is
    /// what the [`Profile`] is for: a position is only good for the arrangement
    /// it was read from, and a form outlives several.
    pub(crate) fn growing(&self, models: Models<'_>) -> Option<Growing> {
        let profile = self.about.extruding()?;
        Some(Growing {
            sketch: profile.sketch(),
            region: profile.face_of(models)?,
            distance: self.says(0)?,
        })
    }

    /// What the `nth` field says as a number, or `None` where it says something
    /// that is not one.
    ///
    /// A `Result` would be the wrong shape: text that is not a number is not a
    /// failure to report but a draft that is not finished, and what a commit
    /// does with one is refuse to commit. Untrusted input in the sense the
    /// error-handling rules mean it — a person typing — but the only thing that
    /// can be said about it is whether it parses.
    ///
    /// Where a formula goes. Today a field holds a literal and this is
    /// [`str::parse`]; when it holds an expression this is where it is
    /// evaluated, and every caller already treats `None` as "not yet".
    pub(crate) fn value(&self, nth: usize) -> Option<f64> {
        self.fields.get(nth)?.draft.trim().parse().ok()
    }

    /// Put `to` in the `nth` field, as the drawing's own handle for it moved.
    ///
    /// Written over the draft rather than beside it, so what the field shows and
    /// what the gesture said are one number: a drag and the keyboard are two
    /// ways of saying the same thing, and a form that kept them apart would have
    /// to decide which one Enter meant.
    pub(crate) fn write(&mut self, nth: usize, to: f64) {
        let Some(field) = self.fields.get_mut(nth) else {
            return;
        };
        field.draft.clear();
        let _ = write!(field.draft, "{to:.*}", DECIMALS);
    }

    /// Show `to` in the `nth` field while nobody has typed into it.
    ///
    /// What a pointer merely *moving* says. Incidental, where
    /// [`Prompt::write`] is deliberate: a hover offers a value and a drag sets
    /// one, and only the second may overwrite what has been typed.
    pub(crate) fn suggest(&mut self, nth: usize, to: f64) {
        let Some(field) = self.fields.get_mut(nth) else {
            return;
        };
        field.suggested.clear();
        let _ = write!(field.suggested, "{to:.*}", DECIMALS);
    }

    /// What the `nth` field says, where somebody has typed it.
    ///
    /// `None` while the draft is empty, which is the pointer still driving —
    /// see [`Field::suggested`]. What asks is whatever the number *moves*: a
    /// band that went on following the cursor after a radius was typed would be
    /// showing one number while the form showed another.
    pub(crate) fn typed(&self, nth: usize) -> Option<f64> {
        let field = self.fields.get(nth)?;
        if field.draft.trim().is_empty() {
            return None;
        }
        self.value(nth)
    }

    /// What the `nth` field currently means, whoever put it there.
    ///
    /// [`Prompt::typed`] where somebody has typed, and the pointer's own
    /// suggestion where nobody has. What *commits* asks this rather than the
    /// draft: a form showing a number and refusing to accept it because the
    /// number came from the pointer would be a form arguing with what it says.
    pub(crate) fn says(&self, nth: usize) -> Option<f64> {
        self.typed(nth)
            .or_else(|| self.fields.get(nth)?.suggested.trim().parse().ok())
    }

    /// Show the form, and put what it asked for in `intents`.
    ///
    /// Shows and does not act, like every other control the application draws:
    /// what a field says reaches the document as a [`Change`], and closing the
    /// form is a [`Choice::Ask`].
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        stands: Stands,
        models: Models<'_>,
        intents: &mut Intents,
    ) {
        // Taken after the caller has established there is somewhere to stand,
        // so a form the projection never drew has not used up the one frame
        // that takes focus.
        let opening = !std::mem::replace(&mut self.shown, true);
        let done = match stands {
            Stands::Over(at) => self.over(ui, at, opening),
            Stands::Beside(anchor) => self.beside(ui, anchor, opening),
        };
        // Outside the bodies, because a value is read off a draft the widget
        // has only just finished writing.
        match done {
            // A draft that is not a number is not finished, so Enter on one is
            // not a refusal to report but a key that does nothing. The form
            // stays open, which is what says so.
            Some(Done::Commit) => self.commit(models, intents),
            Some(Done::Cancel) => intents.push(Choice::Ask(None)),
            None => {}
        }
    }

    /// Whether losing focus is this form's cancel.
    ///
    /// It is where clicking away is the only way out, and it is *not* where the
    /// form carries its own buttons: an extrude's depth is dragged by an arrow
    /// in the drawing, and a form that threw itself away when the pointer went
    /// to that arrow would be one you could never drag.
    fn blurs(&self) -> bool {
        match self.about {
            Asking::Dimension { .. } => true,
            Asking::Radius { .. } | Asking::Extrude { .. } | Asking::Circle { .. } => false,
        }
    }

    /// What the widget's three signals come to, given how this form is
    /// dismissed.
    ///
    /// **Cancel is tested before submit**, which is what a commit-on-blur form
    /// owes itself: Escape blurs as well as cancelling, so asking about focus
    /// first could not tell a cancel from a click away — and where it blurs the
    /// two mean the same thing, which is why they share an arm.
    fn resolve(&self, said: Said) -> Option<Done> {
        if said.cancelled || (said.lost_focus && self.blurs()) {
            Some(Done::Cancel)
        } else if said.submitted {
            Some(Done::Commit)
        } else {
            None
        }
    }

    /// Ask the document for what the form says, and for the form to close.
    fn commit(&self, models: Models<'_>, intents: &mut Intents) {
        match &self.about {
            Asking::Dimension { part } => {
                let Some(to) = self.says(0) else {
                    return;
                };
                let (Some(sketch), Some(Entity::Constraint(constraint))) =
                    (part.sketch(), part.entity())
                else {
                    unreachable!("a dimension form is only ever opened over one");
                };
                intents.push(Change::Resize {
                    sketch,
                    constraint,
                    to,
                });
                // One gesture, one step to take back — the same signal a
                // scrub's release gives, because `Resize` coalesces and this is
                // what closes the run of them.
                intents.push(Step::Release);
            }
            // The one commit that *makes* geometry. A radius rather than a rim
            // to put it at, so the rim is one the plane's own x-axis puts there
            // — bare plane, holding to nothing, which is what a number typed
            // rather than a place clicked has to mean.
            Asking::Circle { sketch, center } => {
                let Some(radius) = self.says(0).filter(|radius| *radius > 0.0) else {
                    return;
                };
                let Some(drawing) = models.at(*sketch).map(Model::drawing) else {
                    return;
                };
                let middle = drawing.at(*center);
                let rim = Anchor::At(middle + drawing.plane().x.as_vec3() * radius as f32);
                intents.push(Change::AddCircle {
                    sketch: *sketch,
                    center: *center,
                    rim,
                });
                // Back to its first click, which is where the tool would be
                // after a second one. A form is the other way of giving the
                // same answer, so it leaves the tool in the same place.
                intents.push(Choice::Hold(Tool::Circle { center: None }));
            }
            Asking::Radius { sketch, circle } => {
                let Some(radius) = self.says(0) else {
                    return;
                };
                // A relation rather than a restatement: the circle has no
                // radius to restate until this puts one on it, which is the
                // whole of what the offer was short of.
                intents.push(Change::Constrain {
                    sketch: *sketch,
                    constraint: Constraint::Radius {
                        circle: *circle,
                        dimension: Dimension::new(radius),
                    },
                });
            }
            Asking::Extrude { profile } => {
                let Some(distance) = self.says(0) else {
                    return;
                };
                // Resolved here rather than carried, for the reason the form
                // holds a name at all — and `None` is a region the drawing no
                // longer has, which is nothing to grow and nothing to report.
                let sketch = profile.sketch();
                let Some(region) = profile.face_of(models) else {
                    return;
                };
                // The one step the whole operation costs. The solid has been on
                // screen since the form opened, drawn from this form's own
                // reading — see [`Asking::Extrude`] — so what reaches the
                // timeline is the depth that was settled on rather than a zero
                // that was then carried.
                intents.push(Change::Extrude {
                    sketch,
                    region,
                    distance,
                });
            }
        }
        intents.push(Choice::Ask(None));
    }

    /// What a field is recorded under.
    ///
    /// **Named rather than derived from the call site**, and the reason is when
    /// focus has to be asked for. A form that has just opened *is* the thing
    /// being typed into, so it has to come up focused — and focus is read at
    /// the top of the widget's own pass. An `auto_id` is only knowable from the
    /// response, which is a pass too late: the field would paint once with no
    /// caret and nothing picked out, and — since changing focus asks for no
    /// frame of its own — stay that way until some unrelated event woke one.
    ///
    /// A session has one form open at a time, so a name and the field's place
    /// in it are between them enough to tell any two apart.
    fn field_id(nth: usize) -> WidgetId {
        WidgetId::from_hash(("catcad.prompt.field", nth))
    }

    /// The number as a field will shape it.
    ///
    /// Built off the same face the field is styled with, so what this measures
    /// is what that draws. Unbounded and single-line: a dimension is one run
    /// that the box is sized to rather than the other way about.
    fn run(&self, nth: usize) -> TextRun<'_> {
        let font = mark_font();
        TextRun {
            text: &self.fields[nth].draft,
            font_size_px: font.size_px,
            line_height_px: font.line_height_px,
            wrap: TextWrap::Scroll,
            align: Align::CENTER,
            family: font.family,
            weight: font.weight,
            max_width_px: None,
        }
    }

    /// Whether the form carries its own confirm and cancel.
    ///
    /// The same question [`Prompt::blurs`] answers from the other side, and
    /// deliberately its opposite: a form is dismissed by its buttons or by
    /// clicking away, never by both and never by neither.
    fn answered(&self) -> bool {
        !self.blurs()
    }

    /// One field standing exactly where the drawing would have put the mark it
    /// replaces.
    ///
    /// A `Canvas` filling the view, so the field's position is measured from
    /// the same corner the projection answers in. It senses nothing, so every
    /// press it does not contain falls through to the viewport beneath — which
    /// is what makes clicking away from the field a click on the drawing as
    /// well as a blur.
    fn over(&mut self, ui: &mut Ui, centre: Vec2, opening: bool) -> Option<Done> {
        // Measured before the field is shown, because where its corner goes
        // depends on how wide its number comes out. The same shaper the widget
        // itself will use, asked the same question, so the two cannot answer
        // differently.
        let width = ui.probe_text(self.run(0)).size().w;
        let origin = self.placed_over(centre, width);
        let Self { fields, look, .. } = self;
        let id = Self::field_id(0);
        let said = Panel::canvas()
            .id_salt("prompt.over")
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui, |ui| {
                // **Before the field is shown, not after**, so that the pass
                // recording it is the pass that reads the focus — see
                // [`Prompt::field_id`]. Only on the frame it opens: after that
                // focus is palantir's, and losing it is how clicking away
                // cancels.
                if opening {
                    ui.request_focus(Some(id));
                }
                let shown = TextEdit::new(&mut fields[0].draft)
                    .id(id)
                    .style(look)
                    .select_all_on_focus()
                    .text_align(Align::CENTER)
                    .size((Sizing::HUG, Sizing::HUG))
                    .position(origin)
                    .show(ui);
                Said::of(&shown)
            })
            .inner;
        self.resolve(said)
    }

    /// Every field and both answers, in a row standing clear of `anchor`.
    ///
    /// [`Popup`] does the placing: it resolves the row against the anchor from
    /// its own measured size and flips or shifts it to stay on the surface, so
    /// a face near the bottom of the view gets its form above instead. Nothing
    /// here measures anything.
    ///
    /// [`ClickOutside::PassThrough`] because this annotates rather than
    /// interrupts. A modal popup installs a click-eater over the whole surface
    /// and claims the keyboard for every layer below, which for a form whose
    /// value is *also* dragged by an arrow in the drawing would mean a form you
    /// could never drag against.
    fn beside(&mut self, ui: &mut Ui, anchor: Rect, opening: bool) -> Option<Done> {
        // Stood clear of what it is about rather than against it. [`Popup`]
        // places a body flush with its anchor, and flush with the outline of
        // the thing being measured is on top of it — so the anchor is grown by
        // the gap the form should keep, which is also the gap the fitting then
        // preserves when it flips the form to the other side.
        let anchor = Rect::new(
            anchor.min.x - STANDS_CLEAR,
            anchor.min.y - STANDS_CLEAR,
            anchor.size.w + STANDS_CLEAR * 2.0,
            anchor.size.h + STANDS_CLEAR * 2.0,
        );
        let answerable = self.answered();
        let blurs = self.blurs();
        let Self {
            fields,
            look,
            goes,
            stops,
            ..
        } = self;
        let mut said = Said::default();
        let mut answered = None;
        Popup::below(anchor)
            .id_salt("prompt.beside")
            .click_outside(ClickOutside::PassThrough)
            .show(ui, |ui, _| {
                // **Every frame, not only the first.** A form standing against a
                // gesture moves with what it is measuring, so there is no
                // clicking back into one that has lost focus — it is not where
                // it was by the time the press lands. A form that is dismissed
                // by *clicking away* is the opposite case and must not be held,
                // which is the same question [`Prompt::blurs`] answers.
                if opening || !blurs {
                    ui.request_focus(Some(Self::field_id(0)));
                }
                // A column, so the answers sit under what they answer rather
                // than running off to one side of it — two buttons on the end
                // of a row put the whole form wider than the number it is
                // about, and it is standing on a drawing.
                Panel::vstack().id_salt("form").gap(GAP).show(ui, |ui| {
                    Panel::hstack().id_salt("row").gap(GAP).show(ui, |ui| {
                        for (nth, field) in fields.iter_mut().enumerate() {
                            if !field.label.is_empty() {
                                // Against the middle of the box beside it, not
                                // the top of the row: a field is taller than
                                // the word naming it, so a label left to its
                                // own devices sits on the field's top edge.
                                Text::new(field.label)
                                    .id_salt(field.label)
                                    .align(Align::v(VAlign::Center))
                                    .show(ui);
                            }
                            let shown = TextEdit::new(&mut field.draft)
                                .id(Self::field_id(nth))
                                .style(look)
                                .select_all_on_focus()
                                // Cloned because [`TextEdit::placeholder`]
                                // takes a `Cow<'static, str>` and this string
                                // is the form's, not the program's — a borrow
                                // cannot satisfy that lifetime. One short
                                // string a frame, and the only way in.
                                .placeholder(field.suggested.clone())
                                .text_align(Align::CENTER)
                                .size((Sizing::HUG, Sizing::HUG))
                                .show(ui);
                            said = said.and(Said::of(&shown));
                        }
                    });
                    // Only where this is how the form is dismissed. A form that
                    // blurs shut has no use for them, and two buttons that were
                    // not the way out would be two buttons lying about it.
                    if answerable {
                        // Under the far end of the row rather than its start,
                        // so the answers line up with the number they are about
                        // instead of with the word naming it.
                        Panel::hstack()
                            .id_salt("answers")
                            .gap(GAP)
                            .align(Align::h(HAlign::Right))
                            .show(ui, |ui| {
                                for (glyph, theme, answer) in [
                                    (look::CONFIRM, &*goes, Done::Commit),
                                    (look::CANCEL, &*stops, Done::Cancel),
                                ] {
                                    let pressed = Button::new()
                                        .id_salt(glyph)
                                        .label(glyph)
                                        .style(theme)
                                        .size((
                                            Sizing::fixed(look::ANSWER_SIDE),
                                            Sizing::fixed(look::ANSWER_SIDE),
                                        ))
                                        .show(ui)
                                        .left
                                        .clicked();
                                    if pressed {
                                        answered = Some(answer);
                                    }
                                }
                            });
                    }
                });
            });
        // The buttons first: a press on one is an answer whatever the fields
        // made of the same frame, and a field that lost focus *to* the button
        // must not cancel out from under it.
        answered.or_else(|| self.resolve(said))
    }

    /// Where to put a field's own corner so that its glyphs land exactly where
    /// the mark's did.
    ///
    /// **What keeps a number from jumping as it becomes editable.** `centre` is
    /// the middle of the box the mark's own glyphs sat in — the drawing worked
    /// that out, up the run's frame and through however many lanes it rose. What
    /// is left here is the field's own furniture: it hangs its text off its
    /// corner by the ring it is drawn with, the padding inside that, and the
    /// caret's room at the trailing edge, so the corner is that middle with all
    /// of it backed off.
    ///
    /// Read off the theme this module authored rather than written out again,
    /// so a box restyled in [`look::field`] moves the field with it instead of
    /// leaving this behind.
    ///
    /// The text's own width cancels: both centre their text on the same point,
    /// so both land half a run to the left of it whatever the run measures. Only
    /// the caret's room breaks the symmetry, being reserved on one side.
    fn placed_over(&self, centre: Vec2, width: f32) -> Vec2 {
        // Off `normal`, and safe to be: palantir pins the ring's width across
        // every state so that focus changes its colour without moving the inner
        // rect — which is the same shift this call exists to avoid, one state
        // further along.
        let ring = self
            .look
            .looks
            .normal
            .background
            .as_ref()
            .map_or(0.0, |bg| bg.stroke.width);
        let inset = Vec2::new(self.look.padding.horiz(), self.look.padding.vert()) * 0.5
            + Vec2::splat(ring);
        Vec2::new(
            centre.x - width * 0.5 - inset.x - self.look.caret_width * 0.5,
            centre.y - mark_font().line_height_px * 0.5 - inset.y,
        )
    }
}

/// What a harness measuring the open form reaches past it for.
///
/// A field's id is the form's own business — it is how focus is asked for
/// before the widget reads it — and no part of the application needs to know
/// one. A test does: it finds where the box was laid out to check it stands
/// where the mark did.
///
/// Gated on `test` alone rather than on `internals` beside it: the one caller
/// is a unit test, and the wider gate would leave this dead in every build that
/// turned the feature on without turning tests on.
#[cfg(test)]
pub(crate) mod internals {
    use palantir::WidgetId;

    use crate::prompt::Prompt;

    impl Prompt {
        /// What the `nth` field is recorded under.
        pub(crate) fn nth_field_id(nth: usize) -> WidgetId {
            Self::field_id(nth)
        }
    }
}

#[cfg(test)]
mod tests;
