//! What can be asked of what is picked out, along the bottom.

use palantir::{Align, DragValue, Ui, WidgetId};
use silverpoint::{Constraint, ConstraintId, Entity, Named, SegmentId};

use crate::control::chip::Chip;
use crate::control::pill::{self, Pill};
use crate::hud::{Shown, control};
use crate::intent::change::Change;
use crate::intent::{Choice, Intent, Intents, Opening, Step};
use crate::look::Theme;
use crate::look::icons::{Glyph, Icons};
use crate::marked::{self, Marked};
use crate::model::{Model, Models};
use crate::paint::DECIMALS;
use crate::part::Part;
use crate::selection::Selection;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;
use crate::wording;

/// Sketch units per pixel of scrub. A hundredth, so a drag reads a number out
/// at the same precision the drawing prints it to and a slow pull can land on a
/// round number.
const SCRUB_SPEED: f64 = 0.01;

/// What the radius field opens at, before anybody has scrubbed one.
///
/// **A number rather than nothing**, because a blend of no radius is one the
/// kernel refuses — see [`Rounding::round`](silverpoint::Rounding) — so a field
/// opening at nought would offer a chip whose only answer is a step that will
/// not build. One is what a document with no unit can honestly seed, and the
/// field keeps whatever it is scrubbed to for the next fillet.
const FIRST_RADIUS: f64 = 1.0;

/// The number the radius field is showing.
///
/// **A type for one number, because its default is not nought.** The bar keeps
/// it between frames — see [`Hud`](crate::hud::Hud) — and a derived default
/// there would open the field on a radius the kernel refuses. Spelled here, so
/// what the field opens at and what it means are one line apart.
#[derive(Debug, Clone, Copy)]
pub(super) struct Radius(f64);

impl Default for Radius {
    fn default() -> Self {
        Self(FIRST_RADIUS)
    }
}

pub(super) fn relation_id(label: &str) -> WidgetId {
    control("relation", label)
}

/// What the chip that takes a step away is recorded under.
///
/// Named rather than spelled at each of the two: the bar draws it and
/// [`Hud::show`](crate::hud::Hud) reads whether it is hovered, and a chip
/// pointed at under one name and read under another would be a preview that
/// never showed.
pub(super) const REMOVE: &str = "Remove";

