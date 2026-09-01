//! The recipe, down the right edge: a row per step, in the order they build.

use palantir::{
    Align, Background, Color, Configure, Corners, FontWeight, HAlign, InternedStr, Panel, Rect,
    Sense, Sizing, Spacing, Text, TextStyle, TextWrap, Ui, VAlign, WidgetId,
};

use crate::build::bodied::Built;
use crate::control::pill::{self, Pill};
use crate::hud::{Shown, control};
use crate::intent::{Choice, Intents};
use crate::look::Theme;
use crate::look::geometry;
use crate::look::icons::{Glyph, Icons};
use crate::look::wearing::{Standing, Wearing};
use crate::marked::{self, Marked};
use crate::part::Part;
use crate::timeline::FeatureId;
use crate::timeline::feature::Feature;
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
pub(super) fn show(ui: &mut Ui, shown: Shown<'_>, doomed: &[FeatureId], intents: &mut Intents) {
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
            let chrome = &theme.chrome;
            // Standing clear of the first row at both ends, so it reads as what
            // the card is called rather than as a row above the list.
            caption(
                ui,
                theme,
                "recipe",
                "RECIPE",
                chrome.ink_dim,
                Spacing::new(chrome.pad, chrome.gap * 0.5, 0.0, chrome.gap),
            );
            let (mut planes, mut sketches, mut solids) = (0, 0, 0);
            // Whether the walk has passed the last step the document built —
            // see [`Models::rolled`](crate::model::models::Models::rolled), which names
            // that step rather than marking the tail below it.
            let mut built = true;
            for (at, feature) in models.chosen() {
                // **What the step came to, matched whole.** The three faults
                // are worded as the status line words them, so a person meets
                // one word for one state — see
                // [`Status`](crate::status::Status) — and coming to nothing is
                // the recipe's alone, the status line reporting what is *wrong*
                // and this listing what is there.
                //
                // Through what a step came to rather than through the faults
                // alone, which is what let a row that built nothing read
                // exactly like one that built: a fifth thing a step can come to
                // is a compile error here instead.
                let came = match models.came_at(at) {
                    Some(Built::Lost) => " · adrift",
                    Some(Built::Unmerged) => " · not merged",
                    Some(Built::Unrounded) => " · refused",
                    Some(Built::Empty) => " · empty",
                    Some(Built::Made) | None => "",
                };
                // Numbered within its kind, and every row is: what the walk
                // holds is what somebody put there, and the three the world
                // comes with — the only steps that carry a name of their own —
                // are not among them. See [`Models::chosen`].
                //
                // Grouped rather than one counter per kind, because what a
                // reader counts is what a step *leaves*: every kind that makes
                // a body is a solid in this list, however it made one.
                let nth = match feature {
                    Feature::Plane(_) => {
                        planes += 1;
                        planes
                    }
                    Feature::Sketch { .. } => {
                        sketches += 1;
                        sketches
                    }
                    Feature::Extrude { .. } | Feature::Revolve { .. } | Feature::Round { .. } => {
                        solids += 1;
                        solids
                    }
                };
                // Off the one table the relation bar and the form read too, so
                // no two of the three can draw a kind differently.
                let Marked { glyph, word } = marked::making(feature);
                // Interned into the pass's own arena rather than formatted into a
                // `String`: this is a row per step per frame, and the record pass is
                // gated at zero allocations.
                let label = ui.fmt(format_args!("{word} {nth}{came}"));
                let showing = Row {
                    at,
                    glyph,
                    label,
                    picked: selection.contains(Part::Step(at)),
                    built,
                    // Scanned rather than hashed, on the terms
                    // [`Timeline::doomed`](crate::timeline::Timeline) states:
                    // a cascade is a handful of steps and is nearly always one.
                    doomed: doomed.contains(&at),
                };
                if row(ui, shown.icons, theme, showing) {
                    // Picked out and nothing else. What follows from picking a step
                    // is decided where everything else about a selection is — a
                    // sketch opens, a plane does not — see
                    // [`Models::opens`](crate::model::models::Models).
                    intents.push(Choice::Select(Some(Part::Step(at))));
                }
                // **The bar, drawn between the rows it divides.** One marker rather
                // than a mark on every row below it: what is rolled back is a
                // *tail*, so where it starts is the whole of what there is to show.
                if models.rolled() == Some(at) {
                    rolled(ui, theme, shown.theme.geometry.free);
                    built = false;
                }
            }
        });
}

