//! The recipe, down the right edge: a row per step, in the order they build.

use palantir::{
    Align, Background, Configure, Corners, HAlign, InternedStr, Panel, Rect, Sense, Sizing,
    Spacing, Text, TextStyle, TextWrap, Ui, VAlign, WidgetId,
};

use crate::hud::pill::{self, Pill};
use crate::hud::wearing::Wearing;
use crate::hud::{Shown, control};
use crate::intent::{Choice, Intents};
use crate::look::Theme;
use crate::look::drawing;
use crate::look::icons::{Glyph, Icons};
use crate::model::Broken;
use crate::part::Part;
use crate::timeline::FeatureId;
use crate::timeline::feature::{Datum, Feature};
use glam::Vec3;

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
pub(super) fn show(ui: &mut Ui, shown: Shown<'_>, intents: &mut Intents) {
    let Shown {
        models, selection, ..
    } = shown;
    let theme = shown.theme;
    // Rows nearly touching, where a pill of chips leaves the chip gap: a list
    // reads as one thing, and rows a chip's width apart read as a column of
    // separate slabs.
    Pill::vstack(theme, "recipe")
        .align(Align::TOP_RIGHT)
        .width(theme.chrome.card)
        .gap(1.0)
        .show(ui, |ui| {
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
                    shown.icons,
                    theme,
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
                    rolled(ui, theme, shown.theme.drawing.free);
                }
            }
        });
}

/// One row, and whether it was pressed.
fn row(
    ui: &mut Ui,
    icons: &Icons,
    theme: &Theme,
    at: FeatureId,
    glyph: Glyph,
    label: InternedStr,
    picked: bool,
) -> bool {
    let chrome = &theme.chrome;
    let id = step_id(at);
    let wearing = Wearing::row(theme, picked, ui.response_for(id).hovered).eased(ui, id, theme);
    let style = TextStyle {
        color: wearing.ink,
        font_size_px: chrome.readout_text,
        ..TextStyle::default()
    };
    let row = Panel::hstack()
        .id(id)
        .size((Sizing::FILL, Sizing::fixed(ROW)))
        .padding(Spacing::new(chrome.pad, 0.0, chrome.pad, 0.0))
        .gap(chrome.gap)
        .sense(Sense::CLICK)
        .background(Background::rounded(
            wearing.fill,
            Corners::all(chrome.chip_radius),
        ))
        .show(ui, |ui| {
            let lift = (ROW - ROW_ICON) * 0.5;
            ui.add_shape(
                icons
                    .shape(glyph)
                    .at(Rect::new(chrome.pad, lift, ROW_ICON, ROW_ICON))
                    .tint(wearing.ink),
            );
            // Told to fill and cut off, for the reason the readout's line is:
            // a run of text reports its natural width as the least it will
            // accept, so a card that states a width does not bound it on its
            // own — a long enough name would run out past the card's edge.
            Text::new(label)
                .auto_id()
                .style(&style)
                .text_wrap(TextWrap::Ellipsis)
                .size((Sizing::FILL, Sizing::HUG))
                .align(Align::new(HAlign::Left, VAlign::Center))
                .margin(Spacing::new(ROW_ICON + chrome.gap, 0.0, 0.0, 0.0))
                .show(ui);
        });
    row.response.left.clicked()
}

/// How far the rollback bar stands clear of the card's inner edge.
const BAR_INSET: f32 = 6.0;

/// Where the build stops, drawn as a rule rather than as a run of dashes.
///
/// In the colour the drawing paints loose geometry, which is the same news said
/// twice: a tail of the recipe is not built, so whatever it would have made is
/// not there to be pinned down.
///
/// One salt however many steps there are, because there is only ever one bar —
/// what is rolled back is a *tail*, and where it starts is the whole of what
/// there is to show.
fn rolled(ui: &mut Ui, theme: &Theme, free: Vec3) {
    let run = theme.chrome.card - (theme.chrome.pad + BAR_INSET) * 2.0;
    pill::line(ui, "rolled", run, 1.0, drawing::tint(free));
}
