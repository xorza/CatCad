//! Writing a document out and reading it back.

use crate::hud::internals::POINT_BUTTON;
use crate::tests::harness::Raised;
use crate::tool::Tool;
use glam::DVec2;
use glam::Vec3;
use palantir::Key;

/// A document written out comes back the way it was left, and everything this
/// run made of the one that was open goes with it.
///
/// The whole loop, through the real application. What each half of it is worth
/// on its own is checked nearer where it lives — the format in
/// `document::file`, the stamp in `filing` — so what this adds is that the
/// pieces are wired to each other and to the keyboard.
///
/// The dialogs are stepped around by naming the path directly, which is what
/// answering one comes to: they put a window on the screen and wait for a
/// person, so a test that reached them would wait for one too. The Ctrl+S in
/// the middle is the exception and is the point of being able to: a document
/// that already has a name must be written *without* asking, and a version of
/// that branch which asked would hang this test rather than fail it.
#[test]
fn a_document_written_out_comes_back_the_way_it_was_left() {
    let mut raised = Raised::new();

    // Somewhere to put it, named for this process so two runs of the suite
    // cannot land on one file.
    let path = std::env::temp_dir().join(format!("catcad-{}.cat", std::process::id()));
    let _ = std::fs::remove_file(&path);

    // Something to notice afterwards: the arm moved somewhere the demo does not
    // start, which is a thing only this document says.
    let held = raised.app.wrist();
    raised.drag(held, held + Vec3::new(0.0, 0.0, 0.6));
    assert!(raised.app.status().to_string().contains(" · unsaved"));

    // As answering a Save As dialog would.
    raised.app.write(path.clone());
    raised.frame();
    assert!(path.exists(), "the document was not written");
    assert!(
        !raised.app.status().to_string().contains(" · unsaved"),
        "a written document still reads as unsaved: {}",
        raised.app.status()
    );

    // Move it again, and this time let the keyboard write it. The document has
    // a name now, so Ctrl+S goes straight to the disk.
    let held = raised.app.wrist();
    raised.drag(held, held + Vec3::new(0.0, 0.0, -1.2));
    let drawn: Vec<DVec2> = raised.points();
    assert!(raised.app.status().to_string().contains(" · unsaved"));
    raised.ctrl(Key::Char('S'));
    raised.frame();
    assert!(
        !raised.app.status().to_string().contains(" · unsaved"),
        "Ctrl+S on a named document did not write it: {}",
        raised.app.status()
    );

    // Now spoil it: a third drag, and a tool in hand.
    let held = raised.app.wrist();
    raised.drag(held, held + Vec3::new(0.0, 0.0, 0.9));
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Point);
    assert_ne!(raised.points(), drawn, "the third drag moved nothing");

    // Opening the file puts the drawing back where Ctrl+S left it, and takes
    // the session with it: nothing in hand, and nothing to take back.
    raised.app.read(path.clone());
    raised.frame();

    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "opening a document left the last one's tool in hand"
    );
    assert_eq!(raised.app.session.selection().count(), 0);
    assert_eq!(
        raised.app.session.editing(),
        None,
        "opening a document left a sketch of the last one open"
    );

    // Back into the drawing to read it, which is what a user would do and the
    // only way there is: a document is opened on no sketch — see
    // [`Document::opening`](crate::document::Document).
    raised.enter_first_sketch();
    assert_eq!(
        raised.points(),
        drawn,
        "the reopened drawing is not the one saved"
    );
    assert!(!raised.app.status().to_string().contains(" · unsaved"));
    assert!(
        raised.app.status().to_string().contains("opened"),
        "the readout said nothing about the file: {}",
        raised.app.status()
    );

    // The undo that would have taken the third drag back finds a history that
    // never saw it — what was done to the document that was open cannot be
    // taken back off the one that replaced it.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.points(),
        drawn,
        "an undo reached past the document it opened"
    );

    let _ = std::fs::remove_file(&path);
}

/// A file that will not open leaves the document that is open exactly as it
/// was, and says why.
///
/// The claim the ordering in [`Document::open`](crate::document::Document) is
/// there to make: nothing is written until the file has been read, parsed,
/// checked and solved. A build reset before that would have taken the *open*
/// document's report with it, and every reader of it would panic.
#[test]
fn a_file_that_will_not_open_disturbs_nothing() {
    let mut raised = Raised::new();
    let drawn = raised.points();

    let path = std::env::temp_dir().join(format!("catcad-bad-{}.cat", std::process::id()));
    std::fs::write(&path, "this is not a document").expect("the scratch file is writable");

    raised.app.read(path.clone());
    // A whole frame afterwards, because the failure that would matter is the
    // one the *next* frame trips over: a build that had forgotten the open
    // document has nothing to draw it from.
    raised.frame();

    assert_eq!(raised.points(), drawn, "a refused file moved the drawing");
    assert!(
        raised
            .app
            .status()
            .to_string()
            .contains("is not a document"),
        "the readout said nothing about the refusal: {}",
        raised.app.status()
    );
    // Still where it was, which is nowhere: a refused open does not name the
    // document after the file it would not read, so the next Ctrl+S asks rather
    // than writing over something that is not a document.
    assert!(raised.app.filing.path().is_none());

    let _ = std::fs::remove_file(&path);
}
