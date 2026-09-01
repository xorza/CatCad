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
//! belong to the [`TextEdit`] a field is shown through — its own furniture
//! included, which is what puts a field's glyphs back on the pixels the mark's
//! were read at. *Fitting the form on screen* belongs to [`Popup`], which
//! resolves a body against an anchor and flips or shifts it to stay on the
//! surface. What is left is the two things neither can know: where in the world
//! the form is about, and what pressing Enter means.

use glam::Vec2;
use palantir::{
    Align, ClickOutside, Configure, HAlign, Panel, Popup, Rect, Size, Sizing, Spacing, Text,
    TextEdit, TextRun, TextStyle, TextWrap, Ui, VAlign, WidgetId,
};
use silverpoint::{Entity, Operation, Sector, SegmentId};
use std::fmt::Write;

use crate::control::chip::Chip;
use crate::control::pill::{self, Pill};
use crate::drawing::anchor::Anchor;
use crate::intent::change::Change;
use crate::intent::{Choice, Intents, Opening, Step};
use crate::look::Theme;
use crate::look::icons::Icons;
use crate::marked::{self, Marked};
use crate::model::{Model, Models};
use crate::paint::growing::Growing;
use crate::paint::{DECIMALS, MARK_FONT};
use crate::part::Part;
use crate::profile::Profile;
use crate::timeline::{Axle, FeatureId, Sweep};
use crate::tool::Tool;

/// What a form is about, and so what committing it asks for.
///
/// One arm per operation, which is the same shape [`Tool`] and [`Change`]
/// already take. It is what turns "the user pressed Enter" into something the
/// document understands, and there is no way to know that without knowing which
/// operation is in hand.
///
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
    /// A solid being grown off a region, *before* it reaches the timeline.
    ///
    /// Held here rather than created at zero and carried, because a
    /// a prism was a reading rather than a thing the
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
    /// **What it does with what stands is here rather than on a field**,
    /// because it is a choice among three and not a number: a field holds text
    /// somebody types and this is a control somebody sets. It starts as a join,
    /// which is what a second solid means nine times in ten and the only one
    /// whose answer is the extrude itself where nothing stands yet.
    Extrude {
        profile: Profile,
        operation: Operation,
    },
    /// A solid being spun off a region, *before* it reaches the timeline.
    ///
    /// The extrude's twin, and it holds a [`Profile`] for the same reason: the
    /// viewport stays live under an open form, so a position would name a
    /// different region by the time it committed.
    ///
    /// **Two fields where an extrude has one**, which is what a sector asks
    /// for: where round the line the turn starts, and how far it goes. Both in
    /// degrees — see [`Prompt::sector`], which is the one place they become the
    /// radians the kernel takes.
    Revolve {
        profile: Profile,
        axis: SegmentId,
        operation: Operation,
    },
}

impl Asking {
    /// Where the operation this form is deciding is held, and `None` for a form
    /// that decides none.
    ///
    /// **Two of the four forms carry one and two do not**, and both the
    /// reading and the setting have to agree about which. Beside its own `mut`
    /// twin rather than at the two places that ask, so that a fifth form added
    /// to the enum meets them together — see [`Asking::doing_mut`].
    fn doing(&self) -> Option<&Operation> {
        match self {
            Self::Extrude { operation, .. } | Self::Revolve { operation, .. } => Some(operation),
            Self::Dimension { .. } | Self::Circle { .. } => None,
        }
    }

