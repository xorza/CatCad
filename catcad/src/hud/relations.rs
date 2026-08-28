//! What can be asked of what is picked out, along the bottom.

use palantir::{Align, DragValue, Ui, WidgetId};
use silverpoint::{Constraint, ConstraintId, Entity, SegmentId};

use crate::hud::chip::Chip;
use crate::hud::pill::{self, Pill};
use crate::hud::{Shown, control};
use crate::intent::change::Change;
use crate::intent::{Choice, Intent, Intents, Opening, Step};
use crate::look::Theme;
use crate::look::icons::{Glyph, Icons};
use crate::model::{Model, Models};
use crate::paint::DECIMALS;
use crate::part::Part;
use crate::selection::Selection;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;
use crate::wording;

/// Sketch units per pixel of scrub. A hundredth, so a drag reads a dimension
/// out at the same precision the drawing prints it to and a slow pull can land
/// on a round number.
const DIMENSION_SPEED: f64 = 0.01;

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
/// document you are only looking at. Starting a sketch is the one offer that is
/// not, so it is the one read before that gate.
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
    intents: &mut Intents,
) {
    let Shown {
        models, selection, ..
    } = shown;
    let startable = picked.plane(models);
    let removable = picked.removable(models);
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
    let theme = shown.theme;
    Pill::hstack(theme, "relations")
        .align(Align::BOTTOM)
        .show(ui, |ui| {
            // First, because it is the one thing here that can be asked of a
            // document nobody is drawing in.
            if let Some(on) = startable
                && Chip::icon(relation_id("Sketch"), "Start a sketch", Glyph::Sketch).show(
                    ui,
                    shown.icons,
                    theme,
                )
            {
                intents.push(Change::AddSketch { on });
            }
            let Some(sketch) = open.map(Model::of) else {
                return;
            };
            if let Some(resizable) = dimension {
                let edited = DragValue::new(&mut *draft)
                    .speed(DIMENSION_SPEED)
                    .decimals(DECIMALS)
                    .show(ui);
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
                && Chip::icon(relation_id("Extrude"), "Extrude", Glyph::Extrude).show(
                    ui,
                    shown.icons,
                    theme,
                )
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
            //
            // The extrude's own glyph until a revolve has one drawn for it, as
            // the recipe's row does — the word is what tells the two apart.
            if let Some(spinnable) = spinning
                && Chip::icon(relation_id("Revolve"), "Revolve", Glyph::Extrude).show(
                    ui,
                    shown.icons,
                    theme,
                )
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
                    || removable.is_some())
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
    /// Whether anything picked is none of those three, or is of a second
    /// drawing.
    ///
    /// One flag rather than a list, because every reading below refuses the
    /// moment it is set: what the bar offers is what can be said about *one*
    /// drawing, and a face of a solid is not something it says anything about.
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
                _ => self.strays = true,
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

    /// The one dimension picked out, if what is picked is exactly that.
    ///
    /// One rather than any, because the field edits a value and two values have
    /// no single answer.
    fn resizable(&self, model: Model<'_>) -> Option<Resizable> {
        if self.strays || !self.regions.is_empty() || !self.steps.is_empty() {
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
        let alone = !self.strays && self.entities.is_empty() && self.steps.is_empty();
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
        if self.strays || self.regions.is_empty() || !self.steps.is_empty() {
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
        if self.strays || !self.regions.is_empty() || !self.entities.is_empty() {
            return None;
        }
        let [at] = self.steps[..] else {
            return None;
        };
        Some(at)
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

/// A dimension the bar can scrub, and the number it states as it stands.
#[derive(Debug, Clone, Copy)]
struct Resizable {
    constraint: ConstraintId,
    value: f64,
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