/// Show it, where there is anything to show.
///
/// Shown only when there is something to offer, rather than a fixed bar of
/// mostly-dead chips. A selection admits at most four relations and usually
/// none, so a bar sized to every constraint there is would be mostly grey the
/// whole time — and what the user wants to know is what *this* selection can do.
///
/// **Almost everything here wants a sketch open**, and for one reason: what it
/// offers is what can be *said about* a drawing, and none of that is asked of a
/// document you are only looking at. Three are not — starting a sketch, and the
/// two a fillet is asked through, which are about the model rather than about
/// any drawing — so those are the ones read before that gate.
///
/// **Centred, which the rest of the overlay is not.** Two edges picked in the
/// middle of the view deserve an answer in the middle of the view. It is safe
/// to centre because everything on it is a chip and a chip count is a width —
/// see [`theme.chrome.card`](crate::theme.chrome.card) on why that matters.
pub(super) fn show(
    ui: &mut Ui,
    shown: Shown<'_>,
    offers: &mut Vec<Constraint>,
    picked: &Picked,
    draft: &mut f64,
    radius: &mut Radius,
    intents: &mut Intents,
) {
    let Shown {
        models, selection, ..
    } = shown;
    let startable = picked.plane(models);
    let removable = picked.removable(models);
    let roundable = picked.roundable();
    let blendable = picked.blendable(models);
    let open = models.open();
    match open {
        Some(model) => model.offers(selection.picked(), offers),
        // Cleared rather than left, because it is kept between frames: what the
        // last open sketch admitted is not what a document being looked at
        // admits, and the walk below reads this list whether or not anything
        // refilled it.
        None => offers.clear(),
    }
    let dimension = open.and_then(|model| picked.resizable(model));
    let region = open.and_then(|_| picked.growable());
    let spinning = open.and_then(|_| picked.spinnable());
    if offers.is_empty()
        && dimension.is_none()
        && region.is_none()
        && spinning.is_none()
        && startable.is_none()
        && removable.is_none()
        && roundable.is_none()
        && blendable.is_none()
    {
        return;
    }
    // Seeded from the drawing every frame rather than remembered, which is what
    // makes the field a *view* of the dimension: an undo, a drag that moved it,
    // or picking a different one all show up here without anything having to
    // notice.
    if let Some(resizable) = dimension {
        *draft = resizable.value;
    }
    // The same, for a rounding already in the recipe. A fillet still being
    // *offered* names no step, so the field keeps what it was last scrubbed to
    // — which is what makes a second fillet open at the first one's radius.
    if let Some(stated) = blendable.map(|blendable| blendable.radius) {
        radius.0 = stated;
    }
    let theme = shown.theme;
    Pill::hstack(theme, "relations")
        .align(Align::BOTTOM)
        .show(ui, |ui| {
            // First, because it is the one thing here that can be asked of a
            // document nobody is drawing in.
            if let Some(on) = startable
                // The one chip here that does not go through [`offering`]: it
                // is drawn and recorded off the table like every other, and
                // captioned as the *command* that makes a sketch rather than as
                // the thing the recipe's row names.
                && Chip::icon(
                    relation_id(marked::SKETCH.word),
                    "Start a sketch",
                    marked::SKETCH.glyph,
                )
                .show(ui, shown.icons, theme)
            {
                intents.push(Change::AddSketch { on });
            }
            // Before the gate below, because neither wants a drawing: what a
            // fillet is asked of is the model, and a face of it is picked in the
            // viewport rather than drawn — see
            // [`Change::Round`](crate::intent::change::Change).
            if let Some(along) = roundable {
                scrub(ui, &mut radius.0);
                if offering(ui, shown.icons, theme, marked::FILLET) {
                    // The list reaches the heap on the frame the chip is
                    // pressed and on no other, as the extrude's profile does.
                    intents.push(Change::Round {
                        along: vec![along],
                        radius: radius.0,
                    });
                }
            }
            // The fillet already in the recipe, restated. The same field, so
            // the number is read and scrubbed in one place whether the step
            // exists yet or not.
            if let Some(blendable) = blendable {
                let edited = scrub(ui, &mut radius.0);
                if edited.changed {
                    intents.push(Change::Blend {
                        round: blendable.at,
                        to: radius.0,
                    });
                }
                // One gesture, one step to take back, on the terms the
                // dimension above states: `Blend` coalesces.
                if edited.committed {
                    intents.push(Step::Release);
                }
            }
            let Some(sketch) = open.map(Model::of) else {
                return;
            };
            if let Some(resizable) = dimension {
                let edited = scrub(ui, draft);
                if edited.changed {
                    intents.push(Change::Resize {
                        sketch,
                        constraint: resizable.constraint,
                        to: *draft,
                    });
                }
                // One gesture, one step to take back. `Resize` coalesces, so
                // the run of them a scrub sends is one open step — and this is
                // what closes it.
                if edited.committed {
                    intents.push(Step::Release);
                }
            }
            // Before the relations, because it is the one thing here that
            // builds rather than states.
            if let Some(growable) = region
                && offering(ui, shown.icons, theme, marked::EXTRUDE)
                && let Some(model) = models.at(growable.sketch)
            {
                // Asks rather than builds. The solid appears at no depth at all
                // and the form beside it decides how far it goes.
                //
                // **Named here rather than a pass later**, this being the one
                // moment positions become a durable name — see
                // [`Model::profile`]. The list reaches the heap on the frame
                // the chip is pressed and on no other.
                intents.push(Choice::Ask(Some(Opening::Extrude {
                    profile: model.profile(growable.regions),
                })));
            }
            // Asks rather than builds, as the extrude above does — the ring
            // appears whole from the moment the form opens, there being no
            // number to wait for, and the form beside it decides what it does
            // to the model.
            if let Some(spinnable) = spinning
                && offering(ui, shown.icons, theme, marked::REVOLVE)
                && let Some(model) = models.at(spinnable.sketch)
            {
                intents.push(Choice::Ask(Some(Opening::Revolve {
                    profile: model.profile(spinnable.regions),
                    axis: spinnable.axis,
                })));
            }
            // Last of the things that *do* something, and the one that
            // takes rather than makes — so the offers below it stay what can be
            // said about a drawing.
            //
            // **What goes with it is on the card before the press**, worn by
            // every row this would take: the cascade is read where both drawers
            // can see one answer — see [`Hud::show`](crate::hud::Hud).
            if let Some(step) = removable
                && Chip::icon(relation_id(REMOVE), REMOVE, Glyph::Remove).show(
                    ui,
                    shown.icons,
                    theme,
                )
            {
                intents.push(Change::DeleteStep { step });
            }
            if !offers.is_empty()
                && (startable.is_some()
                    || dimension.is_some()
                    || region.is_some()
                    || spinning.is_some()
                    || removable.is_some()
                    || roundable.is_some()
                    || blendable.is_some())
            {
                pill::divider(ui, theme, "offers");
            }
            for &constraint in offers.iter() {
                if offered(ui, shown.icons, theme, constraint) {
                    // **Two answers, and which one a chip gives follows from
                    // what it is short of.** A relation says something the
                    // drawing can work out for itself, so pressing it states it
                    // outright. A dimension already knows its number and is
                    // short of somewhere to put it, so it goes into the
                    // pointer's hands.
                    intents.push(match constraint {
                        constraint if let Some(placing) = Dimensioning::placing(constraint) => {
                            Intent::from(Choice::Hold(Tool::Dimension(placing)))
                        }
                        constraint => Change::Constrain { sketch, constraint }.into(),
                    });
                }
            }
        });
}

