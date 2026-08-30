//! What the renderer keeps between frames, and what a second frame owes it.

use crate::harness::{DEMO_FRAME, DRAWING, Frame, SceneApp, capture, edge_on};
use crate::ink::{INK, Ink, strokes};
use aperture::{Curve, Highlight, Lit, Ring};
use catcad::CatCad;
use glam::UVec2;

/// The renderer's buffers outlive the geometry in them, so a second paint has
/// to overwrite what the first left behind — not append to it, and not leave a
/// removed batch still drawing out of bytes nothing cleared.
///
/// The only test that paints one app twice, and so the only one that reaches
/// the re-upload path at all.
#[test]
fn a_second_paint_replaces_the_geometry_the_first_left() {
    let size = DEMO_FRAME;
    let mut app = CatCad::probe();
    app.enter_first_sketch();
    edge_on(1.4)(app.camera_mut());
    // The constraint marks go before anything is measured. This is about the
    // two batches the column is counted in, and a dimension set as a number is
    // ink in that column that belongs to neither — so it would show up as a
    // stroke that appeared or vanished for reasons nothing here is testing.
    app.pane_mut().scene.texts.clear();

    let first = capture(size, &mut app);
    assert!(
        !strokes(&first, 430).is_empty(),
        "no strokes to begin with, so the rest proves nothing"
    );
    let original: Vec<Curve> = app.pane().scene.curves.to_vec();
    let rings: Vec<Ring> = app.pane().scene.rings.to_vec();

    // Emptied — rings too, since the sketch's circle is one and would still be
    // ink in the column. The buffers stay behind, so anything still drawn here
    // is a ghost read out of bytes the removed batch left in them.
    {
        let mut pane = app.pane_mut();
        let scene = &mut pane.scene;
        scene.curves.clear();
        scene.rings.clear();
    }
    let cleared = capture(size, &mut app);
    assert_eq!(
        strokes(&cleared, 430).len(),
        strokes(&bare(size), 430).len(),
        "strokes outlived the curves they were drawn from"
    );

    // Refilled past what the first batch needed, so the buffer has to grow and
    // the new geometry has to land in the buffer that replaces it.
    {
        let mut pane = app.pane_mut();
        let scene = &mut pane.scene;
        scene.curves.extend(original.iter().cloned());
        scene.curves.extend(original);
        scene.rings.extend(rings);
    }
    let refilled = capture(size, &mut app);
    assert_eq!(
        strokes(&refilled, 430).len(),
        strokes(&first, 430).len(),
        "a grown buffer drew a different set of strokes than the same geometry did"
    );
}

/// The same view with nothing in the curves batch, and nothing ever uploaded
/// from it.
///
/// What the emptiness check above compares against, because emptiness is no
/// longer the right word: the controls and a dimension's lines are their own
/// batch, written against the *camera* on every frame — so the very capture that
/// reads the column redraws them, and no clearing on this side can reach them.
///
/// A second app rather than a second look at the first, and that is the whole of
/// what makes it a reference: this one is emptied before it has ever painted, so
/// there are no bytes behind its curves batch for a ghost to be read out of. Any
/// stroke the cleared frame has over this one came from the buffer that was
/// supposed to have been replaced.
///
/// Its own drawing is skipped for free: [`redraw`] is gated on what it last drew
/// and a fresh app's layout already claims to have drawn, so the batch emptied
/// here stays empty through the frame.
fn bare(size: UVec2) -> Frame {
    let mut app = CatCad::probe();
    app.enter_first_sketch();
    edge_on(1.4)(app.camera_mut());
    // Drawn once before it is emptied, which is what makes this the same frame
    // the caller compares against: a layout that has drawn nothing declines
    // nothing, so a capture straight after a clear would refill every batch and
    // hand back the whole drawing rather than none of it.
    capture(size, &mut app);
    {
        let mut pane = app.pane_mut();
        let scene = &mut pane.scene;
        scene.texts.clear();
        scene.curves.clear();
        scene.rings.clear();
    }
    capture(size, &mut app)
}

/// A highlight is drawn over the primitive it names, and taking it away puts
/// the frame back exactly as it was.
///
/// Driven through `highlight_only` rather than the pointer, because a headless
/// frame has no pointer to hover with — the wiring from one to the other is
/// `CatCad::hover`, and what this pins is the drawing underneath it.
#[test]
fn a_highlighted_edge_is_drawn_over_its_ordinary_self() {
    let size = DEMO_FRAME;
    let mut app = CatCad::probe();
    app.enter_first_sketch();
    // The renderer's own camera rather than the document's, unlike everywhere
    // else: what paints below is a `SceneApp` borrowing this renderer, and the
    // app never records a frame — so nothing ever hands the document's camera
    // over, and aiming it would aim at nothing.
    edge_on(1.1)(&mut app.pane_mut().camera);
    let mut app_pane = SceneApp {
        view: app.renderer().clone(),
    };

    let look = Highlight::new(INK).scale(4.0);
    let magenta = |frame: &Frame| Ink::found_in(frame).count;

    let plain = capture(size, &mut app_pane);
    assert_eq!(magenta(&plain), 0, "nothing is that colour to begin with");

    let edge = app.pane().scene.curves[0]
        .tag
        .expect("the drawing tags its edges");
    app.renderer()
        .borrow_mut()
        .highlight_only(DRAWING, Lit { tag: edge, look });
    let lit = capture(size, &mut app_pane);
    assert!(
        magenta(&lit) > 200,
        "the highlighted edge drew {} px",
        magenta(&lit)
    );

    // And it is *drawn over*, not drawn instead: the rest of the frame is
    // untouched, so clearing restores it pixel for pixel.
    app.renderer().borrow_mut().highlight_all(DRAWING, &[]);
    let cleared = capture(size, &mut app_pane);
    assert_eq!(magenta(&cleared), 0);
    assert_eq!(cleared.image, plain.image, "clearing left something behind");
}
