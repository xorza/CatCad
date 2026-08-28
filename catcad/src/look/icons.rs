//! The artwork the overlay draws its controls with.

use std::rc::Rc;

use palantir::{IconAtlas, IconId, IconSet, IconShape, Ui};

/// One icon of the set, named for what it stands for rather than for what it
/// draws.
///
/// An enum rather than sixteen fields, because the set is walked as often as it
/// is indexed: [`Icons`] resolves every id in one pass over [`SOURCES`], and a
/// control names what it wants by naming one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Glyph {
    Pointer,
    Point,
    Line,
    Circle,
    Dimension,
    Tidy,
    Remove,
    Finish,
    New,
    Open,
    Save,
    Plane,
    Sketch,
    Extrude,
    Revolve,
    Perspective,
    Orthographic,
    Fit,
    Join,
    Cut,
    Intersect,
    Confirm,
    Cancel,
}

/// Every icon, and the source it is drawn from.
///
/// **In the order [`Glyph`] declares them**, which is what lets [`Icons`] index
/// by `glyph as usize` rather than search. The atlas sorts its own table by
/// name, so the ids are resolved back by name into this order — the two orders
/// are unrelated and neither may be assumed of the other.
const SOURCES: [(Glyph, &str, &str); 23] = [
    (Glyph::Pointer, "pointer", POINTER),
    (Glyph::Point, "point", POINT),
    (Glyph::Line, "line", LINE),
    (Glyph::Circle, "circle", CIRCLE),
    (Glyph::Dimension, "dimension", DIMENSION),
    (Glyph::Tidy, "tidy", TIDY),
    (Glyph::Remove, "remove", REMOVE),
    (Glyph::Finish, "finish", FINISH),
    (Glyph::New, "new", NEW),
    (Glyph::Open, "open", OPEN),
    (Glyph::Save, "save", SAVE),
    (Glyph::Plane, "plane", PLANE),
    (Glyph::Sketch, "sketch", SKETCH),
    (Glyph::Extrude, "extrude", EXTRUDE),
    (Glyph::Revolve, "revolve", REVOLVE),
    (Glyph::Perspective, "perspective", PERSPECTIVE),
    (Glyph::Orthographic, "orthographic", ORTHOGRAPHIC),
    (Glyph::Fit, "fit", FIT),
    (Glyph::Join, "join", JOIN),
    (Glyph::Cut, "cut", CUT),
    (Glyph::Intersect, "intersect", INTERSECT),
    // The same tick a sketch is finished with, under a second name. What the
    // two mean is not one thing — one ends a sketch, the other answers a form
    // — and a [`Glyph`] is named for what it stands for, so the pair share the
    // artwork rather than the role.
    (Glyph::Confirm, "confirm", FINISH),
    (Glyph::Cancel, "cancel", CANCEL),
];

/// The loaded icon set, and where each [`Glyph`] sits in it.
///
/// **An owner.** The [`IconSet`] holds the host's parse of every source and the
/// rasters it has made of them, and dropping the last clone unloads all three —
/// which is why a shape is asked of the set rather than minted once from a
/// handle kept beside it. A handle owns nothing, so one outliving its set names
/// a set the host has let go and panics when the renderer draws it.
#[derive(Debug, Clone)]
pub(crate) struct Icons {
    set: IconSet,
    ids: [IconId; SOURCES.len()],
}

