//! Every label of the drawing, as the renderer holds one.

use aperture::{Batch, Facing, Precedence, Text, Turn};
use glam::Vec2;
use std::fmt::Write;

use crate::look::Theme;
use crate::model::models::Models;
use crate::paint::marks::{Placed, Proposed};
use crate::paint::names::Names;
use crate::paint::{DECIMALS, MARK_FONT, SHEET_NAME_LIFT, marks, shade, standing, symbol};
use crate::part::Part;
use crate::wording;

use crate::paint::write::Marked;

/// A mark for every relation the drawing states, saying what holds and where.
///
/// Set in type rather than drawn as geometry, which is what makes the whole set
/// one rule: every relation gets a symbol, the symbol is legible at any zoom
/// because it is sized in pixels, and adding a tenth constraint is a line in
/// [`symbol`] rather than a shape to construct.
///
/// **Turned into the sketch's plane**, so a mark reads as lettering on the
/// drawing rather than as a note pinned over it. Only the *direction* it runs
/// in: it is still sized in pixels, so the zoom cannot reach it and neither can
/// the angle the plane is seen at — see [`Facing`]. Which way up it comes out is
/// the renderer's, and always the way that reads.
///
/// One mark *or two*, and stacked where several want one place — see
/// [`marks::stacked`], which decides all of how many, where, and how high.
/// Where there are two they carry the same name, so a click on either takes
/// the constraint.
///
/// Tagged like everything else, so a mark is picked and deleted the way the
/// geometry it is about is — which is the whole of how an over-constrained
/// sketch gets un-stuck.
pub(crate) fn write(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    placed: &mut Vec<Placed>,
    proposed: Option<Proposed>,
    typed: Option<Part>,
    into: &mut Batch<Text>,
) {
    let geometry = &theme.geometry;
    // **The open sketch alone.** A constraint is a statement *about* a drawing,
    // and one you are not in is not a drawing you can argue with: its marks can
    // neither be selected into a relation nor typed into, so all they do is
    // crowd the sketch you are working in — and a dimension is the densest
    // thing the drawing puts on screen. The geometry of a dormant sketch still
    // shows, dimmed, because where it *is* is something you build against.
    //
    // No sketch open is no marks at all — and the planes take the batch over
    // instead, which is why this hands off rather than emptying it. The
    // placements still have to be cleared: they are kept across frames for the
    // rules drawn under them a phase later, and there are none — see
    // [`gizmos::ruled`](crate::paint::gizmos).
    let Some(live) = models.open() else {
        placed.clear();
        return named_planes(models, theme, names, into);
    };
    // Laid out whole, before anything is left out. What lane a mark rises in
    // depends on how many share its place, so a stack that was worked out from
    // what is *shown* would close ranks the moment a field opened over one of
    // them — and closing ranks under a double-click reads as the click having
    // nudged the drawing.
    marks::stacked(live, placed);
    into.refill(
        placed
            .iter()
            // The one being retyped has a field standing over it — see
            // [`Prompt::show`](crate::prompt::Prompt) — and a mark left
            // under one would be a second copy of the number showing
            // through wherever the field did not quite cover it.
            .filter(move |placed| Some(live.part(placed.of)) != typed)
            .map(|placed| Marked::Stated(*placed))
            // Last, so a dimension being placed is written over the drawing
            // rather than under it — and so the tags the drawing handed out
            // are the same whether or not a tool is half-way through one.
            .chain(proposed.map(Marked::Proposed)),
        |mark, marked| {
            let placed = marked.mark();
            let constraint = marked.constraint(live.sketch());
            // Rewritten in place rather than assigned, so a drawing whose marks are
            // laid out every frame keeps the string it already has — which is what
            // keeps a scrubbed dimension off the heap sixty times a second.
            mark.content.clear();
            match constraint.value() {
                // A dimension reads as its measurement. That *is* the mark: a
                // number beside a length says both that the length is stated and
                // what it is stated as, where a symbol would say only the first and
                // leave the drawing unreadable.
                Some(value) => {
                    let prefix = wording::of(constraint).prefix;
                    write!(mark.content, "{prefix}{value:.*}", DECIMALS)
                        .expect("writing to a string cannot fail");
                }
                None => mark.content.push_str(symbol(constraint)),
            }
            let plane = live.plane();
            mark.position = plane.point(placed.at).as_vec3();
            mark.font = MARK_FONT;
            // **Centred on its own box**, with the clearance carried by the
            // lift below instead. An anchor fraction rides in the run's own
            // frame, and both rules that settle that frame — the mirror and the
            // half turn — would carry it along, swinging the box to the other
            // side of the very line it stands clear of. A centred box is mapped
            // onto itself by either, so it only ever changes direction.
            mark.anchor = Vec2::splat(0.5);
            mark.color = match marked {
                // A proposal has no state to report: the constraints have not
                // been asked about it, so it cannot be redundant and cannot be
                // anything else either. The grey a rubber band wears, and for
                // the same reason — it is not in the drawing yet.
                Marked::Proposed(..) => geometry.ghost,
                Marked::Stated(stated) if live.outcome().is_redundant(stated.of) => {
                    shade(theme, live, geometry.redundant)
                }
                Marked::Stated(..) => shade(theme, live, geometry.mark),
            };
            mark.precedence = standing(live);
            // Lettered on the drawing rather than pinned over it: set along the
            // geometry it is about — the span a dimension measures, the edge a
            // symbol names — so a number reads as belonging to the line under it
            // and turns with the plane it belongs to. Which direction that is,
            // is [`marks::anchors`]'s; what is here is putting it on the plane.
            //
            // Clear of that geometry by the lift, which is stated in the plane's
            // own axes and so is the one thing about a mark the projection
            // cannot move.
            mark.facing = Facing::Turned(placed.turn(live.drawing()));
            // Untagged where it is a proposal, which is what keeps it out of the
            // way: a pick skips a primitive with no tag, so the click that
            // *commits* the dimension resolves against the geometry behind it
            // rather than against the picture of what it is about to make.
            mark.tag = match marked {
                Marked::Stated(stated) => Some(names.tag(live.part(stated.of))),
                Marked::Proposed(..) => None,
            };
        },
    );
}