/// Show one number the bar scrubs, writing what the gesture makes of it into
/// `value`.
///
/// Its own call because three readings show one — a dimension, a fillet being
/// offered, and a fillet already in the recipe — and a field spelled three
/// times would be three places for the speed and the precision to differ.
///
/// The widget's own response is read here and not handed back: it borrows both
/// the number and the surface for as long as it lives, which would leave every
/// caller unable to read the number it had just been scrubbed.
fn scrub(ui: &mut Ui, value: &mut f64) -> Scrubbed {
    let edited = DragValue::new(value)
        .speed(SCRUB_SPEED)
        .decimals(DECIMALS)
        .show(ui);
    Scrubbed {
        changed: edited.changed,
        committed: edited.committed,
    }
}

/// What one scrub of a number came to.
///
/// Two answers rather than one, because they close different things: a change
/// is what the document is asked to become, and a commit is the end of the
/// gesture that asked — see [`Step::Release`].
#[derive(Debug, Clone, Copy)]
struct Scrubbed {
    changed: bool,
    committed: bool,
}

/// The chip that offers to build `what`, drawn and named off the one table.
///
/// **The word is the id as well as the caption**, which is what keeps a chip
/// pointed at under the same name it is drawn under — see [`REMOVE`], where
/// that is argued for the one chip a second reader looks up.
fn offering(ui: &mut Ui, icons: &Icons, theme: &Theme, what: Marked) -> bool {
    Chip::icon(relation_id(what.word), what.word, what.glyph).show(ui, icons, theme)
}

/// One offer, drawn as the drawing draws it.
///
/// **The mark comes off [`wording`], which is where the drawing gets it too.**
/// A relation has a draughtsman's mark — ∥, ⊥, =, ∈ — and painting the chip
/// with the same character means a user reads the bar in the vocabulary the
/// geometry is already annotated in. A dimension has no mark, because it is
/// drawn as its number, so it falls back to its word.
fn offered(ui: &mut Ui, icons: &Icons, theme: &Theme, constraint: Constraint) -> bool {
    let named = wording::named(constraint);
    match named.glyph {
        Some(glyph) => Chip::mark(relation_id(named.word), named.word, glyph),
        None => Chip::word(relation_id(named.word), named.word, named.word),
    }
    .show(ui, icons, theme)
}

