//! What a drawing looks like: turning one into the strokes, rims and markers a
//! renderer is handed.
//!
//! Everything here is a choice about appearance — which colour says how much
//! freedom is left, how wide an edge is, how far the drawing rides in front of
//! the solids — held apart from the model it is applied to so that neither has
//! to be read to change the other. It is also where the model's `f64` becomes
//! the renderer's `f32`, and the only place it does.
//!
//! The drawing only. What a drawing puts on screen to be *used* rather than
//! read — a datum's axes, the arrow carrying a solid's depth — is
//! [`gizmos`], held apart because it is written on the camera's schedule
//! rather than the document's.
//!
//! **What is in this file is the deciding**, and the two calls that spend it:
//! [`scene`] makes a picture and [`redraw`] keeps one current. Turning geometry
//! into primitives is [`write`](mod@write), a writer per batch; where a mark's *box* lands
//! on screen is [`Mark`](marks::mark::Mark)'s, being the one part of a drawing
//! measured in pixels after it has been laid out.

use aperture::{Precedence, Scene};
use glam::{Vec2, Vec3};
use palantir::{FontFamily, FontWeight, GlyphFont};
use silverpoint::Constraint;

use crate::look::ink::DORMANT;
use crate::model::{Model, Models};
use crate::paint::layout::{Layout, Made, Stage};
use crate::paint::showing::Showing;
use crate::preview::Preview;
use crate::wording;

pub(crate) mod cut;
pub(crate) mod gizmos;
pub(crate) mod growing;
pub(crate) mod layout;
pub(crate) mod marks;
pub(crate) mod names;
pub(crate) mod showing;
pub(crate) mod write;

/// Marker diameters in logical pixels. A pinned point reads larger because it
/// is the one the drawing hangs off.
const FIXED_MARKER: f32 = 9.0;
const FREE_MARKER: f32 = 7.0;

/// How far a face's edge may sit from the curve it was cut from, in sketch
/// units.
///
/// A face is flattened once, when the drawing moves, rather than again whenever
/// the camera does — so this is chosen for the drawing rather than for the
/// zoom. At the size a sketch is worked at it puts forty-odd corners around a
/// rim, which reads round without giving the triangulation a hundred to chew.
const FACE_SAGITTA: f64 = 0.005;

/// The same, for the walls and ends of a solid.
///
/// Its own number rather than [`FACE_SAGITTA`] reused, because the two are read
/// at different sizes: a region is a flat sheet seen against the plane it lies
/// in, and a solid is turned about and lit, so a wall that is a shade too coarse
/// shows up as banding across a shaded surface where the same coarseness on a
/// flat fill shows up as nothing at all.
const SOLID_SAGITTA: f64 = 0.002;

/// How far a plane's square reaches from its middle, in logical pixels.
///
/// **On screen rather than in the world**, which is what a plane's square is
/// *for*: a plane has no edges, so the square is a symbol for one rather than a
/// measurement of it, and one sized off the drawing moved every time the drawing
/// did. Sized here it holds still while a sketch is edited, and holds its size
/// as the camera pulls back — which is what every other handle a drawing puts on
/// screen already does, and why the square is cut with them in
/// [`gizmos::shape`](gizmos).
///
/// What it costs is the fill: a sheet built against the camera is rewritten when
/// the camera moves, and nothing that fills is written on that schedule. So a
/// plane is an outline and a name.
const SHEET_REACH: f64 = 40.0;

/// How far in from the top-left corner of its square the *middle* of a plane's
/// name sits, in logical pixels across and down that plane's own axes.
///
/// **In from the corner rather than the corner itself**, because what the lift
/// below carries is the middle of the run: a name set at the corner exactly
/// would straddle both edges and hang half of itself outside the square. So
/// this is the padding that holds it clear of the two edges, plus half the run
/// it has to hold clear of them.
///
/// Half a run is a number that can be written down here only because every name
/// is two characters of one mono face — so half of one is a single character's
/// body, and [`MARK_FONT`] fixes what that is. Nothing here can *measure* a run:
/// a [`Text`](aperture::Text) is measured by the pass that paints it, and this
/// is decided before there is a frame. A third character or a proportional face
/// and this stops being knowable.
const SHEET_NAME_INSET: Vec2 = Vec2::new(16.0, 13.0);