/// A name against each plane, for a document being looked at rather than drawn
/// in.
///
/// **The same batch the marks use, and never at the same time.** A name says
/// which plane you would be starting on, which is worth knowing exactly when
/// there is no drawing to start from; a mark says what a drawing states, of
/// which there is none. So the two are one batch's two contents rather than two
/// batches, and the refill that writes either is the one that clears the other.
///
/// **Laid into the plane rather than pinned over it**, which is what makes a
/// name read as belonging to the sheet it is on: it runs along that plane's own
/// +x and takes its depth from it, so the plane can hide it and turning the
/// model turns it. The two rules that keep a laid run legible — the mirror that
/// answers a camera behind the plane, and the half turn that keeps it upright —
/// are [`Turn`]'s own, so nothing here has to know where the eye
/// is.
///
/// **Inside the top-left corner of the plane's square**, running along that
/// plane's own +x — a title's place, and the corner a reader's eye starts from.
/// Reached by anchoring the run centred on the plane's origin, where the square
/// is, and carrying it out with a lift: the lift is stated in logical pixels and
/// resolved where the run is drawn, so it lands in the same corner of a square
/// that is itself a fixed number of pixels, at any zoom and whatever that
/// number becomes. It is also the only one of the three ways to shift a run that
/// survives the two rules above — see [`SHEET_NAME_LIFT`](crate::paint).
///
/// A plane somebody put there carries no name: until steps have names of their
/// own every one of them would read the same word — see
/// [`World::named`](crate::timeline::feature::World).
fn named_planes(models: Models<'_>, theme: &Theme, names: &mut Names, into: &mut Batch<Text>) {
    into.refill(
        models
            .planes()
            .filter_map(|sheeted| Some((sheeted, sheeted.world?.named()))),
        |text, (sheeted, named)| {
            // Written over what is there rather than assigned, like a mark and
            // for the same reason: a `Text` owns its content on the heap.
            text.content.clear();
            text.content.push_str(named);
            let plane = sheeted.plane;
            text.position = plane.origin.as_vec3();
            text.font = MARK_FONT;
            text.anchor = Vec2::splat(0.5);
            text.facing = Facing::Turned(
                Turn::new(plane.x.as_vec3(), plane.normal().as_vec3()).lifted(SHEET_NAME_LIFT),
            );
            text.color = theme.geometry.sheet_ink(sheeted.world);
            // A frame, which does two things and both matter. It yields a
            // click to anything ordinary, so a name lying over the model cannot
            // take one; and it is left out of how far the scene reaches — so a
            // camera is not framed on a label whose distance from the origin is
            // a number of pixels rather than anything the model said.
            text.precedence = Precedence::Frame;
            text.tag = Some(names.tag(Part::Step(sheeted.at)));
        },
    );
}