/// What is picked out, sorted into what the bar can be asked about.
///
/// **The contents rather than the shape.** Every reading below used to match the
/// whole selection against a pattern — one region, or a region and a segment
/// written both ways round — so the bar went silent for any pick it had not
/// spelled out, and a third thing picked would have wanted six arms. Sorted
/// once, each is a question about what is *in* the selection.
///
/// Kept across frames for its room, like the offers beside it: the bar runs
/// every frame and these come out the same size each time.
#[derive(Debug, Default)]
pub(super) struct Picked {
    /// Which sketch everything picked belongs to, where they agree on one.
    sketch: Option<FeatureId>,
    /// Every region picked, in the order they were picked.
    regions: Vec<usize>,
    /// Every piece of drawn geometry picked.
    entities: Vec<Entity>,
    /// Every step picked.
    steps: Vec<FeatureId>,
    /// Every face of the model picked, in the order they were picked.
    ///
    /// **A [`Named`] rather than the [`Part`] it came off**, which is where a
    /// pick becomes a name the kernel takes: a face answers to one across every
    /// edit, and the pair of them a fillet is asked for is exactly the two
    /// picked out — see [`Change::Round`].
    faces: Vec<Named>,
    /// Whether anything picked is none of those four, or is of a second
    /// drawing.
    ///
    /// One flag rather than a list, because every reading below refuses the
    /// moment it is set: what the bar offers is what can be said about one
    /// drawing or about the model, and what is left is a handle on a form.
    strays: bool,
}

impl Picked {
    /// Sort `selection` into this, emptying whatever was there.
    ///
    /// Called before anything is drawn rather than by the bar that reads it,
    /// because the recipe card reads it too — see
    /// [`Hud::show`](crate::hud::Hud).
    pub(super) fn sort(&mut self, selection: &Selection) {
        self.sketch = None;
        self.regions.clear();
        self.entities.clear();
        self.steps.clear();
        self.faces.clear();
        self.strays = false;
        for &part in selection.picked() {
            match part {
                Part::Region { sketch, at } => {
                    self.claim(sketch);
                    self.regions.push(at);
                }
                Part::Entity { sketch, entity } => {
                    self.claim(sketch);
                    self.entities.push(entity);
                }
                Part::Step(at) => self.steps.push(at),
                Part::Solid { of, face } => self.faces.push(of.step().grew(face)),
                Part::Growing | Part::Turning => self.strays = true,
            }
        }
    }

    /// Note that something of `sketch` is picked, and whether that makes two.
    fn claim(&mut self, sketch: FeatureId) {
        match self.sketch {
            Some(had) => self.strays |= had != sketch,
            None => self.sketch = Some(sketch),
        }
    }

    /// Whether nothing is picked out beyond the lists `wanted` names.
    ///
    /// **One reading rather than one per offer.** Every offer below refuses a
    /// selection holding anything it did not ask for, and a spelling apiece is
    /// a spelling apiece to forget when a fifth kind of thing becomes pickable
    /// — which fails silently, an offer appearing for a selection it has no
    /// answer for.
    fn only(&self, wanted: Wanted) -> bool {
        !self.strays
            && (wanted.regions || self.regions.is_empty())
            && (wanted.entities || self.entities.is_empty())
            && (wanted.steps || self.steps.is_empty())
            && (wanted.faces || self.faces.is_empty())
    }

    /// The one dimension picked out, if what is picked is exactly that.
    ///
    /// One rather than any, because the field edits a value and two values have
    /// no single answer.
    fn resizable(&self, model: Model<'_>) -> Option<Resizable> {
        if !self.only(Wanted {
            entities: true,
            ..Wanted::default()
        }) {
            return None;
        }
        let [Entity::Constraint(id)] = self.entities[..] else {
            return None;
        };
        model
            .drawing()
            .holds(id)
            .then(|| {
                model
                    .sketch()
                    .constraint(id)
                    .value()
                    .map(|value| Resizable {
                        constraint: id,
                        value,
                    })
            })
            .flatten()
    }