/// Where a plane's name sits, as a displacement from the middle of its square in
/// logical pixels along that plane's own +x and +y.
///
/// **Off the square's own reach**, so the name follows it: the two used to be
/// set apart from each other, and shrinking the square left the names orbiting
/// where its corners had been — far enough out that one plane's name landed
/// inside another plane's square and read as labelling it.
///
/// Carried as a [`Turn`](aperture::Turn)'s lift rather than as a position or an
/// anchor fraction, and none of the three is interchangeable. A *position* would
/// have to be worked out in the world against the camera, which is the schedule
/// the drawing is deliberately not on. An offset written into the *anchor* rides
/// in a frame two camera-dependent rules settle — the mirror that keeps a run
/// readable from behind its plane, and the half turn that keeps it upright — so
/// a name tucked into a corner from one side sticks out of it from the other,
/// which is exactly what a look from behind showed. A lift is stated in the
/// plane and holds.
const SHEET_NAME_LIFT: Vec2 = Vec2::new(
    -(SHEET_REACH as f32) + SHEET_NAME_INSET.x,
    SHEET_REACH as f32 - SHEET_NAME_INSET.y,
);

/// How wide a plane's outline is, in logical pixels.
///
/// Under [`EDGE_WIDTH`], so a plane's own edge cannot be taken for something
/// drawn on it.
const SHEET_WIDTH: f32 = 1.0;

/// `lit` where `model` is the sketch being edited, and ground where it is not.
///
/// The one place the difference is made, so every kind of mark in a dormant
/// sketch is dimmed by the same rule rather than each writer having its own
/// idea of what "not here" looks like.
fn shade(model: Model<'_>, lit: Vec3) -> Vec3 {
    if model.live() { lit } else { DORMANT }
}

/// Where a sketch's marks stand in the competition for a click.
///
/// The same branch as [`shade`] beside it, and for the same reason: a sketch
/// nobody is working in is drawn to be read rather than aimed at, so a click
/// that lands on it and on the open sketch at once was meant for the open one.
fn standing(model: Model<'_>) -> Precedence {
    if model.live() {
        Precedence::Shaped
    } else {
        Precedence::Aside
    }
}

/// How wide a sketch stroke is drawn, in logical pixels.
///
/// Not [`aperture::Curve`]'s own default, which is narrower: a drawing is read
/// at a glance against a shaded model behind it and wants a little more weight
/// than a bare overlay does. Every stroke and every rim is set to this, so the
/// default is never seen — and the visual suite measures against it through
/// `internals`, rather than keeping a second opinion about what it should be.
pub(super) const EDGE_WIDTH: f32 = 1.6;

/// Type size of a constraint's mark, in logical pixels. Small: a drawing may
/// carry dozens, and what they have to be is legible rather than prominent.
const MARK_SIZE: f32 = 13.0;

/// What a mark is set in.
///
/// Mono and bold, which are two claims about legibility rather than about
/// style. A mark is one character read at a glance against a drawing behind it,
/// so it wants the weight to hold its own over a stroke it may be sitting on —
/// and the mono face is drawn on a fixed body, so ⊥ and ∥ and ∈ come out the
/// same size as each other instead of set to the widths a running line would
/// want.
///
/// Named rather than written where it is used, so that
/// `every_relation_is_named_both_ways_and_every_mark_has_a_glyph` asks about the faces the drawing
/// actually sets marks in. A coverage check against a font nobody uses would
/// pass while the drawing showed nothing.
///
/// A constant beside the sizes and colours around it, because nothing about a
/// mark, a frame or a camera moves it — and
/// [`Mark::rise`](crate::paint::marks::mark::Mark) reads it once per mark on
/// the drawing, every frame.
pub(crate) const MARK_FONT: GlyphFont = GlyphFont {
    family: FontFamily::Mono,
    weight: FontWeight::Bold,
    ..GlyphFont::new(MARK_SIZE)
};

