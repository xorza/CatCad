//! Where the document came from, and whether it has been written since.

use std::path::{Path, PathBuf};

use crate::document::Edits;
use crate::document::file::error::{LoadError, SaveError};

/// Where the document lives, and everything about it that is not the document.
///
/// Beside the document rather than in it, like the
/// [`History`](crate::history::History) and the
/// [`Session`](crate::session::Session): a file's name is not something the
/// drawing says, and a document that carried where it was last written would be
/// a document whose contents changed by being saved somewhere else.
///
/// Answers and does not act. Which file to write is
/// [`dialog`](crate::dialog)'s to ask and [`CatCad::run`](crate::CatCad)'s to
/// answer, because answering means replacing the document, the history and the
/// session at once. What is kept here is only what outlives the errand that set
/// it.
#[derive(Debug, Default)]
pub(crate) struct Filing {
    /// Where the document came from and where saving puts it back, or `None`
    /// for one that has never been anywhere — which is what the app starts
    /// with.
    path: Option<PathBuf>,
    /// What the document's [`Edits`] stood at when it was last written or read.
    ///
    /// `None` until it has been either, which is not the same as zero: a
    /// document that has never been saved is unsaved however little has been
    /// done to it, and a stamp that started where the count does would call one
    /// saved before it had been anywhere.
    saved: Option<Edits>,
    /// What the last errand came to, in the words the readout says it in.
    ///
    /// A sentence rather than the outcome it describes, because that is all
    /// anything does with it — and building it once when the errand lands beats
    /// keeping the pieces to rebuild it sixty times a second.
    report: Option<String>,
}

impl Filing {
    /// Where saving would write, or `None` if it would have to ask.
    ///
    /// Also where a dialog opens, which is the other thing knowing this is good
    /// for: the next document someone wants is usually beside the last one.
    pub(crate) fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Whether the document has been changed since it was last written.
    ///
    /// Conservative, in the direction [`Edits`] is conservative: an undo back to
    /// where the document was saved still reads as unsaved. Erring the other way
    /// would mean a quit that discarded work without asking.
    pub(crate) fn unsaved(&self, edits: Edits) -> bool {
        self.saved != Some(edits)
    }

    /// What the last errand came to, for the readout to say.
    pub(crate) fn report(&self) -> Option<&str> {
        self.report.as_deref()
    }

    /// Note that the document was written to `path`.
    pub(crate) fn wrote(&mut self, path: PathBuf, edits: Edits) {
        self.settled(path, edits, "saved to");
    }

    /// Note that the document was read from `path`.
    pub(crate) fn opened(&mut self, path: PathBuf, edits: Edits) {
        self.settled(path, edits, "opened");
    }

    /// Note that the document now lives at `path` and agrees with what is
    /// there.
    ///
    /// One body for both directions, because saving and opening leave the same
    /// three facts behind: this is where it lives, this is what it said when it
    /// got there, and this is what to tell the reader. Private, and reached
    /// through the two above, so the words a reader sees are chosen here rather
    /// than passed in from wherever the errand landed.
    fn settled(&mut self, path: PathBuf, edits: Edits, did: &str) {
        self.report = Some(format!("{did} {}", path.display()));
        self.path = Some(path);
        self.saved = Some(edits);
    }

    /// Note that saving `path` did not work.
    ///
    /// Where the document lives is left alone, and so is what it last agreed
    /// with: a write that failed changed nothing, so a document that was saved
    /// before this is still saved — and the next Ctrl+S goes where the last one
    /// did rather than where this one was aimed.
    pub(crate) fn refused_write(&mut self, path: &Path, error: SaveError) {
        self.report = Some(format!("{} {error}", path.display()));
    }

    /// Note that opening `path` did not work, for the same reasons.
    pub(crate) fn refused_read(&mut self, path: &Path, error: LoadError) {
        self.report = Some(format!("{} {error}", path.display()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A document is unsaved until it has been written, and again as soon as it
    /// changes.
    ///
    /// The first of those is the one a bare count would get wrong: a document
    /// that has never been anywhere stands at [`Edits::default`], and a marker
    /// comparing against a stamp that also started there would call it saved.
    /// That is why the stamp is an `Option`.
    #[test]
    fn a_document_reads_as_unsaved_until_it_is_written_and_again_once_it_moves() {
        let mut filing = Filing::default();
        let fresh = Edits::default();
        assert!(
            filing.unsaved(fresh),
            "a document that has never been saved"
        );
        assert!(filing.path().is_none());
        assert!(filing.report().is_none());

        filing.wrote(PathBuf::from("/tmp/frame.catcad"), fresh);
        assert!(!filing.unsaved(fresh));
        assert_eq!(filing.path(), Some(Path::new("/tmp/frame.catcad")));
        assert_eq!(filing.report(), Some("saved to /tmp/frame.catcad"));

        // One edit, and it is unsaved again.
        assert!(filing.unsaved(fresh.stepped()));
    }

    /// A refusal moves nothing but what the readout says.
    ///
    /// Nothing reached the disk, so nothing about the document changed — and a
    /// failed Save As that had moved the path would have the *next* Ctrl+S
    /// write somewhere nobody asked for.
    #[test]
    fn a_refusal_moves_nothing_but_what_the_readout_says() {
        let mut filing = Filing::default();
        let saved = Edits::default();
        filing.wrote(PathBuf::from("/tmp/frame.catcad"), saved);

        let error = SaveError::Write(std::io::Error::other("the disk is full"));
        filing.refused_write(Path::new("/tmp/elsewhere.catcad"), error);
        assert_eq!(filing.path(), Some(Path::new("/tmp/frame.catcad")));
        assert!(
            !filing.unsaved(saved),
            "a refused write unsaved the document"
        );
        assert_eq!(
            filing.report(),
            Some("/tmp/elsewhere.catcad could not be written: the disk is full")
        );
    }
}