    /// The regions picked out, if regions of one drawing are all that is.
    fn growable(&self) -> Option<Growable<'_>> {
        let sketch = self.sketch?;
        let alone = self.only(Wanted {
            regions: true,
            ..Wanted::default()
        });
        (alone && !self.regions.is_empty()).then_some(Growable {
            sketch,
            regions: &self.regions,
        })
    }

    /// The regions picked out and the line to spin them about, if what is
    /// picked is those and one segment of the same drawing.
    ///
    /// **Which was clicked first says nothing**, which is what sorting bought.
    fn spinnable(&self) -> Option<Spinnable<'_>> {
        let sketch = self.sketch?;
        let alone = self.only(Wanted {
            regions: true,
            entities: true,
            ..Wanted::default()
        });
        if !alone || self.regions.is_empty() {
            return None;
        }
        let [Entity::Segment(axis)] = self.entities[..] else {
            return None;
        };
        Some(Spinnable {
            sketch,
            regions: &self.regions,
            axis,
        })
    }

    /// The one step picked out that may be taken away, if what is picked is
    /// exactly that.
    ///
    /// One rather than any, and the reason is what the *card* does with it: a
    /// removal shows what goes with it before it happens, and the cascade of
    /// several heads at once is a different picture. The Delete key goes on
    /// taking everything picked.
    pub(super) fn removable(&self, models: Models<'_>) -> Option<FeatureId> {
        self.step().filter(|&at| models.removable(at))
    }

    /// The one step picked out, if what is picked is exactly that.
    ///
    /// What the two readings either side of it both begin with, and neither
    /// may answer for itself: a step picked *alongside* geometry says nothing
    /// about either, and two spellings of that would be two chances to differ
    /// over which of them to refuse.
    fn step(&self) -> Option<FeatureId> {
        if !self.only(Wanted {
            steps: true,
            ..Wanted::default()
        }) {
            return None;
        }
        let [at] = self.steps[..] else {
            return None;
        };
        Some(at)
    }

    /// The pair of faces a fillet would be asked for, if what is picked is
    /// exactly two of them.
    ///
    /// **Two, because a pick names one edge**, which is the edge those two
    /// faces divide — see [`Feature::Round`](crate::timeline::feature::Feature).
    /// One face names no edge and three name no one edge, so neither is
    /// something the bar has an answer for.
    ///
    /// The two have to differ. A face picked twice cannot be, a selection
    /// holding nothing twice over — but the *same name* twice is reachable, a
    /// face of the body coming in several patches, and an edge between a face
    /// and itself is one the kernel refuses.
    fn roundable(&self) -> Option<[Named; 2]> {
        if !self.only(Wanted {
            faces: true,
            ..Wanted::default()
        }) {
            return None;
        }
        let [one, two] = self.faces[..] else {
            return None;
        };
        (one != two).then_some([one, two])
    }

    /// The one rounding picked out and the radius it states, if what is picked
    /// is exactly that step.
    ///
    /// The mirror of [`Picked::resizable`] one level up: a step somebody may
    /// restate, and what it says as it stands.
    fn blendable(&self, models: Models<'_>) -> Option<Blendable> {
        let at = self.step()?;
        models.radius_at(at).map(|radius| Blendable { at, radius })
    }

    /// The one plane picked out, if what is picked is exactly that.
    ///
    /// **A plane and not merely a step**, which is the whole of what `models` is
    /// here for. Every kind of step is one thing a press can pick out, and a
    /// sketch cannot be started on either a sketch or an extrude: what would
    /// follow is the timeline being asked for the frame of something that has
    /// none.
    fn plane(&self, models: Models<'_>) -> Option<FeatureId> {
        self.step()
            .filter(|&at| models.planes().any(|sheeted| sheeted.at == at))
    }
}

/// Which of the sorted lists an offer reads, and so which it will tolerate
/// something in.
///
/// Named rather than four bools in a row, on the terms [`Scrubbed`] states
/// about two: they are all one type, so any two could change places without a
/// word from the compiler — and an offer that tolerated steps where it meant
/// faces would appear for a selection it has no answer for.
#[derive(Debug, Clone, Copy, Default)]
struct Wanted {
    regions: bool,
    entities: bool,
    steps: bool,
    faces: bool,
}

/// A dimension the bar can scrub, and the number it states as it stands.
#[derive(Debug, Clone, Copy)]
struct Resizable {
    constraint: ConstraintId,
    value: f64,
}

/// A rounding the bar can scrub, and the radius it states as it stands.
///
/// [`Resizable`]'s twin, and the same shape for the same reason: which step to
/// name in the change it raises, and what to seed the field with.
#[derive(Debug, Clone, Copy)]
struct Blendable {
    at: FeatureId,
    radius: f64,
}

/// The regions the bar can grow a solid off.
///
/// Both halves, because a region is only a region of the arrangement it was
/// walked out of — see [`Part::Region`] — so the sketch they belong to travels
/// with them rather than being looked up again at the press.
#[derive(Debug, Clone, Copy)]
struct Growable<'a> {
    sketch: FeatureId,
    regions: &'a [usize],
}

/// The regions the bar can spin a solid off, and the line to spin them about.
#[derive(Debug, Clone, Copy)]
struct Spinnable<'a> {
    sketch: FeatureId,
    regions: &'a [usize],
    axis: SegmentId,
}