/// The whole picture of a document as it stands — the `solids` standing around
/// it, its drawing over them, and a name for every part that can be pointed at.
///
/// Where a scene comes from, and the only place one does. Hands back a fresh
/// scene rather than filling one the caller owns, which is the shape the cost
/// deserves: the meshes are copied across, and handing a renderer its objects
/// again has it upload them again. Anything wanting this every frame wants
/// [`redraw`] instead — and the two are shaped as differently as they are so
/// that reaching for the wrong one is a change of code rather than of nothing.
///
/// Everything in the picture comes out of the document, solids included: a
/// step grows one, so it is laid out with the drawing and by the same call —
/// which is also what lets one be pointed at. Nothing is handed in beside the
/// model, and there is nowhere in the signature for anything to be.
///
/// The controls are the one thing not here, because they are built against a
/// camera rather than against the document. See [`gizmos::write()`].
pub(crate) fn scene(models: Models<'_>, layout: &mut Layout) -> Scene {
    let mut scene = Scene::default();
    // Nothing half-done: no band, nothing being retyped and nothing being grown
    // in a document nobody has looked at yet.
    redraw(models, layout, Showing::default(), &mut scene);
    scene
}

/// Draw the whole of `model`, and `band` over it, into the room `layout` keeps
/// and under the names it holds.
///
/// The half of a picture that moves. A drawing is edited and the solids beside
/// it are not, so this rewrites what the drawing is made of — the four overlay
/// batches, and the sheets its curves enclose — and leaves `into.solids`
/// untouched, which is what keeps a drag from re-uploading every mesh in the
/// model: a batch nobody wrote to reports nothing to upload.
///
/// Fills buffers rather than returning them, so a drag refills what the renderer
/// already holds instead of handing it new vectors every frame. The tags come
/// out the same across a rewrite, because they are positions in a list built in
/// the same order — which is what lets a drag keep hold of what it grabbed.
///
/// `layout` is the caller's, not the drawing's. A tag is an index into a list
/// of what was drawn, so it describes a *layout* of a drawing and not the
/// drawing itself — nothing in one would be written down by saving, and whoever
/// laid the drawing out is who has to be able to read its tags back. Its names
/// are emptied here rather than by the caller, because a name list half from
/// one layout and half from another names nothing.
///
/// What a gesture is half-way through arrives in `showing` and the drawing
/// knows nothing about it — which is why it comes in here rather than off
/// [`Drawing`](crate::drawing::Drawing). The band is written among the strokes
/// and rims and never named, so it cannot be picked; see [`write::curves`]
/// and [`write::rings`].
///
/// Does nothing where the layout already describes what it would draw, and says
/// so on the layout when it has drawn — so a caller settles a frame by calling
/// this and reading nothing back.
///
/// **How much it draws is [`Stage`]'s to decide.** A redraw is a suffix of the
/// writers below: the layout says which stage what has moved reaches, the names
/// are wound back to where that stage began, and everything from there is
/// written afresh. So the band a tool is drawing rewrites the strokes it is
/// among and nothing else, where it once put every region and every face of
/// every solid through the filler again on each frame the pointer moved.
///
/// Which is what decides the order the writers stand in. It is not a drawing
/// order — each fills its own batch and the renderer draws them in its own
/// sequence — it is a *naming* order, and it runs from what a gesture cannot
/// move to what it moves every frame.
pub(crate) fn redraw(models: Models<'_>, layout: &mut Layout, showing: Showing, into: &mut Scene) {
    // The check is here rather than at the call, so that what a layout claims
    // to describe and what was drawn into it are decided in one place. A caller
    // that skipped the call would leave a stale picture; one that made it and
    // forgot to stamp would redraw for ever.
    let made = Made::of(models, showing);
    let Some(from) = layout.resume(made) else {
        return;
    };
    let Layout {
        names,
        sheets,
        placed,
        proposed,
        ..
    } = &mut *layout;
    names.wind_back(from);
    // The writers below take what they draw and not the bundle: what a stroke
    // wants of a gesture is the band, and handing each of them the whole of it
    // would be handing every writer the two thirds that are not theirs.
    //
    // Every stage opens by saying where its names begin, including the one that
    // was resumed at — whose start is where the wind-back has just left the
    // list, so it says the same thing twice rather than being a case of its own.
    if from <= Stage::Drawing {
        names.opened(Stage::Drawing);
        write::points(models, names, &mut into.points);
        write::faces(models, names, sheets, &mut into.faces);
    }
    if from <= Stage::Solid {
        names.opened(Stage::Solid);
        write::solids(models, names, sheets, showing.growing, &mut into.solids);
    }
    if from <= Stage::Marks {
        names.opened(Stage::Marks);
        // Where the dimension being placed would put its mark, worked out once
        // for the figure written here and the rule written against the camera —
        // see [`Proposed`](marks::Proposed).
        // Nothing proposed where nothing is open: a dimension being placed is
        // one a *tool* is half-way through, and a tool draws in the sketch you
        // are in.
        *proposed = models
            .open()
            .zip(showing.band.and_then(Preview::dimension))
            .and_then(|(open, constraint)| marks::Proposed::of(open.sketch(), constraint));
        write::texts(
            models,
            names,
            placed,
            *proposed,
            showing.typed,
            &mut into.texts,
        );
    }
    if from <= Stage::Band {
        names.opened(Stage::Band);
        // One spelling of what a band is and where it lies, for the two writers
        // that draw one — see [`write::Band`]. A band is a stroke or a rim and
        // never both, so at most one of the calls below is handed anything and
        // the other has read nothing by the time it answers `None`.
        write::curves(
            models,
            names,
            write::Band::new(models, showing.band.and_then(Preview::line)),
            &mut into.curves,
        );
        write::rings(
            models,
            names,
            write::Band::new(models, showing.band.and_then(Preview::ring)),
            &mut into.rings,
        );
    }
    // Where the controls start naming from — see [`gizmos::write()`].
    names.drew();
    layout.drawn(made);
}

