//! A dimension opened for typing, and the line being typed into it.

use aperture::{Seek, Selecting, TextEdit};
use palantir::{Key, KeyboardEvent};

use crate::paint::DECIMALS;
use crate::part::Part;

/// A dimension being retyped: which one, what has been typed so far, and where
/// the caret is in it.
///
/// **Session state, and scratch.** Nothing here is written down by saving, none
/// of it is a step to take back, and a draft abandoned is a draft that never
/// happened — the dimension is not touched until [`Done::Commit`]. That puts it
/// on the same footing as the rubber band a half-drawn line follows, which is
/// likewise kept rather than raised as an intent.
///
/// **Written during the asking half of a frame**, unlike everything else the
/// session holds, and the reason is the rule intents are built on: every one
/// *names where it wants to end up, never how far to go*, so that a replayed
/// pass lands on the same answer. A keystroke cannot be written that way — "put
/// a 5 in" is a step and not a destination, and two passes would put two in. So
/// a stroke is applied where it is read, which is safe for the reason
/// palantir's own text field is: the keyboard queue is drained between the two
/// passes of a settling frame, so the second reads no keys at all. Opening and
/// closing *are* destinations, and those go through
/// [`Choice::Type`](crate::intent::Choice) like everything else.
#[derive(Debug)]
pub(crate) struct Typing {
    /// The dimension being retyped.
    ///
    /// What the drawing skips when it lays its marks out — the field is drawn
    /// where the mark would be, and both at once would be one on top of the
    /// other — and what a commit names.
    part: Part,
    /// What has been typed, and where the caret is in it.
    ///
    /// A whole [`TextEdit`] rather than a string and two offsets, so that what
    /// a keystroke does to a line is written once, in the crate that draws one.
    /// Where it sits in the world is not read: this is the draft, and the copy
    /// in the scene is placed on the drawing every time it is written — see
    /// [`paint::retype`](crate::paint::retype).
    field: TextEdit,
    /// How many edits the draft has had.
    ///
    /// What lets the picture be brought up to date without comparing what the
    /// field says against what was last drawn — which would mean keeping a copy
    /// of the string to compare against. A counter is the cheap half of that,
    /// and the field is the only thing that writes it.
    ///
    /// Wraps at `u64`, which is a number of keystrokes no session reaches.
    revision: u64,
}

/// What a keystroke asked of the session, beyond editing the draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Done {
    /// Put what the field says on the dimension.
    Commit,
    /// Leave the dimension as it was.
    Cancel,
}

impl Typing {
    /// Open `part` at `value`, with the whole of it picked out.
    ///
    /// Everything selected, because a dimension is retyped far more often than
    /// it is amended: the first digit typed should replace the number rather
    /// than land in the middle of it. Amending is still there — an arrow key
    /// drops the selection without taking anything out.
    pub(crate) fn on(part: Part, value: f64) -> Self {
        // From [`Default`] rather than [`TextEdit::new`], which wants a
        // position and a type size: this draft is drawn through a copy that
        // takes both off the mark it replaces, so anything said here would be
        // a number nothing ever reads — see
        // [`paint::retype`](crate::paint::retype).
        let mut field = TextEdit::default();
        field.set_content(&format!("{value:.*}", DECIMALS));
        field.select_all();
        Self {
            part,
            field,
            revision: 0,
        }
    }

    /// The dimension being retyped.
    pub(crate) fn part(&self) -> Part {
        self.part
    }

    /// The draft, for whoever is drawing it.
    pub(crate) fn field(&self) -> &TextEdit {
        &self.field
    }

    /// How many edits it has had. See [`Typing::revision`].
    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    /// What the field says as a number, or `None` where it says something that
    /// is not one.
    ///
    /// A `Result` would be the wrong shape: text that is not a number is not a
    /// failure to report but a draft that is not finished, and what a commit
    /// does with one is refuse to commit. Untrusted input in the sense the
    /// error-handling rules mean it — a person typing — but the only thing that
    /// can be said about it is whether it parses.
    ///
    /// Where a formula goes. Today the field holds a literal and this is
    /// [`str::parse`]; when it holds an expression this is where it is
    /// evaluated, and every caller already treats `None` as "not yet".
    pub(crate) fn value(&self) -> Option<f64> {
        self.field.content().trim().parse().ok()
    }

    /// Take one keyboard event, editing the draft, and say what it asked of the
    /// session beyond that.
    ///
    /// `None` is both an event that edited the draft and one this had no use
    /// for — which are the same answer to the caller, who is asking only
    /// whether the dimension is finished with.
    pub(crate) fn take(&mut self, event: &KeyboardEvent) -> Option<Done> {
        let press = match event {
            // What an input method hands over once it has finished composing.
            // *Not* how ordinary typing arrives — a window sends that as a key
            // going down, below — so both paths have to insert, and a caller
            // must not raise the two for one character.
            KeyboardEvent::Text(chunk) => {
                self.edited(|field| field.insert(chunk.as_str()));
                return None;
            }
            KeyboardEvent::Down(press) => *press,
        };
        let selecting = if press.mods.shift {
            Selecting::Extend
        } else {
            Selecting::Drop
        };
        match press.key {
            Key::Enter => return Some(Done::Commit),
            Key::Escape => return Some(Done::Cancel),
            Key::Backspace => self.edited(TextEdit::delete_back),
            Key::Delete => self.edited(TextEdit::delete_forward),
            Key::ArrowLeft => self.edited(|field| field.seek(Seek::Left, selecting)),
            Key::ArrowRight => self.edited(|field| field.seek(Seek::Right, selecting)),
            Key::Home => self.edited(|field| field.seek(Seek::Start, selecting)),
            Key::End => self.edited(|field| field.seek(Seek::End, selecting)),
            // Before the arm below, which would otherwise type an `a`.
            Key::Char('a' | 'A') if press.mods.ctrl => self.edited(TextEdit::select_all),
            // How typing really arrives. The logical key is post-shift, so an
            // uppercase letter is already `Char('A')` and nothing here has to
            // fold a case.
            //
            // A command chord is not this field's, which is what the guard
            // says: cut, copy and paste are the ones that will be, and until
            // they are wired, taking them would put a `v` in the field on
            // paste and swallow the application's own chords besides.
            Key::Char(ch) if !press.mods.any_command() => {
                let mut buffer = [0; 4];
                let typed = ch.encode_utf8(&mut buffer);
                self.edited(|field| field.insert(typed));
            }
            _ => {}
        }
        None
    }

    /// Do something to the draft and count it.
    ///
    /// Every write goes through here, which is the whole of what keeps
    /// [`Typing::revision`] honest: a field edited around this would be one the
    /// picture never noticed had changed.
    fn edited(&mut self, edit: impl FnOnce(&mut TextEdit)) {
        edit(&mut self.field);
        self.revision = self.revision.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests;
