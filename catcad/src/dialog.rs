//! The native open and save dialogs — the one place this program hands the
//! question to the desktop rather than answering it itself.
//!
//! Free functions rather than a type, because there is no state here to keep:
//! a dialog is put up, answered or dismissed, and gone. Called
//! namespace-qualified — `dialog::open(..)` — so the call site says where the
//! answer comes from.
//!
//! **Each of these blocks until the dialog is answered**, and the window behind
//! it stops repainting for as long as it is up. That is what a modal file
//! dialog is, and it is the whole reason these are as simple as they are. The
//! alternative is running the dialog off the frame and collecting the answer
//! later, which buys a window that can be resized while a file is being picked
//! and costs a thread, a channel, a way to wake a frame when the answer lands,
//! and a rule about what happens if the document changes underneath it in the
//! meantime.
//!
//! Called from where the errand lands, which is inside a record pass. Palantir
//! only delivers action input on the first pass of a frame, so a command gated
//! on a keypress or a click runs once however often the pass replays — see
//! [`App::record`](palantir::App).

use std::path::{Path, PathBuf};

/// What a document file is called.
///
/// Short, and not a word: a document is a recipe rather than a picture of one,
/// and the extension is the only thing that says so before it is opened.
const EXTENSION: &str = "catcad";

/// What the filter in the dialog's corner is captioned.
const KIND: &str = "CatCad document";

/// Ask which document to open, or `None` where the dialog was dismissed.
///
/// `near` is where the document being worked on lives, which is where the
/// dialog opens — the next document someone wants is usually beside the last
/// one. `None` leaves that to the desktop, which remembers where it was.
pub(crate) fn open(near: Option<&Path>) -> Option<PathBuf> {
    at(near, "Open a document").pick_file()
}

/// Ask where to put the document, or `None` where the dialog was dismissed.
///
/// Seeded with the document's own name as well as its directory, so saving one
/// that already has a name somewhere else is that name with a word changed.
pub(crate) fn save(near: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = at(near, "Save the document");
    if let Some(name) = near.and_then(Path::file_name) {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    // Given an extension on the way out rather than demanded on the way in.
    // Whether the dialog appends one for the filter is the desktop's business
    // and differs between them, so a name typed bare comes back bare on some
    // and not others; this is what makes the answer the same either way.
    dialog.save_file().as_deref().map(named)
}

/// A dialog over documents, opened wherever `near` lives.
///
/// The one place either of the two is built, so both wear the same filter and
/// neither can be given one the other has not.
fn at(near: Option<&Path>, title: &str) -> rfd::FileDialog {
    let mut dialog = rfd::FileDialog::new()
        .set_title(title)
        .add_filter(KIND, &[EXTENSION]);
    if let Some(directory) = near.and_then(Path::parent) {
        dialog = dialog.set_directory(directory);
    }
    dialog
}

/// `path` with the document extension, if it had none of its own.
///
/// Only where there is no extension at all. A name typed bare is a name and
/// wants one; `drawing.old.catcad` already has one, and so does `notes.txt` — a
/// path someone meant, and not this crate's to correct.
fn named(path: &Path) -> PathBuf {
    match path.extension() {
        Some(_) => path.to_path_buf(),
        None => path.with_extension(EXTENSION),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path chosen without an extension gets the document's, and one that
    /// already has any is left alone.
    ///
    /// Any, not this one: `notes.txt` is a path someone meant, and a rule that
    /// counted dots would turn `frame.old.catcad` into `frame.old.catcad.catcad`.
    ///
    /// The one thing here a test can reach. What the dialogs above do is put a
    /// window on the screen and wait for a person, so the rest of this module
    /// is answered by using it rather than by a harness.
    #[test]
    fn a_chosen_path_without_an_extension_is_given_one() {
        for (chosen, want) in [
            ("frame", "frame.catcad"),
            ("frame.catcad", "frame.catcad"),
            ("frame.old.catcad", "frame.old.catcad"),
            ("notes.txt", "notes.txt"),
            // A directory with a dot in it does not count as the file having
            // one, which is the case a naive search for `.` would get wrong.
            ("/tmp/a.b/frame", "/tmp/a.b/frame.catcad"),
        ] {
            assert_eq!(
                named(Path::new(chosen)),
                PathBuf::from(want),
                "chose {chosen:?}"
            );
        }
    }
}