impl Icons {
    /// Take up the set for this frame.
    ///
    /// **Every frame, rather than the first one only**, and that is not waste.
    /// A set is registered against the *host* that will draw it, so one taken
    /// up under one host names nothing under another — and the visual suite
    /// paints one app through two, which is exactly the case a set held from
    /// the first frame gets wrong.
    ///
    /// **What comes back has to outlive the frame**, which is why the caller
    /// parks it: the record pass writes a shape naming the set, and the paint
    /// that reads it runs at submit, after recording has returned.
    ///
    /// Built through [`IconAtlas::from_svgs`] rather than baked into the
    /// binary. What baking saves is one parse per icon — about three
    /// milliseconds across the set, paid once on the frame the overlay first
    /// draws — and what it costs is a generator and a table nobody may edit by
    /// hand. Sixteen icons do not earn that; a hundred would.
    ///
    /// The atlas itself is built once and parked, because `load_icons`
    /// recognises the same allocation and hands back a clone of the set
    /// registered against it, with no parsing, no upload and no allocation. A
    /// fresh `Rc` each time would register a second set and re-rasterize the
    /// whole of it.
    pub(crate) fn load(ui: &Ui) -> Self {
        thread_local! {
            static BUILT: Rc<IconAtlas> = Rc::new(IconAtlas::from_svgs(
                SOURCES.map(|(_, name, svg)| (name, svg)),
            ));
        }
        let set = BUILT.with(|atlas| ui.load_icons(Rc::clone(atlas)));
        let ids = SOURCES.map(|(_, name, _)| {
            set.by_name(name)
                .expect("every source in the table is in the set built from it")
        });
        Self { set, ids }
    }

    /// The artwork for `glyph`, ready to be placed and tinted.
    pub(crate) fn shape(&self, glyph: Glyph) -> IconShape {
        self.set.shape(self.ids[glyph as usize])
    }
}

// One colour throughout every source below, which is what makes each of them
// tintable: an icon whose every paint resolves to one colour rasterizes to a
// coverage mask and takes a draw's tint whole, so one piece of artwork serves
// the resting, lit and held looks alike.

const POINTER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 3l13 8-5.5 1.4L10.6 18z" fill="#fff" stroke="#fff" stroke-width="1.5" stroke-linejoin="round"/></svg>"##;

const POINT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="2.6" fill="#fff"/><path d="M12 3v3.4M12 17.6V21M3 12h3.4M17.6 12H21" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/></svg>"##;

const LINE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5.5 18.5L18.5 5.5" fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round"/><circle cx="5.5" cy="18.5" r="2.3" fill="#fff"/><circle cx="18.5" cy="5.5" r="2.3" fill="#fff"/></svg>"##;

const CIRCLE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="7.4" fill="none" stroke="#fff" stroke-width="1.7"/><circle cx="12" cy="12" r="1.7" fill="#fff"/></svg>"##;

const DIMENSION: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 6v12M20 6v12M6.6 12h10.8M9 9.6L6.4 12 9 14.4M15 9.6L17.6 12 15 14.4" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

const TIDY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M9.5 20.5L20.5 9.5a2.6 2.6 0 0 0 0-3.7l-2.3-2.3a2.6 2.6 0 0 0-3.7 0L3.5 14.5z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M9.5 20.5H20.5M11.5 6.5l6 6" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/></svg>"##;

const FINISH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4.5 12.6l4.8 4.8L19.5 7.2" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

const NEW: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M6 2.8h7.6L19 8.2v13H6z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M13.4 2.8v5.6H19" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M12.5 11.6v6.2M9.4 14.7h6.2" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/></svg>"##;

const OPEN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 18.5V6.2A1.2 1.2 0 0 1 4.2 5h4.9l2.1 2.6h7.6A1.2 1.2 0 0 1 20 8.8v1.7" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M3 18.5l2.7-8h16L19 18.5z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>"##;

const SAVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 4.8h11.4L20 9.4V19a1.2 1.2 0 0 1-1.2 1.2H5.2A1.2 1.2 0 0 1 4 19z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M8 4.8v5h7v-5M7.5 20.2v-5.6h9v5.6" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>"##;

const REMOVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4.5 6.8h15M9.4 6.8V4.4h5.2v2.4" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/><path d="M6.6 6.8l.9 13.4h9l.9-13.4" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M10.4 10.4v6.4M13.6 10.4v6.4" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/></svg>"##;

const PLANE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2.5 16.5L9 7.5h12.5L15 16.5z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>"##;

const SKETCH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 18.5l5-7 4 3.2 3.2-5.7L21 14" fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="3" cy="18.5" r="1.8" fill="#fff"/><circle cx="21" cy="14" r="1.8" fill="#fff"/></svg>"##;

const EXTRUDE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2.5 19.5L8 12.5h13.5L16 19.5z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M12 10.5V3M12 3L9.3 5.9M12 3l2.7 2.9" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

const REVOLVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M5 3v18" fill="none" stroke="#fff" stroke-width="1.5" stroke-linecap="round" stroke-dasharray="2.6 2.6"/><path d="M9.5 8h5.5v8H9.5z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M15 5.4c3.4 1 5.6 3.5 5.6 6.6 0 3.1-2.2 5.7-5.6 6.6" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round"/><path d="M13.2 3.6l2.4 1.9-2.6 1.7" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

const PERSPECTIVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 4.5l16 3v9l-16 3z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M4 12h16" fill="none" stroke="#fff" stroke-width="1.6"/></svg>"##;

const ORTHOGRAPHIC: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3.5 8.5h12v12h-12z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M3.5 8.5L8 4h12v12l-4.5 4.5M15.5 8.5L20 4" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>"##;

const FIT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3.5 8.5v-5h5M15.5 3.5h5v5M20.5 15.5v5h-5M8.5 20.5h-5v-5" fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12" cy="12" r="2.4" fill="#fff"/></svg>"##;

// The three sweeps share one pair of squares — A at the top left, B at the
// bottom right — and differ only in which region is drawn solid. What is kept
// is stroked whole; what is consumed is dashed. Told apart by *shape* rather
// than by a symbol, so none of them has to be already known.

const JOIN: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 3h12v6h6v12H9v-6H3z" fill="none" stroke="#fff" stroke-width="1.7" stroke-linejoin="round"/><path d="M9 15V9h6" fill="none" stroke="#fff" stroke-width="1.2" stroke-linejoin="round"/></svg>"##;

const CUT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M9 9h12v12H9z" fill="none" stroke="#fff" stroke-width="1.3" stroke-linejoin="round" stroke-dasharray="2.4 2.2"/><path d="M3 3h12v6H9v6H3z" fill="none" stroke="#fff" stroke-width="1.7" stroke-linejoin="round"/></svg>"##;

const INTERSECT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 3h12v12H3zM9 9h12v12H9z" fill="none" stroke="#fff" stroke-width="1.3" stroke-linejoin="round" stroke-dasharray="2.4 2.2"/><path d="M9 9h6v6H9z" fill="none" stroke="#fff" stroke-width="1.7" stroke-linejoin="round"/></svg>"##;

const CANCEL: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M6.6 6.6l10.8 10.8M17.4 6.6L6.6 17.4" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round"/></svg>"##;

#[cfg(test)]
mod tests {
    use crate::look::icons::SOURCES;

    /// **Every source sits at its own glyph's index, and paints in one
    /// colour.** Two invariants the set rests on, and neither one fails
    /// loudly.
    ///
    /// [`Icons::shape`] indexes by `glyph as usize`, so a variant added to
    /// [`Glyph`] without a row here at the same place still compiles: it
    /// shifts every source after it, and a whole run of controls quietly draws
    /// its neighbour's artwork.
    ///
    /// The colour is the other half. An icon takes a tint only where its every
    /// paint resolves to one colour — a second one makes it an image, which is
    /// then drawn in the artwork's own colours whatever the control asked for.
    ///
    /// A name resolving to nothing needs no test: [`Icons::load`] drops an
    /// unparseable source and then panics on the name it cannot find.
    #[test]
    fn every_source_sits_at_its_own_glyph_and_paints_in_one_colour() {
        for (at, (glyph, name, svg)) in SOURCES.iter().enumerate() {
            assert_eq!(
                *glyph as usize, at,
                "{name} is the {at}th source and {glyph:?} is not the {at}th glyph",
            );
            for (from, _) in svg.match_indices('#') {
                let rest = &svg[from..];
                let end = rest.find('"').expect("a paint is a quoted attribute");
                assert_eq!(
                    &rest[..end],
                    "#fff",
                    "{name} paints in a second colour, so it will not take a tint",
                );
            }
        }
        for (at, (_, one, _)) in SOURCES.iter().enumerate() {
            for (_, two, _) in &SOURCES[at + 1..] {
                assert_ne!(one, two, "two sources answer to {one}");
            }
        }
    }
}