/// Decimal places a dimension is read out to.
///
/// Two, which is a hundredth of a sketch unit — fine enough to draw with and
/// coarse enough that a solve's own drift never shows. What a *unit* is remains
/// the document's to decide; until it decides, a number is a number.
pub(crate) const DECIMALS: usize = 2;

/// The mark a relation is drawn as.
///
/// Read off [`wording`], which is where a relation's word and
/// its mark are stated together — the bar captions what this draws, and the two
/// were a table apiece.
///
/// The four that carry a number never reach here: a dimension is drawn as its
/// measurement, which is the arm above the call. A mark for one would be a
/// second thing the drawing could show for the same relation, and nothing would
/// say which was meant — so there is not one, and asking for it is a caller that
/// failed to read [`Constraint::value`] first.
fn symbol(constraint: Constraint) -> &'static str {
    wording::named(constraint)
        .glyph
        .expect("a dimension is drawn as its number, not as a mark")
}

/// Where `models` says its markers stand, drawn into a scene of its own.
///
/// The question a drag moves and an undo has to put back, and the one thing
/// that answers it without a view: the document is redrawn from nothing rather
/// than read off whatever scene a renderer is still holding, so what comes back
/// is what the drawing *is* rather than what was last painted.
///
/// Here rather than in either harness that wants it, which is both of them —
/// the history's tests and the view's, one asking the opening sketch and the
/// other the open one, and neither able to see the other's copy.
#[cfg(test)]
pub(crate) fn markers(models: crate::model::Models<'_>) -> Vec<glam::Vec3> {
    let mut scene = Scene::default();
    redraw(
        models,
        &mut Layout::default(),
        Showing::default(),
        &mut scene,
    );
    scene.points.iter().map(|point| point.position).collect()
}

#[cfg(test)]
mod tests;