    /// The same, to be set by the control that shows it — see [`Asking::doing`].
    fn doing_mut(&mut self) -> Option<&mut Operation> {
        match self {
            Self::Extrude { operation, .. } | Self::Revolve { operation, .. } => Some(operation),
            Self::Dimension { .. } | Self::Circle { .. } => None,
        }
    }

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
            Asking::Extrude { profile, .. } => Some(profile),
            Asking::Dimension { .. } | Asking::Circle { .. } | Asking::Revolve { .. } => None,
        }
    }

    /// What this form calls itself, over the fields it asks for.
    ///
    /// **`None` for a dimension, and that is the placement rather than an
    /// omission.** A dimension's field stands *over* the mark it replaces — see
    /// [`Stands`] — so a caption there would be a second thing in a place that
    /// holds exactly one, and it would cover the geometry the number is about.
    /// Every form that stands *beside* what it is about has room to say what it
    /// is, and nothing else on one does: two fields and three chips are the
    /// same two fields and three chips whichever sweep they are for.
    fn named(&self) -> Option<Marked> {
        match self {
            Asking::Dimension { .. } => None,
            Asking::Circle { .. } => Some(marked::CIRCLE),
            Asking::Extrude { .. } => Some(marked::EXTRUDE),
            Asking::Revolve { .. } => Some(marked::REVOLVE),
        }
    }

    /// The region a solid is being raised from, whichever way it is raised.
    ///
    /// Wider than [`Asking::extruding`] beside it, and the two are not one:
    /// what a *depth arrow* needs is a depth, which only an extrude has, where
    /// what the drawing shows is a solid either way.
    fn raising(&self) -> Option<&Profile> {
        match self {
            Asking::Extrude { profile, .. } | Asking::Revolve { profile, .. } => Some(profile),
            Asking::Dimension { .. } | Asking::Circle { .. } => None,
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
    /// What is *in* the footprint is the caller's: a form open about a solid
    /// being grown is handed the region and everywhere the handle carrying its
    /// value can go, so that the fitting cannot put the form on the handle.
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

/// What an angle a form asks for is stated in.
///
/// Degrees, which is what a person types. [`Prompt::sector`] is the one place
/// they become the radians the kernel takes, and this is the one place the form
/// says so — stated once, so what is shown and what is converted cannot come to
/// mean two things.
const DEGREES: &str = "\u{b0}";

/// How tall the row naming the form is, and how far across the mark on it
/// reaches, in logical pixels.
///
/// Smaller than a chip's artwork, because this is a caption's mark rather than
/// a control: what it stands beside is the least lettering the overlay sets —
/// see [`Chrome::caption_text`](crate::look::chrome::Chrome::caption_text).
const NAMING_ROW: f32 = 15.0;
const NAMING_ICON: f32 = 12.0;

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

/// One value a form opens asking for.
///
/// **Named rather than a tuple in a row**, on the terms [`Said`] states about
/// three bools: two of these are `&'static str`, so they could change places
/// without a word from the compiler — and a label swapped with a unit is a
/// field called `\u{b0}`.
#[derive(Debug, Clone, Copy)]
struct Asks {
    label: &'static str,
    unit: &'static str,
    seed: Seed,
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

/// A turn being decided: the drawing whose line it spins about, which segment
/// that line is, and how much of a turn it currently reads.
///
/// [`Carrying`]'s twin, and it carries the same shape of answer for the same
/// reason: what a press on the handle needs, in one piece. The line is named
/// rather than resolved, on the terms that one states — a handle is grabbed
/// before anything asks the drawing anything.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Turning {
    pub(crate) sketch: FeatureId,
    pub(crate) axis: SegmentId,
    pub(crate) sector: Sector,
}

/// One value being typed, and what to call it where two are asked for at once.
#[derive(Debug)]
struct Field {
    /// Empty where the form asks for one thing and standing over it says which
    /// — a dimension's field is the number, and a word beside it would be a
    /// word the drawing never had.
    label: &'static str,
    /// What the number is in, set after the box in the size the overlay captions
    /// with.
    ///
    /// **The one thing on a form nothing else could say.** Both of a revolve's
    /// fields are degrees — [`Prompt::sector`] is where they become the radians
    /// the kernel takes — and a box reading `360.00` over a solid says nothing
    /// about which. Empty where the value is a length, because the document has
    /// no unit for one yet and a guess would be worse than the silence.
    unit: &'static str,
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

impl Field {
    /// Whether the keyboard is driving this field rather than the pointer.
    ///
    /// **Exactly when the draft is not empty** — see [`Field::suggested`],
    /// which is where that state machine is stated. One method rather than the
    /// test written out at each reader, because a reader that spelled it as a
    /// *fallback* — try the draft, take the suggestion if it does not parse —
    /// would hand the pointer back the moment a draft stopped reading as a
    /// number, so a half-typed `1.` would show the value the pointer last
    /// offered and a draft of `abc` would read as the offer outright.
    fn driving(&self) -> bool {
        !self.draft.trim().is_empty()
    }

    /// The number the pointer is offering, where it is offering one.
    ///
    /// Read straight rather than through [`Prompt::value`], which reads a
    /// *draft*. The two look alike today and are not the same question: a draft
    /// is what somebody typed and is where a formula will one day be evaluated,
    /// where this is what [`Prompt::suggest`] wrote with `{:.*}` and is a
    /// literal by construction.
    fn offered(&self) -> Option<f64> {
        self.suggested.trim().parse().ok()
    }
}

/// A form open against the drawing: what it is about, and what has been typed.
#[derive(Debug)]
pub(crate) struct Prompt {
    /// Which opening this is, so two forms are told apart by something that
    /// cannot be equal by accident.
    ///
    /// What the drawing compares one frame's solid against the last one's with
    /// — see [`Growing`]. A form closing and another opening on other regions
    /// moves nothing in the document, so the revision says nothing, and two
    /// extrudes both opening at no depth and a join would otherwise read alike.
    form: Form,
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
    /// The form `opening` asks for.
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
    /// `form` is which opening this is, minted by the session — see
    /// [`Prompt::form`], which is what the drawing tells two forms apart by.
    ///
    /// Nothing is resolved here any more. A position among the faces becomes a
    /// name where the regions are *picked*, which is what lets the request
    /// carry the name — see [`Opening::Extrude`].
    pub(crate) fn opening(opening: Opening, form: Form) -> Self {
        match opening {
            Opening::Dimension { part, from } => Self::on(
                form,
                Asking::Dimension { part },
                [Asks {
                    label: "",
                    unit: "",
                    seed: Seed::Stated(from),
                }],
            ),
            // Offered rather than stated, like every form that *makes*
            // something: the pointer has the value until somebody types one,
            // and the field shows whichever is speaking. See [`Seed`].
            Opening::Circle { sketch, center } => Self::on(
                form,
                Asking::Circle { sketch, center },
                [Asks {
                    label: "Radius",
                    unit: "",
                    seed: Seed::Offered(0.0),
                }],
            ),
            // At no depth at all, which is where the ask starts: the solid is on
            // screen from the moment the form opens, and a zero-depth prism is a
            // well-formed one.
            Opening::Extrude { profile } => Self::on(
                form,
                Asking::Extrude {
                    profile,
                    operation: Operation::Join,
                },
                [Asks {
                    label: "Depth",
                    unit: "",
                    seed: Seed::Offered(0.0),
                }],
            ),
            // A whole turn from the drawing's own place, which is where the
            // ask starts: the ring is on screen whole from the moment the form
            // opens, and what somebody types cuts it down. A turn of nothing
            // would be nothing to see, where a solid at no depth is the
            // extrude's honest answer to the same question.
            //
            // Off the kernel's own whole turn rather than the number, so the
            // two cannot come to mean different amounts — [`Prompt::sector`] is
            // where the form's degrees meet its radians.
            Opening::Revolve { profile, axis } => Self::on(
                form,
                Asking::Revolve {
                    profile,
                    axis,
                    operation: Operation::Join,
                },
                [
                    Asks {
                        label: "Start",
                        unit: DEGREES,
                        seed: Seed::Offered(0.0),
                    },
                    Asks {
                        label: "Turn",
                        unit: DEGREES,
                        seed: Seed::Offered(Sector::WHOLE.sweep.to_degrees()),
                    },
                ],
            ),
        }
    }

    /// Open a form for `about`, seeded with `values`.
    ///
    /// Which of the two inputs each field starts with is [`Seed`]'s to say.
    /// Reached through [`Prompt::opening`], which is what decides both for each
    /// kind of form; each arm stands up its own rather than answering with a
    /// pair the match then builds from, because the seeds are arrays and two
    /// arms of different *lengths* would not agree on a type.
    ///
    /// **A form with no field has to carry its own way out**, which is the one
    /// thing asserted here. Nothing to type into is fine, and a form dismissed
    /// by *clicking away* and holding no chip would be one nothing could
    /// answer. That is [`Prompt::blurs`], so the two are asked together.
    ///
    /// And such a form stands *beside* what it is about, never over it:
    /// [`Prompt::over`] and [`Prompt::run`] index the first field rather than
    /// carry an `Option` for a state a dimension cannot be in.
    fn on<const N: usize>(form: Form, about: Asking, values: [Asks; N]) -> Self {
        let made = Self {
            form,
            about,
            fields: values
                .iter()
                .map(|&Asks { label, unit, seed }| {
                    let said = |value: f64| format!("{value:.*}", DECIMALS);
                    match seed {
                        Seed::Stated(value) => Field {
                            label,
                            unit,
                            draft: said(value),
                            suggested: String::new(),
                        },
                        Seed::Offered(value) => Field {
                            label,
                            unit,
                            draft: String::new(),
                            suggested: said(value),
                        },
                    }
                })
                .collect(),
            shown: false,
        };
        debug_assert!(
            N > 0 || !made.blurs(),
            "a form that asks for nothing and has no chip to answer with",
        );
        made
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
            depth: self.shows(0)?,
        })
    }

    /// The turn this form is deciding, where deciding one is what it is about.
    ///
    /// [`Prompt::carrying`]'s twin. What it shows rather than what it says, on
    /// the same terms: the handle has to stand where the solid is drawn, and a
    /// draft mid-word is a number nothing should move against.
    pub(crate) fn turning(&self) -> Option<Turning> {
        let Asking::Revolve { profile, axis, .. } = &self.about else {
            return None;
        };
        Some(Turning {
            sketch: profile.sketch(),
            axis: *axis,
            sector: self.sector(Self::shows)?,
        })
    }

    /// The dimension being restated, where that is what this is about.
    ///
    /// Read by the drawing and by a prune, which want one answer for the two
    /// halves of a single reason: the field is drawn where the mark would be,
    /// so the drawing leaves that mark out rather than stacking two numbers on
    /// each other, and a form standing where a mark *was* is a form over
    /// geometry an undo can take away.
    ///
    /// They stay one question only while no form is about a part it does not
    /// also mark — a form naming something the document holds names it by
    /// standing over it, and one standing over nothing is about nothing the
    /// document can lose.
    pub(crate) fn marks(&self) -> Option<Part> {
        match &self.about {
            Asking::Dimension { part } => Some(*part),
            Asking::Extrude { .. } | Asking::Circle { .. } | Asking::Revolve { .. } => None,
        }
    }

    /// The solid this form is deciding, as the drawing should show it now.
    ///
    /// `None` for a form about anything else, and for one whose region the
    /// drawing no longer holds. **Not** for a depth mid-word: a form opened on
    /// an offer always has a number to draw at, because a solid flickering away
    /// between the two keystrokes of `-2` would be worse than one that waits.
    /// That is [`Prompt::shows`]'s doing, and it is why this reads that rather
    /// than [`Prompt::says`].
    ///
    /// Resolved against `models` every time rather than remembered, which is
    /// what the [`Profile`] is for: a position is only good for the arrangement
    /// it was read from, and a form outlives several.
    pub(crate) fn growing(&self, models: Models<'_>) -> Option<Growing<'_>> {
        let profile = self.about.raising()?;
        let sweep = match &self.about {
            Asking::Extrude { .. } => Sweep::Carried(self.shows(0)?),
            Asking::Revolve { axis, .. } => Sweep::Spun {
                axle: Axle::of(models.at(profile.sketch())?.drawing().sketch(), *axis),
                sector: self.sector(Self::shows)?,
            },
            Asking::Dimension { .. } | Asking::Circle { .. } => return None,
        };
        Some(Growing {
            form: self.form,
            profile,
            sweep,
            operation: self.about.doing().copied()?,
        })
    }

    /// How much of a turn the form's two fields describe, in the radians the
    /// kernel takes.
    ///
    /// **Degrees on the form and radians below it**, and this is the one place
    /// the two meet. Every other number a form asks for is a length in the
    /// sketch's own units, so an angle is the first with a unit of its own —
    /// stated once here rather than at each reader, which is what keeps a form
    /// showing one number and a step built from another.
    ///
    /// `read` is which reading the caller wants, the two being different
    /// questions: what the drawing *shows* while somebody is still typing, and
    /// what a commit *says*. See [`Prompt::shows`] and [`Prompt::says`].
    fn sector(&self, read: fn(&Self, usize) -> Option<f64>) -> Option<Sector> {
        Some(Sector {
            from: read(self, 0)?.to_radians(),
            sweep: read(self, 1)?.to_radians(),
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
    ///
    /// **The draft and nothing else**, which is what tells it from
    /// [`Prompt::says`] and [`Prompt::shows`] below: an empty draft reads as no
    /// number here like any other text that is not one, so a caller asking
    /// whether somebody has typed a number is asking exactly this. What asks is
    /// whatever the number *moves* — a band that went on following the cursor
    /// after a radius was typed would be showing one number while the form
    /// showed another.
    pub(crate) fn value(&self, nth: usize) -> Option<f64> {
        self.fields.get(nth)?.draft.trim().parse().ok()
    }

    /// Put `to` in the `nth` field, as the drawing's own handle for it moved.
    ///
    /// Written over the draft rather than beside it, so what the field shows and
    /// what the gesture said are one number: a drag and the keyboard are two
    /// ways of saying the same thing, and a form that kept them apart would have
    /// to decide which one Enter meant.
    ///
    /// **A field that is not there is nothing to write, not a bug** — which is
    /// why this and its neighbours reach through `get` where the form's own
    /// readings index. The index arrives on a replayed [`Choice::Set`], and a
    /// drag that outlived the form it was writing into lands on whatever is
    /// open now.
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

    /// What the `nth` field currently means, whoever put it there.
    ///
    /// The draft where somebody has typed, and the pointer's own suggestion
    /// where nobody has. What *commits* asks this rather than the draft: a form
    /// showing a number and refusing to accept it because the number came from
    /// the pointer would be a form arguing with what it says.
    ///
    /// **Which of the two, and then that one** — never one and then the other
    /// if the first came back empty. [`Field::driving`] is what decides, and
    /// asking the other way round is what would let Enter on a draft of `abc`
    /// commit whatever the pointer last offered: a number nobody typed, behind
    /// text that is on screen and says otherwise.
    ///
    /// So a draft that does not read as a number means *nothing yet*, and a
    /// commit reading `None` leaves the form open. What the drawing shows
    /// meanwhile is a different question — see [`Prompt::shows`].
    pub(crate) fn says(&self, nth: usize) -> Option<f64> {
        let field = self.fields.get(nth)?;
        if field.driving() {
            return self.value(nth);
        }
        field.offered()
    }

    /// What the drawing shows for the `nth` field while the form is open.
    ///
    /// [`Prompt::says`] where the field means something, and the pointer's own
    /// offer where it does not — which is the one place the two questions come
    /// apart, and each has to be asked by name. A draft mid-word means nothing
    /// to *commit*; a solid that blinked out between the two keystrokes of `-2`
    /// would be worse than one that waits where the pointer left it.
    pub(crate) fn shows(&self, nth: usize) -> Option<f64> {
        self.says(nth).or_else(|| self.fields.get(nth)?.offered())
    }

    /// Show the form, and put what it asked for in `intents`.
    ///
    /// Shows and does not act, like every other control the application draws:
    /// what a field says reaches the document as a [`Change`], and closing the
    /// form is a [`Choice::Ask`].
    pub(crate) fn show(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        icons: &Icons,
        stands: Stands,
        models: Models<'_>,
        intents: &mut Intents,
    ) {
        // Taken after the caller has established there is somewhere to stand,
        // so a form the projection never drew has not used up the one frame
        // that takes focus.
        let opening = !std::mem::replace(&mut self.shown, true);
        let done = match stands {
            Stands::Over(at) => self.over(ui, theme, at, opening),
            Stands::Beside(anchor) => self.beside(ui, theme, icons, anchor, opening),
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
    /// form carries its own chips: an extrude's depth is dragged by an arrow
    /// in the drawing, and a form that threw itself away when the pointer went
    /// to that arrow would be one you could never drag.
    ///
    /// Read the other way round it is whether the form carries its own confirm
    /// and cancel, which is the same bit and deliberately so: a form is
    /// dismissed by its chips or by clicking away, never by both and never by
    /// neither.
    fn blurs(&self) -> bool {
        match self.about {
            Asking::Dimension { .. } => true,
            Asking::Extrude { .. } | Asking::Circle { .. } | Asking::Revolve { .. } => false,
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
            Asking::Extrude { profile, operation } => {
                let Some(distance) = self.says(0) else {
                    return;
                };
                // The one step the whole operation costs, and it carries the
                // name the form has held since it opened — see
                // [`Asking::Extrude`]. So what reaches the timeline is the
                // depth that was settled on rather than a zero then carried,
                // and the regions are the ones that were picked rather than
                // whatever those positions mean by now.
                intents.push(Change::Extrude {
                    profile: profile.clone(),
                    distance,
                    operation: *operation,
                });
            }
            // The same, off two fields rather than one — see
            // [`Prompt::sector`], which is where degrees become radians.
            Asking::Revolve {
                profile,
                axis,
                operation,
            } => {
                let Some(sector) = self.sector(Self::says) else {
                    return;
                };
                intents.push(Change::Revolve {
                    profile: profile.clone(),
                    axis: *axis,
                    sector,
                    operation: *operation,
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

    /// What the chip setting the form to `marked`'s operation is recorded
    /// under.
    ///
    /// Named rather than salted, for the reason a field's id is: a caller
    /// outside cannot work out an `auto_id`, and a test pressing the control
    /// has to find where it was laid out. By its glyph because that is what
    /// tells the three apart and is already what the chip carries — which is
    /// also why no two rows of [`marked`] may share one.
    fn doing_id(marked: Marked) -> WidgetId {
        WidgetId::from_hash(("catcad.prompt.doing", marked.glyph))
    }

    /// What the chip answering with `marked` is recorded under.
    fn answer_id(marked: Marked) -> WidgetId {
        WidgetId::from_hash(("catcad.prompt.answer", marked.glyph))
    }

    /// How wide the column naming a form's fields has to be.
    ///
    /// **The widest of the labels, and not each label's own width.** Two fields
    /// under labels of different lengths would otherwise open their boxes at two
    /// different places, and what a form of two values should read as is a table
    /// of two values.
    ///
    /// Measured in the ambient face, which is what the labels are drawn in —
    /// the same call [`Prompt::over`] measures a dimension's own run with.
    fn labelling(ui: &mut Ui, fields: &[Field]) -> f32 {
        let font = ui.theme().text.font();
        fields.iter().fold(0.0, |so_far, field| {
            let run = TextRun {
                text: field.label,
                font,
                wrap: TextWrap::SingleLine,
                align: Align::default(),
                max_width_px: None,
            };
            so_far.max(ui.probe_text(run).size().w)
        })
    }

    /// The first field's number, as that field will shape it.
    ///
    /// Built off the same face the field is styled with, so what this measures
    /// is what that draws. Unbounded and single-line: a dimension is one run
    /// that the box is sized to rather than the other way about.
    ///
    /// The first and no other, because the only thing that measures a *number*
    /// is the form that stands *over* a mark — see [`Prompt::over`] — and that
    /// is the one-field kind. A form standing beside something is laid out by
    /// palantir, and the one thing it measures is how wide its labels run.
    fn run(&self) -> TextRun<'_> {
        TextRun {
            text: &self.fields[0].draft,
            font: MARK_FONT,
            wrap: TextWrap::Scroll,
            align: Align::CENTER,
            max_width_px: None,
        }
    }

    /// One field standing exactly where the drawing would have put the mark it
    /// replaces.
    ///
    /// A `Canvas` filling the view, so the field's position is measured from
    /// the same corner the projection answers in. It senses nothing, so every
    /// press it does not contain falls through to the viewport beneath — which
    /// is what makes clicking away from the field a click on the drawing as
    /// well as a blur.
    ///
    /// **What keeps the number from jumping as it becomes editable** is
    /// [`TextEditTheme::corner_centring`](palantir::TextEditTheme::corner_centring),
    /// which is asked rather than worked out here: a field is a box *around* a
    /// run, and how far inside its own corner it hangs that run is a handful of
    /// facts about the widget's own layout. What is left is measuring the run
    /// and saying where its middle goes.
    fn over(&mut self, ui: &mut Ui, theme: &Theme, centre: Vec2, opening: bool) -> Option<Done> {
        // Measured before the field is shown, because where its corner goes
        // depends on how wide its number comes out. The same shaper the widget
        // itself will use, asked the same question, so the two cannot answer
        // differently.
        //
        // The run's own leading rather than what it measured, so an empty draft
        // is a box where the number was and not one half a line higher: a
        // backspace onto nothing must not move the field it was typed in.
        let dressed = theme.dressed();
        let width = ui.probe_text(self.run()).size().w;
        let run = Size::new(width, MARK_FONT.line_height_px);
        let origin = dressed.field.corner_centring(run, centre);
        let Self { fields, .. } = self;
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
                    .style(&dressed.field)
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

    /// The whole form on a pill of its own, standing clear of `anchor`.
    ///
    /// [`Popup`] does the placing: it resolves the pill against the anchor from
    /// its own measured size and flips or shifts it to stay on the surface, so
    /// a face near the bottom of the view gets its form above instead. The one
    /// thing measured here is how wide the labels run, which is a question
    /// about the fields rather than about where they land.
    ///
    /// **That fitting is what keeps a form off the handle it shares a value
    /// with**, and it is why nothing here chooses a side. A side chosen would be
    /// one the fitting is free to give back for want of room; an anchor that
    /// holds where the handle can go leaves no room on the handle's side, so the
    /// same flip lands clear of it. See [`Stands::Beside`].
    ///
    /// [`ClickOutside::PassThrough`] because this annotates rather than
    /// interrupts. A modal popup installs a click-eater over the whole surface
    /// and claims the keyboard for every layer below, which for a form whose
    /// value is *also* dragged by an arrow in the drawing would mean a form you
    /// could never drag against.
    fn beside(
        &mut self,
        ui: &mut Ui,
        theme: &Theme,
        icons: &Icons,
        anchor: Rect,
        opening: bool,
    ) -> Option<Done> {
        let chrome = &theme.chrome;
        let dressed = theme.dressed();
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
        let blurs = self.blurs();
        // Measured before the popup, because the rows inside it borrow the
        // fields the labels are read off.
        let labelling = Self::labelling(ui, &self.fields);
        // Both borrowed off `self` at once, and they may be: they are different
        // fields, and the row below writes one while reading the other.
        let Self { about, fields, .. } = self;
        let named = about.named();
        let doing = about.doing_mut();
        let label = TextStyle {
            color: chrome.ink,
            font_size_px: chrome.readout_text,
            ..TextStyle::default()
        };
        let caption = TextStyle {
            font_size_px: chrome.caption_text,
            ..label.clone()
        };
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
                // Nothing to focus on a form with no field, and asking every
                // frame for a widget that is not there would take focus off the
                // chips that *are*.
                //
                // **What is held is that *a* field has the caret, not that the
                // first one does.** Asked of the first alone, a second field
                // took the caret on the frame it was clicked and lost it on the
                // next — so a form of two fields had one nobody could reach.
                let held =
                    (0..fields.len()).any(|nth| ui.focused_id() == Some(Self::field_id(nth)));
                if (opening || (!blurs && !held)) && !fields.is_empty() {
                    ui.request_focus(Some(Self::field_id(0)));
                }
                // **The overlay's own slab, at the overlay's own width.** A form
                // used to float on the drawing with nothing under it, so what
                // its words read against was whatever geometry happened to be
                // behind them — and on a lit solid that is nothing at all.
                //
                // Stated rather than hugged, which is what lets every row line
                // up: a label column, the boxes and the answers all end on one
                // edge. It is also what keeps the confirm still. Hugged, the
                // form would be as wide as the longest of Join, Cut and
                // Intersect, so pressing a chip would carry the confirm out
                // from under the pointer about to press it.
                Pill::vstack(theme, "form")
                    .width(chrome.card)
                    .over_drawing()
                    .show(ui, |ui| {
                        // **What says which form is open.** Everything below it
                        // is the same on an extrude and a revolve alike.
                        if let Some(named) = named {
                            Panel::hstack()
                                .id_salt("named")
                                .size((Sizing::FILL, Sizing::fixed(NAMING_ROW)))
                                .show(ui, |ui| {
                                    // A shape is not a layout item, so the
                                    // word steps past it by hand — the reach
                                    // the recipe's own rows are built on.
                                    let lift = (NAMING_ROW - NAMING_ICON) * 0.5;
                                    ui.add_shape(
                                        icons
                                            .shape(named.glyph)
                                            .at(Rect::new(0.0, lift, NAMING_ICON, NAMING_ICON))
                                            .tint(chrome.ink),
                                    );
                                    Text::new(named.word)
                                        .id_salt("naming")
                                        .style(&caption)
                                        .align(Align::new(HAlign::Left, VAlign::Center))
                                        .margin(Spacing::new(
                                            NAMING_ICON + chrome.gap,
                                            0.0,
                                            0.0,
                                            0.0,
                                        ))
                                        .show(ui);
                                });
                            pill::filling_line(ui, "under", 1.0, chrome.rule);
                        }
                        // A row apiece rather than a run of them, so both labels
                        // end on one column and both boxes open on one column.
                        // It is also what keeps a form narrow: it stands on a
                        // drawing, and a row grows with every field.
                        for (nth, field) in fields.iter_mut().enumerate() {
                            Panel::hstack()
                                .id_salt(("field", nth))
                                .size((Sizing::FILL, Sizing::HUG))
                                .gap(chrome.gap)
                                .show(ui, |ui| {
                                    if !field.label.is_empty() {
                                        // Against the middle of the box beside
                                        // it, not the top of the row: a field is
                                        // taller than the word naming it, so a
                                        // label left to its own devices sits on
                                        // the field's top edge.
                                        Text::new(field.label)
                                            .id_salt("label")
                                            .style(&label)
                                            .size((Sizing::fixed(labelling), Sizing::HUG))
                                            .align(Align::new(HAlign::Right, VAlign::Center))
                                            .show(ui);
                                    }
                                    // Read out at once, so the field's borrow
                                    // of the frame has ended by the time the
                                    // unit beside it records into the same row.
                                    said = said.and(Said::of(
                                        &TextEdit::new(&mut field.draft)
                                            .id(Self::field_id(nth))
                                            .style(&dressed.field)
                                            .select_all_on_focus()
                                            // Cloned because
                                            // [`TextEdit::placeholder`] takes a
                                            // `Cow<'static, str>` and this
                                            // string is the form's, not the
                                            // program's — a borrow cannot
                                            // satisfy that lifetime. One short
                                            // string a frame, and the only way
                                            // in.
                                            .placeholder(field.suggested.clone())
                                            .text_align(Align::CENTER)
                                            .size((Sizing::FILL, Sizing::HUG))
                                            .show(ui),
                                    ));
                                    if !field.unit.is_empty() {
                                        Text::new(field.unit)
                                            .id_salt("unit")
                                            .style(&label)
                                            .align(Align::new(HAlign::Left, VAlign::Center))
                                            .show(ui);
                                    }
                                });
                        }
                        // **What the answer will do, where there is a choice
                        // about it.** Only a sweep has one — a dimension
                        // restates a number and a circle is drawn, and neither
                        // does anything to a solid.
                        if let Some(doing) = doing {
                            Panel::hstack()
                                .id_salt("doing")
                                .size((Sizing::FILL, Sizing::HUG))
                                .gap(chrome.gap)
                                .show(ui, |ui| {
                                    for operation in
                                        [Operation::Join, Operation::Cut, Operation::Intersect]
                                    {
                                        let marked = marked::doing(operation);
                                        // Which one is set is said by inverting
                                        // it, which is what every held chip on
                                        // the overlay wears — so the row reads
                                        // as one control with a setting rather
                                        // than as three chips any of which
                                        // might be pressed.
                                        let chip = Chip::icon(
                                            Self::doing_id(marked),
                                            marked.word,
                                            marked.glyph,
                                        )
                                        .held(*doing == operation);
                                        if chip.show(ui, icons, theme) {
                                            *doing = operation;
                                        }
                                    }
                                    // **The setting in a word, and not only on
                                    // hover.** A picture of a result is read
                                    // faster once you know it is a result, and a
                                    // tooltip is read by somebody who already
                                    // suspects there is something to read.
                                    //
                                    // Filling what the three chips leave, so the
                                    // word takes the same room whichever of the
                                    // three it is.
                                    Text::new(marked::doing(*doing).word)
                                        .id_salt("chosen")
                                        .size((Sizing::FILL, Sizing::HUG))
                                        .align(Align::new(HAlign::Left, VAlign::Center))
                                        .show(ui);
                                });
                        }
                        // Only where this is how the form is dismissed. A form
                        // that blurs shut has no use for them, and two chips
                        // that were not the way out would be two chips lying
                        // about it.
                        if !blurs {
                            // Under the far end of the rows rather than their
                            // start, so the answers line up with the boxes they
                            // are about instead of with the words naming them.
                            // Confirm last, which is where a hand looking for
                            // the way on goes.
                            Panel::hstack()
                                .id_salt("answers")
                                .gap(chrome.gap)
                                .align(Align::h(HAlign::Right))
                                .show(ui, |ui| {
                                    for (marked, means, answer) in [
                                        (marked::CANCEL, theme.answers.stops, Done::Cancel),
                                        (marked::CONFIRM, theme.answers.goes, Done::Commit),
                                    ] {
                                        // Named like every other id on this
                                        // form, where it was salted: a caller
                                        // outside cannot work out an `auto_id`,
                                        // and these are as pressable by a test
                                        // as the row above them.
                                        let chip = Chip::icon(
                                            Self::answer_id(marked),
                                            marked.word,
                                            marked.glyph,
                                        )
                                        .answers(means);
                                        if chip.show(ui, icons, theme) {
                                            answered = Some(answer);
                                        }
                                    }
                                });
                        }
                    });
            });
        // The chips first: a press on one is an answer whatever the fields made
        // of the same frame, and a field that lost focus *to* the chip must not
        // cancel out from under it.
        answered.or_else(|| self.resolve(said))
    }
}

/// Which opening a form is.
///
/// A count that only goes up, minted by the [`Session`](crate::session::Session)
/// when a form opens. It says nothing but *not the one before it*, which is the
/// whole of what the drawing wants: see [`Prompt::form`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Form(u32);

impl Form {
    /// The one after this.
    pub(crate) fn next(self) -> Self {
        Self(self.0 + 1)
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
/// turned the feature on without turning tests on. Which gate means what is
/// argued at [`CatCad::internals`](crate::internals).
#[cfg(test)]
pub(crate) mod internals {
    use palantir::WidgetId;

    use crate::marked::Marked;
    use crate::prompt::Prompt;

    impl Prompt {
        /// What the `nth` field is recorded under.
        pub(crate) fn nth_field_id(nth: usize) -> WidgetId {
            Self::field_id(nth)
        }

        /// What the chip `marked` draws for an operation is recorded under —
        /// see [`marked`](crate::marked), which is where the rows are.
        pub(crate) fn operation_id(marked: Marked) -> WidgetId {
            Self::doing_id(marked)
        }

        /// What the chip answering with `marked` is recorded under.
        ///
        /// Every form here is answered with Enter as well, which a harness
        /// types. What this is for is a harness that means to press the chip
        /// — the other way out, and the only one a form with no field has.
        pub(crate) fn answering_id(marked: Marked) -> WidgetId {
            Self::answer_id(marked)
        }
    }
}

#[cfg(test)]
mod tests;
