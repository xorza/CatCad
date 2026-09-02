//! Turning what somebody typed into one number.

use crate::notation::unit::Unit;

/// A walk through typed text, working it out as it goes.
///
/// **One pass and no tree.** Every reading here is answered the moment it is
/// asked — a field is read on the keystroke that changed it, and a syntax tree
/// would be a heap allocation per keystroke to hold something nothing looks at
/// twice. What is stored of an expression is the *text*; this is what that text
/// comes to.
///
/// **Worked out in the document's own unit, and turned into millimetres once at
/// the end.** Doing it the other way round makes a division wrong: `1/2` would
/// be a millimetre over two millimetres, which is a half of nothing. So a bare
/// number counts as one, a number with a suffix counts as that suffix over the
/// document's own, and the whole answer is scaled once — see
/// [`Notation`](super::Notation), where the store's own unit is argued.
///
/// **Bytes rather than characters.** Every word the grammar knows is ASCII, so
/// anything else is text that is not a number — and a reader that walked
/// characters would pay for the decoding on every keystroke to reach the same
/// refusal.
#[derive(Debug, Clone, Copy)]
pub(super) struct Reading<'a> {
    text: &'a [u8],
    at: usize,
    /// The unit a bare number is said in, or `None` where the number is not a
    /// length at all.
    ///
    /// **`None` turns the suffixes off rather than scaling by one.** An angle
    /// stated `90mm` is not an angle, so the letters are left where they stand
    /// and the whole reading refuses on them — where a scale of one would read
    /// ninety degrees out of a text that says no such thing.
    said: Option<Unit>,
}

impl<'a> Reading<'a> {
    /// A walk through `text`, reading a bare number as `said` — see
    /// [`Reading::said`].
    pub(super) fn of(text: &'a str, said: Option<Unit>) -> Self {
        Self {
            text: text.as_bytes(),
            at: 0,
            said,
        }
    }

    /// How many millimetres one of the document's own units is, and one for a
    /// number that is not a length.
    fn across(&self) -> f64 {
        self.said.map_or(1.0, Unit::across)
    }

    /// The whole of the text as one length in millimetres, or `None` where it
    /// does not say one.
    ///
    /// **The whole of it**, so trailing text is a refusal rather than something
    /// read and dropped: `5 apples` is not five, and a field that answered five
    /// would be answering a question nobody asked.
    ///
    /// **And finite**, which is where a division by nothing lands. The
    /// arithmetic below never looks for one — `1/0` is an infinity in `f64` and
    /// `0/0` is a `NaN`, and both fail here, so one reading catches both
    /// without a case of its own.
    pub(super) fn whole(mut self) -> Option<f64> {
        let value = self.sum()?;
        self.spaces();
        let read = value * self.across();
        (self.at == self.text.len() && read.is_finite()).then_some(read)
    }

    /// A run of products added and subtracted.
    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        loop {
            self.spaces();
            let step = match self.peek() {
                Some(b'+') => 1.0,
                Some(b'-') => -1.0,
                _ => return Some(value),
            };
            self.at += 1;
            value += step * self.product()?;
        }
    }

    /// A run of pieces multiplied and divided, and the unit it is said in.
    ///
    /// **The suffix closes the product rather than binding to the number in
    /// front of it**, which is the one thing that makes `1/2in` a half inch
    /// rather than one over a half inch. What it costs is a suffix in the
    /// *middle* of a product: `3*2mm` is six millimetres and `2mm*3` is
    /// refused. That is the rarer of the two by a long way, and a refusal is
    /// what a reading owes where it cannot be sure.
    fn product(&mut self) -> Option<f64> {
        let mut value = self.signed()?;
        loop {
            self.spaces();
            let over = match self.peek() {
                Some(b'*') => false,
                Some(b'/') => true,
                _ => break,
            };
            self.at += 1;
            let next = self.signed()?;
            value = if over { value / next } else { value * next };
        }
        Some(match self.unit() {
            Some(unit) => value * unit.across() / self.across(),
            None => value,
        })
    }

    /// One piece, with however many signs somebody put in front of it.
    fn signed(&mut self) -> Option<f64> {
        self.spaces();
        match self.peek() {
            Some(b'-') => {
                self.at += 1;
                Some(-self.signed()?)
            }
            Some(b'+') => {
                self.at += 1;
                self.signed()
            }
            _ => self.piece(),
        }
    }

    /// One number, or one bracketed sum.
    fn piece(&mut self) -> Option<f64> {
        self.spaces();
        if self.peek() != Some(b'(') {
            return self.number();
        }
        self.at += 1;
        let inner = self.sum()?;
        self.spaces();
        if self.peek() != Some(b')') {
            return None;
        }
        self.at += 1;
        Some(inner)
    }

    /// Digits, with a decimal point wherever somebody put one.
    ///
    /// **A point on its own is not a number**, and neither is a point with
    /// nothing either side of it: `.5` and `5.` are both five tenths and five,
    /// and `.` alone is a draft nobody has finished.
    fn number(&mut self) -> Option<f64> {
        let began = self.at;
        let mut digits = 0;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_digit() {
                digits += 1;
            } else if byte != b'.' || self.text[began..self.at].contains(&b'.') {
                break;
            }
            self.at += 1;
        }
        if digits == 0 {
            return None;
        }
        // Read back through `str`, so the rounding is the one every other
        // reading of a literal in this program gets rather than a second
        // arithmetic of our own.
        std::str::from_utf8(&self.text[began..self.at])
            .ok()?
            .parse()
            .ok()
    }

    /// The unit named here, where one is, taking it off the text.
    ///
    /// **Nothing is taken where nothing is a unit.** A run of letters that
    /// names no length is left where it stands, so the whole reading refuses on
    /// it rather than reading a number out of the front of a word.
    fn unit(&mut self) -> Option<Unit> {
        self.said?;
        self.spaces();
        let began = self.at;
        if matches!(self.peek(), Some(b'"' | b'\'')) {
            self.at += 1;
        } else {
            while self.peek().is_some_and(|byte| byte.is_ascii_alphabetic()) {
                self.at += 1;
            }
        }
        let named = std::str::from_utf8(&self.text[began..self.at])
            .ok()
            .and_then(Unit::named);
        if named.is_none() {
            self.at = began;
        }
        named
    }

    /// Step over any spaces.
    fn spaces(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    /// The byte the walk stands on, or `None` at the end of the text.
    fn peek(&self) -> Option<u8> {
        self.text.get(self.at).copied()
    }
}
