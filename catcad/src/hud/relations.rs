//! What can be asked of what is picked out, along the bottom.

use palantir::{Align, DragValue, Ui, WidgetId};
use silverpoint::{Constraint, ConstraintId, Entity};

use crate::hud::chip::Chip;
use crate::hud::pill::{self, Pill};
use crate::hud::{Shown, control};
use crate::intent::change::Change;
use crate::intent::{Choice, Intent, Intents, Opening, Step};
use crate::look::chrome::Chrome;
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
/// see [`chrome.card`](crate::chrome.card) on why that matters.
pub(super) fn show(
    ui: &mut Ui,
    shown: Shown<'_>,
    offers: &mut Vec<Constraint>,
    draft: &mut f64,
    intents: &mut Intents,
) {
    let Shown {
        models, selection, ..
    } = shown;
    let startable = plane_picked(models, selection);
    let open = models.open();
    match open {
        Some(model) => model.offers(selection.picked(), offers),
        // Cleared rather than left, because it is kept between frames: what the
        // last open sketch admitted is not what a document being looked at
        // admits, and the walk below reads this list whether or not anything
        // refilled it.
        None => offers.clear(),
    }
    let dimension = open.and_then(|model| dimension_picked(model, selection));
    let region = open.and_then(|_| region_picked(selection));
    if offers.is_empty() && dimension.is_none() && region.is_none() && startable.is_none() {
        return;
    }
    // Seeded from the drawing every frame rather than remembered, which is what
    // makes the field a *view* of the dimension: an undo, a drag that moved it,
    // or picking a different one all show up here without anything having to
    // notice.
    if let Some(resizable) = dimension {
        *draft = resizable.value;
    }
    let chrome = &shown.theme.chrome;
    Pill::hstack(chrome, "relations")
        .align(Align::BOTTOM)
        .show(ui, |ui| {
            // First, because it is the one thing here that can be asked of a
            // document nobody is drawing in.
            if let Some(on) = startable
                && Chip::icon(relation_id("Sketch"), "Start a sketch", Glyph::Sketch).show(
                    ui,
                    shown.icons,
                    chrome,
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
                    chrome,
                )
            {
                // Asks rather than builds. The solid appears at no depth at all
                // and the form beside it decides how far it goes.
                intents.push(Choice::Ask(Some(Opening::Extrude {
                    sketch: growable.sketch,
                    region: growable.region,
                })));
            }
            if !offers.is_empty()
                && (startable.is_some() || dimension.is_some() || region.is_some())
            {
                pill::divider(ui, chrome, "offers");
            }
            for &constraint in offers.iter() {
                if offered(ui, shown.icons, chrome, constraint) {
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
fn offered(ui: &mut Ui, icons: &Icons, chrome: &Chrome, constraint: Constraint) -> bool {
    let named = wording::named(constraint);
    match named.glyph {
        Some(glyph) => Chip::mark(relation_id(named.word), named.word, glyph),
        None => Chip::word(relation_id(named.word), named.word, named.word),
    }
    .show(ui, icons, chrome)
}

/// A dimension the bar can scrub, and the number it states as it stands.
#[derive(Debug, Clone, Copy)]
struct Resizable {
    constraint: ConstraintId,
    value: f64,
}

/// The one dimension picked out, if what is picked is exactly that.
///
/// One rather than any, because the field edits a value and two values have no
/// single answer.
fn dimension_picked(model: Model<'_>, selection: &Selection) -> Option<Resizable> {
    let [only] = *selection.picked() else {
        return None;
    };
    let Some(Entity::Constraint(id)) = model.entity(only) else {
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

/// A region the bar can grow a solid off.
///
/// Both halves, because a region is only a region of the arrangement it was
/// walked out of — see [`Part::Region`] — so the sketch it belongs to travels
/// with it rather than being looked up again at the press.
#[derive(Debug, Clone, Copy)]
struct Growable {
    sketch: FeatureId,
    region: usize,
}

/// The one region picked out, if what is picked is exactly that.
fn region_picked(selection: &Selection) -> Option<Growable> {
    match *selection.picked() {
        [Part::Region { sketch, at }] => Some(Growable { sketch, region: at }),
        _ => None,
    }
}

/// The one plane picked out, if what is picked is exactly that.
///
/// **A plane and not merely a step**, which is the whole of what `models` is
/// here for. Every kind of step is one thing a press can pick out, and a sketch
/// cannot be started on either a sketch or an extrude: what would follow is the
/// timeline being asked for the frame of something that has none.
fn plane_picked(models: Models<'_>, selection: &Selection) -> Option<FeatureId> {
    let [Part::Step(at)] = *selection.picked() else {
        return None;
    };
    models
        .planes()
        .any(|sheeted| sheeted.at == at)
        .then_some(at)
}
