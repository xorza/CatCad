//! The artwork the overlay draws its controls with.

use std::rc::Rc;

use palantir::{IconAtlas, IconHandle, IconSet, Ui};

/// One icon of the set, named for what it stands for rather than for what it
/// draws.
///
/// An enum rather than sixteen fields, because the set is walked as often as it
/// is indexed: [`Icons`] resolves every handle in one pass over [`SOURCES`],
/// and a control names what it wants by naming one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Glyph {
    Pointer,
    Point,
    Line,
    Circle,
    Dimension,
    Tidy,
    Finish,
    New,
    Open,
    Save,
    Plane,
    Sketch,
    Extrude,
    Perspective,
    Orthographic,
    Fit,
}

/// Every icon, and the source it is drawn from.
///
/// **In the order [`Glyph`] declares them**, which is what lets [`Icons`] index
/// by `glyph as usize` rather than search. The atlas sorts its own table by
/// name, so the handles are resolved back by name into this order — the two
/// orders are unrelated and neither may be assumed of the other.
const SOURCES: [(Glyph, &str, &str); 16] = [
    (Glyph::Pointer, "pointer", POINTER),
    (Glyph::Point, "point", POINT),
    (Glyph::Line, "line", LINE),
    (Glyph::Circle, "circle", CIRCLE),
    (Glyph::Dimension, "dimension", DIMENSION),
    (Glyph::Tidy, "tidy", TIDY),
    (Glyph::Finish, "finish", FINISH),
    (Glyph::New, "new", NEW),
    (Glyph::Open, "open", OPEN),
    (Glyph::Save, "save", SAVE),
    (Glyph::Plane, "plane", PLANE),
    (Glyph::Sketch, "sketch", SKETCH),
    (Glyph::Extrude, "extrude", EXTRUDE),
    (Glyph::Perspective, "perspective", PERSPECTIVE),
    (Glyph::Orthographic, "orthographic", ORTHOGRAPHIC),
    (Glyph::Fit, "fit", FIT),
];

/// The loaded icon set, and one handle per [`Glyph`].
///
/// **An owner.** The [`IconSet`] holds the host's parse of every source and the
/// rasters it has made of them, and dropping the last clone unloads all three.
/// A handle owns nothing, so one outliving its set names a set the host has let
/// go and panics when the renderer draws it — which is why this is parked on
/// [`Look`](super::Look) and handed out by reference.
#[derive(Debug, Clone)]
pub(crate) struct Icons {
    /// Never read, and held for exactly that: the handles beside it are what a
    /// draw names, and this is what keeps them drawable. Dropping the last
    /// clone unloads the parses and the rasters, and a handle that outlives its
    /// set panics when the renderer goes to draw it.
    #[expect(
        dead_code,
        reason = "an RAII owner — dropping it unloads what the handles name"
    )]
    set: IconSet,
    handles: [IconHandle; SOURCES.len()],
}

impl Icons {
    /// Load the set, or hand back a clone of the one already loaded.
    ///
    /// Built through [`IconAtlas::from_svgs`] rather than baked into the
    /// binary. What baking saves is one parse per icon — about three
    /// milliseconds across the set, paid once on the frame the overlay first
    /// draws — and what it costs is a generator and a table nobody may edit by
    /// hand. Sixteen icons do not earn that; a hundred would.
    ///
    /// The atlas itself is built once and parked, because `load_icons`
    /// recognises the same allocation and hands back the set already loaded
    /// against it. A fresh `Rc` each time would load a second set and
    /// re-rasterize the whole of it.
    pub(crate) fn load(ui: &Ui) -> Self {
        thread_local! {
            static BUILT: Rc<IconAtlas> = Rc::new(IconAtlas::from_svgs(
                SOURCES.map(|(_, name, svg)| (name, svg)),
            ));
        }
        let set = BUILT.with(|atlas| ui.load_icons(Rc::clone(atlas)));
        let handles = SOURCES.map(|(_, name, _)| {
            let id = set
                .by_name(name)
                .expect("every source in the table is in the set built from it");
            set.handle(id)
        });
        Self { set, handles }
    }

    /// The artwork for `glyph`.
    pub(crate) fn of(&self, glyph: Glyph) -> IconHandle {
        self.handles[glyph as usize]
    }
}

// One colour throughout every source below, which is what makes each of them
// *tintable*: an icon whose every paint resolves to one colour rasterizes to a
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

const PLANE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2.5 16.5L9 7.5h12.5L15 16.5z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>"##;

const SKETCH: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3 18.5l5-7 4 3.2 3.2-5.7L21 14" fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="3" cy="18.5" r="1.8" fill="#fff"/><circle cx="21" cy="14" r="1.8" fill="#fff"/></svg>"##;

const EXTRUDE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2.5 19.5L8 12.5h13.5L16 19.5z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M12 10.5V3M12 3L9.3 5.9M12 3l2.7 2.9" fill="none" stroke="#fff" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

const PERSPECTIVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M4 4.5l16 3v9l-16 3z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M4 12h16" fill="none" stroke="#fff" stroke-width="1.6"/></svg>"##;

const ORTHOGRAPHIC: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3.5 8.5h12v12h-12z" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/><path d="M3.5 8.5L8 4h12v12l-4.5 4.5M15.5 8.5L20 4" fill="none" stroke="#fff" stroke-width="1.6" stroke-linejoin="round"/></svg>"##;

const FIT: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M3.5 8.5v-5h5M15.5 3.5h5v5M20.5 15.5v5h-5M8.5 20.5h-5v-5" fill="none" stroke="#fff" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12" cy="12" r="2.4" fill="#fff"/></svg>"##;