/// What one row shows: which step it stands for, and what the document makes
/// of that step.
#[derive(Debug, Clone, Copy)]
struct Row {
    at: FeatureId,
    glyph: Glyph,
    label: InternedStr,
    picked: bool,
    /// What the wear reads off a row, less the hover it asks for itself — see
    /// [`Standing`], where the two are what they mean.
    built: bool,
    /// Read a call before the card is drawn, so the bar that offers the removal
    /// and this cannot disagree about what would go — see
    /// [`Hud::show`](crate::hud::Hud).
    doomed: bool,
}

/// One row, and whether it was pressed.
fn row(ui: &mut Ui, icons: &Icons, theme: &Theme, showing: Row) -> bool {
    let Row {
        at,
        glyph,
        label,
        picked,
        built,
        doomed,
    } = showing;
    let chrome = &theme.chrome;
    let id = step_id(at);
    let hovered = ui.response_for(id).hovered;
    let standing = Standing {
        picked,
        hovered,
        built,
        doomed,
    };
    let wearing = Wearing::row(theme, standing).eased(ui, id, theme);
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

/// How much room the bar leaves at either side of the words on it.
const BAR_GAP: f32 = 6.0;

/// What share of the bar the arm before the words takes.
///
/// Short, and the arm after it takes whatever is left: the words are set flush
/// against the left of the card like every label on it, so a bar centred about
/// them would put the caption where no other row starts.
const BAR_ARM: f32 = 0.18;

/// Where the build stops, drawn as a captioned rule rather than as a run of
/// dashes.
///
/// **Said as well as drawn.** A rule across a list is a division and nothing
/// more; what a person has to know is that everything under it is *not built*,
/// and no line says that on its own. So the rule carries the words, and the two
/// halves of it are what makes them read as a division rather than as a row.
///
/// In the colour the drawing paints loose geometry, which is the same news said
/// twice: a tail of the recipe is not built, so whatever it would have made is
/// not there to be pinned down.
///
/// One salt however many steps there are, because there is only ever one bar —
/// what is rolled back is a *tail*, and where it starts is the whole of what
/// there is to show.
fn rolled(ui: &mut Ui, theme: &Theme, free: Vec3) {
    let chrome = &theme.chrome;
    let tint = geometry::tint(free);
    let full = chrome.card - (chrome.pad + BAR_INSET) * 2.0;
    Panel::hstack()
        .id_salt("rolled")
        .size((Sizing::fixed(full), Sizing::fixed(ROW * 0.5)))
        .align(Align::CENTER)
        .gap(BAR_GAP)
        .background(Background::NONE)
        .show(ui, |ui| {
            // The arm before the words is measured and the one after it fills:
            // a stated pair would have to be told how wide the caption came
            // out, which the shaper decides.
            pill::line(ui, "rolled.left", full * BAR_ARM, 1.0, tint);
            caption(ui, theme, "rolled", "ROLLED BACK", tint, Spacing::ZERO);
            pill::filling_line(ui, "rolled.right", 1.0, tint);
        });
}

/// A surface's own name, or the words on a division of one.
///
/// Upper case and small, which is the whole of how a caption is told from a
/// row: the rows are what the list is *for*, and a heading set at their size
/// and in their case would read as one more of them.
fn caption(
    ui: &mut Ui,
    theme: &Theme,
    salt: &'static str,
    text: &'static str,
    color: Color,
    margin: Spacing,
) {
    let style = TextStyle {
        color,
        font_size_px: theme.chrome.caption_text,
        weight: FontWeight::Bold,
        ..TextStyle::default()
    };
    Text::new(text)
        .id_salt(salt)
        .style(&style)
        .size((Sizing::HUG, Sizing::HUG))
        .align(Align::new(HAlign::Left, VAlign::Center))
        .margin(margin)
        .show(ui);
}
