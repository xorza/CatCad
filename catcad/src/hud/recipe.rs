//! The recipe, down the right edge: a row per step, in the order they build.

use palantir::{
    Align, Background, Color, Configure, Corners, Panel, Rect, Sense, Shape, Sizing, Spacing, Text,
    TextStyle, Ui, WidgetId,
};

use crate::hud::{Shown, control};
use crate::intent::{Choice, Intents};
use crate::look;
use crate::look::Look;
use crate::look::icons::Glyph;
use crate::look::ink;
use crate::model::Broken;
use crate::part::Part;
use crate::timeline::FeatureId;
use crate::timeline::feature::{Datum, Feature};

/// A row's identity, by the handle of the step it stands for.
///
/// By the handle and not by position, which is what makes a row's identity
/// survive a step above it going: keyed by position, every row below a delete
/// would take its neighbour's id and the pointer would find itself over a
/// different step.
pub(super) fn step_id(at: FeatureId) -> WidgetId {
    control("step", at)
}

/// How tall one row stands, and the artwork's share of it.
const ROW: f32 = 22.0;
const ROW_ICON: f32 = 13.0;

/// Show it.
///
/// **What two of the three kinds of step could not be pointed at without.** A
/// plane has a square in the view, but a sketch *step* and an extrude have
/// nothing on screen that is the step rather than something it produced — a
/// sketch's geometry is what is drawn *in* it, and a solid's face was grown off
/// a region.
///
/// **One glyph per kind**, which is the one fact a row holds that a name does
/// not: a plane, a drawing and a solid are three different things, and a list
/// that drew them alike threw that away.
pub(super) fn show(ui: &mut Ui, look: &Look, shown: Shown<'_>, intents: &mut Intents) {
    let Shown {
        models, selection, ..
    } = shown;
    Pane::new().show(ui, |ui| {
        let (mut planes, mut sketches, mut solids) = (0, 0, 0);
        for (at, feature) in models.steps() {
            // The same words the status line uses for the same states, so the
            // two say it the same way — see [`Status`](crate::status::Status).
            let broken = match models.broken_at(at) {
                Some(Broken::Profile) => " · lost",
                Some(Broken::Unmerged) => " · apart",
                None => "",
            };
            let (glyph, named, nth) = match feature {
                Feature::Plane(Datum::World(world)) => (Glyph::Plane, world.named(), None),
                Feature::Plane(Datum::Offset { .. }) => {
                    planes += 1;
                    (Glyph::Plane, "Plane", Some(planes))
                }
                Feature::Sketch { .. } => {
                    sketches += 1;
                    (Glyph::Sketch, "Sketch", Some(sketches))
                }
                Feature::Extrude { .. } => {
                    solids += 1;
                    (Glyph::Extrude, "Extrude", Some(solids))
                }
            };
            // Interned into the pass's own arena rather than formatted into a
            // `String`: this is a row per step per frame, and the record pass is
            // gated at zero allocations.
            let label = match nth {
                Some(nth) => ui.fmt(format_args!("{named} {nth}{broken}")),
                None => ui.fmt(format_args!("{named}{broken}")),
            };
            if row(
                ui,
                look,
                at,
                glyph,
                label,
                selection.contains(Part::Step(at)),
            ) {
                // Picked out and nothing else. What follows from picking a step
                // is decided where everything else about a selection is — a
                // sketch opens, a plane does not — see
                // [`Models::opens`](crate::model::Models).
                intents.push(Choice::Select(Some(Part::Step(at))));
            }
            // **The bar, drawn between the rows it divides.** One marker rather
            // than a mark on every row below it: what is rolled back is a
            // *tail*, so where it starts is the whole of what there is to show.
            if models.rolled() == Some(at) {
                rolled(ui, at);
            }
        }
    });
}

/// The card the rows stand on.
///
/// Its own type rather than a [`Pill`](crate::hud::pill::Pill), for the one way
/// it differs: a row fills the card's width where a chip is square, so the card
/// states a width and the pill hugs.
#[derive(Debug)]
struct Pane {
    panel: Panel,
}

impl Pane {
    fn new() -> Self {
        Self {
            panel: Panel::vstack()
                .id_salt("recipe")
                .align(Align::TOP_RIGHT)
                .margin(Spacing::all(look::INSET))
                .size((Sizing::fixed(look::CARD), Sizing::HUG))
                .gap(1.0)
                .padding(Spacing::all(look::PILL_PAD))
                .sense(Sense::CLICK | Sense::DRAG | Sense::SCROLL)
                .background(
                    Background::rounded(ink::PILL, Corners::all(look::PILL_RADIUS))
                        .with_stroke(look::hairline()),
                ),
        }
    }

    fn show(self, ui: &mut Ui, body: impl FnOnce(&mut Ui)) {
        self.panel.show(ui, body);
    }
}

/// One row, and whether it was pressed.
fn row(
    ui: &mut Ui,
    look: &Look,
    at: FeatureId,
    glyph: Glyph,
    label: palantir::InternedStr,
    picked: bool,
) -> bool {
    let id = step_id(at);
    let hovered = ui.response_for(id).hovered;
    let (fill, ink) = match (picked, hovered) {
        (true, _) => (ink::CHIP_HELD, ink::CHROME_ON_HELD),
        (false, true) => (ink::CHIP, ink::CHROME_LIT),
        (false, false) => (Color::TRANSPARENT, ink::CHROME_INK),
    };
    let style = TextStyle {
        color: ink,
        font_size_px: look::READOUT_TEXT,
        ..TextStyle::default()
    };
    let row = Panel::hstack()
        .id(id)
        .size((Sizing::FILL, Sizing::fixed(ROW)))
        .padding(Spacing::new(look::PILL_PAD, 0.0, look::PILL_PAD, 0.0))
        .gap(look::GAP)
        .sense(Sense::CLICK)
        .background(Background::rounded(fill, Corners::all(look::CHIP_RADIUS)))
        .show(ui, |ui| {
            let lift = (ROW - ROW_ICON) * 0.5;
            ui.add_shape(
                Shape::icon(look.icons().of(glyph))
                    .at(Rect::new(look::PILL_PAD, lift, ROW_ICON, ROW_ICON))
                    .tint(ink),
            );
            Text::new(label)
                .auto_id()
                .style(&style)
                .align(Align::new(palantir::HAlign::Left, palantir::VAlign::Center))
                .margin(Spacing::new(ROW_ICON + look::GAP, 0.0, 0.0, 0.0))
                .show(ui);
        });
    row.response.left.clicked()
}

/// Where the build stops, drawn as a rule rather than as a run of dashes.
fn rolled(ui: &mut Ui, at: FeatureId) {
    palantir::Separator::horizontal()
        .id_salt(at)
        .color(ink::tint(ink::FREE))
        .margin(Spacing::new(look::PILL_PAD, 3.0, look::PILL_PAD, 3.0))
        .show(ui);
}
