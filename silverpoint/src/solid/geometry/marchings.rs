//! The runs of places a marched curve is made of.
//!
//! **Where a curve of the fitted tier keeps what it is** — see
//! `.notes/KERNEL.md` §7.3, where the shape of this is argued. A `Curve` is a
//! `Copy` value and a run of places is not, so the run lives here and the curve
//! names it, exactly as a face names the stretch of loops that is its.
//!
//! What fills one is the boolean, meeting a pair it has to march; what reads
//! one is every walk over an edge.

use crate::loops::Loops;
use glam::DVec3;
use std::f64::consts::TAU;

/// One place of a marched run, and how far round the run it stands.
///
/// **The length is carried rather than added up.** What reads a run is a walk
/// asking for a place at every step of it, so working the length out from the
/// beginning each time would be a walk inside a walk — the square of the sample
/// count, per edge, per frame. Carried, the same question is a search through a
/// run that is already in order, and it costs eight bytes against a place's
/// twenty-four.
#[derive(Debug, Clone, Copy, Default)]
struct Sample {
    at: DVec3,
    round: f64,
}

/// What a whole marched run comes to.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Strayed {
    /// How far the furthest chord of it stands from the curve it was walked on.
    ///
    /// **The bound §4.1 says a fitted result carries**, and what an edge on
    /// this run is held to. Fixed where the run was walked: nothing here can
    /// walk it again, having neither the surfaces nor the room.
    pub(crate) most: f64,
    /// How far round the whole run is, which is what its parameter is measured
    /// against.
    pub(crate) round: f64,
    /// How large the numbers reading it work in — see
    /// [`Curve::reach`](super::curve::Curve::reach).
    pub(crate) reach: f64,
    /// Whether the run comes back to where it began.
    ///
    /// **Read off the walk rather than declared**, so no caller can file a run
    /// as closing that does not: a march round a meeting lays its first place
    /// down again at the end, bit for bit, and a curve walked from one corner
    /// to another leaves its two ends where they are.
    pub(crate) shut: bool,
}

/// A curve laid down as a run of places rather than written down.
///
/// **The fitted tier's own curve** — see `.notes/KERNEL.md` §4.1 for the tier
/// and §4.5 for why this is a handle. What holds the places is [`Marchings`],
/// which a body keeps beside its topology; what is here is the handle and the
/// two numbers a reader answers without reaching for them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Marched {
    /// Which run of the body's own store it is.
    pub(crate) run: u32,
    /// What it is filed under — see
    /// [`Curve::key`](super::curve::Curve::key).
    ///
    /// **Over what made it rather than over what it came to**, which is the
    /// whole reason it is carried here: a key read off the places would make a
    /// curve's identity depend on how finely it happened to be walked, and a
    /// crossing met from either side has to key alike.
    pub(crate) key: u64,
    /// How large the numbers reading it work in — see
    /// [`Curve::reach`](super::curve::Curve::reach).
    pub(crate) reach: f64,
    /// Whether it comes back to where it began — see [`Strayed::shut`], which
    /// is where it is derived.
    pub(crate) shut: bool,
}

/// Every marched curve a body stands on, laid end to end.
///
/// **Flat, so that nothing an arena holds owns a heap block.** A body is
/// rebuilt whole on every frame of a drag through the drawing under it, and
/// emptying this is one `clear` that keeps every buffer — see
/// [`Loops`], which is the same arrangement a face's own
/// loops are kept in.
#[derive(Debug, Default)]
pub(crate) struct Marchings {
    runs: Loops<Sample, Strayed>,
    /// One run on its way in — see [`Marchings::add`], which measures a walk
    /// and files it in one pass and wants somewhere to lay it down meanwhile.
    filing: Vec<Sample>,
}

impl Marchings {
    /// Forget every run, keeping the room they took.
    pub(crate) fn clear(&mut self) {
        self.runs.clear();
    }

    /// Take a copy of what `of` holds, over the room these took.
    ///
    /// **A copy and not a trade**, which is what a body written *beside*
    /// another asks for: both go on being read, so both want the runs their
    /// edges name — see [`Carried::take_from`](super::carried::Carried). Refilled
    /// in place, a merge running on the frame a document is rebuilt in.
    pub(crate) fn take_from(&mut self, of: &Self) {
        self.runs.clear();
        for at in 0..of.runs.len() {
            self.runs.push_by(*of.runs.by(at), of.runs.get(at));
        }
    }

    /// How many runs are filed, which is the number the next one takes.
    pub(crate) fn len(&self) -> u32 {
        self.runs.len() as u32
    }

    /// File `walked` as a run of its own, no chord of it straying further than
    /// `most`, and say which run it is.
    ///
    /// **Whether it closes is read here and nowhere else.** A march round a
    /// meeting — [`Marching::walk`](crate::solid::meeting::marching::Marching)
    /// — lays its first place down again at the end, so the last chord shuts
    /// the loop; an edge walked from one corner to another leaves its two ends
    /// where they are. The parameter is a whole turn either way, and what
    /// parts them is what happens at the end of it — see [`Marchings::at`].
    pub(crate) fn add(&mut self, walked: &[DVec3], most: f64) -> u32 {
        // **One pass**, which is what makes filing a run cost its own length
        // rather than three times it: how far round the whole is and how large
        // its numbers work both fall out of the walk that lays the samples
        // down, and a chord measured twice is a chord measured once too often.
        self.filing.clear();
        self.filing.reserve_exact(walked.len());
        let (mut round, mut reach) = (0.0, 0.0_f64);
        let mut last: Option<DVec3> = None;
        for &at in walked {
            round += last.map_or(0.0, |last: DVec3| last.distance(at));
            reach = reach.max(at.length());
            self.filing.push(Sample { at, round });
            last = Some(at);
        }
        // Bit for bit, a walk that closed having laid its own first place down
        // again rather than arrived near it.
        let shut = walked.len() > 1 && walked.first() == walked.last();
        let run = self.len();
        self.runs.push_by(
            Strayed {
                most,
                round,
                reach,
                shut,
            },
            &self.filing,
        );
        run
    }

    /// What the run at `run` comes to as a whole.
    pub(crate) fn strayed(&self, run: u32) -> Strayed {
        *self.runs.by(run as usize)
    }

    /// How far the furthest chord of any run filed here strays from the curve
    /// it was walked on, and nought where none is filed.
    ///
    /// **The bound §4.1 says a fitted result carries**, gathered over the whole
    /// store: a body's own is the worst of the curves it stands on, there being
    /// no reading of it finer than its coarsest edge.
    pub(crate) fn strays(&self) -> f64 {
        (0..self.len())
            .map(|run| self.strayed(run).most)
            .fold(0.0, f64::max)
    }

    /// Where the parameter `t` lands on the run at `run`.
    ///
    /// **A whole turn to a lap**, the length round scaled to `TAU` — so a
    /// closed marched curve is split at its own nought and half turn by the
    /// same reading that splits a circle, and the sewing needs no arm of its
    /// own for one.
    ///
    /// **A run that does not come back has an end**, so a parameter past that
    /// end is held there rather than carried round to the start. That is the
    /// one place the two kinds part company, the whole turn being what both are
    /// read against.
    pub(crate) fn at(&self, run: u32, t: f64) -> DVec3 {
        let (samples, strayed) = (self.runs.get(run as usize), self.strayed(run));
        let share = t / TAU;
        let want = match strayed.shut {
            true => share.rem_euclid(1.0),
            false => share.clamp(0.0, 1.0),
        } * strayed.round;
        // The chord holding it, which is the one that begins at the last sample
        // standing no further round than `want`.
        let step = samples
            .partition_point(|sample| sample.round <= want)
            .max(1)
            - 1;
        let Some(next) = samples.get(step + 1) else {
            return samples[step].at;
        };
        let (here, chord) = (samples[step], next.round - samples[step].round);
        if chord == 0.0 {
            return here.at;
        }
        here.at.lerp(next.at, (want - here.round) / chord)
    }

    /// Which parameter puts the run at `run` at `at`.
    ///
    /// The place has to be on the run, which every caller has — see
    /// [`Curve::along`](super::curve::Curve::along), where that rule is stated.
    ///
    /// **Walked rather than searched**, and that is what caps how finely a run
    /// may be laid down: a place says nothing about where round it stands, so
    /// the chord nearest it is every chord. Lifting the cap wants a curve that
    /// can be solved, which is what the exact tier has and this one does not —
    /// see [`Quartics::along`](super::quartic::Quartics).
    pub(crate) fn along(&self, run: u32, at: DVec3) -> f64 {
        let (samples, strayed) = (self.runs.get(run as usize), self.strayed(run));
        let (mut along, mut off) = (0.0, f64::INFINITY);
        for pair in samples.windows(2) {
            let (from, way) = (pair[0], pair[1].at - pair[0].at);
            let chord = way.length_squared();
            let share = if chord == 0.0 {
                0.0
            } else {
                ((at - from.at).dot(way) / chord).clamp(0.0, 1.0)
            };
            let apart = at.distance(from.at + way * share);
            if apart < off {
                (along, off) = (from.round + share * chord.sqrt(), apart);
            }
        }
        match strayed.round == 0.0 {
            true => 0.0,
            false => along * TAU / strayed.round,
        }
    }

    /// Every place of the run at `run`, with the parameter it stands at.
    ///
    /// **In the order it was walked**, which is what lets a reader lay corners
    /// down along it without asking where each of them stands: a parameter read
    /// off a place is a walk of the whole run — see [`Marchings::along`] — and
    /// read out here it is a division.
    ///
    /// The place it began at stands at the end as well, at a whole turn.
    pub(crate) fn sampled(&self, run: u32) -> impl Iterator<Item = (f64, DVec3)> {
        let (samples, strayed) = (self.runs.get(run as usize), self.strayed(run));
        samples.iter().map(move |sample| {
            let along = if strayed.round == 0.0 {
                0.0
            } else {
                TAU * sample.round / strayed.round
            };
            (along, sample.at)
        })
    }

    /// How many chords a stretch of `span` parameter of the run at `run` is
    /// worth.
    ///
    /// **What it has rather than what was asked for.** A run cannot be laid
    /// down again — see [`Strayed::most`] — so the answer is its own chords
    /// over that stretch, and how good they are is what the edge carries.
    pub(crate) fn steps(&self, run: u32, span: f64) -> usize {
        let chords = self.runs.get(run as usize).len().saturating_sub(1);
        ((span.abs() / TAU) * chords as f64).ceil().max(1.0) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ring of `radius` about the origin in the plane `z = 0`, walked as
    /// `count` chords and shut.
    fn ring(radius: f64, count: usize) -> Vec<DVec3> {
        (0..=count)
            .map(|step| {
                // Shut on its own first place, as a march hands one back.
                let (up, out) = (TAU * (step % count) as f64 / count as f64).sin_cos();
                DVec3::new(radius * out, radius * up, 0.0)
            })
            .collect()
    }

    /// **A run reads back where it was laid down**, and its parameter is a
    /// whole turn however long it is.
    ///
    /// A circle of two, walked as three hundred and twenty equal chords. The
    /// chords are all one length, so a fraction of the way round the run is
    /// that fraction of the way round the circle — and a probe every twentieth
    /// chord lands on a sample, where the run and the circle are the same
    /// place. Read back, each gives the angle it came from.
    ///
    /// **And a probe half a chord along is the chord's own middle**, which is
    /// the whole of what the reading between two samples has to be.
    #[test]
    fn a_run_reads_back_the_angle_it_was_laid_down_at() {
        let mut marchings = Marchings::default();
        let walked = ring(2.0, 320);
        let run = marchings.add(&walked, 1e-4);
        let strayed = marchings.strayed(run);
        assert_eq!(strayed.most, 1e-4);
        assert!((strayed.reach - 2.0).abs() < 1e-12, "{strayed:?}");
        // A chorded circle falls short of `2πr` by `(π/n)²/6` of it, which
        // for three hundred and twenty chords is two parts in ten thousand —
        // and never stands over it.
        assert!(strayed.round < TAU * 2.0, "{strayed:?} came out over");
        assert!(
            strayed.round > TAU * 2.0 - 1e-3,
            "{strayed:?} came out short"
        );

        for step in 0..16 {
            let t = TAU * step as f64 / 16.0;
            let want = walked[step * 20];
            let at = marchings.at(run, t);
            assert!((at - want).length() < 1e-12, "{at:?} rather than {want:?}");
            let back = marchings.along(run, at);
            let apart = (back - t).rem_euclid(TAU).min((t - back).rem_euclid(TAU));
            assert!(apart < 1e-9, "{back} rather than {t}");
        }

        let half = marchings.at(run, TAU * 0.5 / 320.0);
        let middle = (walked[0] + walked[1]) / 2.0;
        assert!(
            (half - middle).length() < 1e-12,
            "{half:?} is not {middle:?}"
        );
    }

    /// **A run answers with the chords it has**, whatever is asked of it.
    ///
    /// Sixteen chords round a whole turn is one for each sixteenth of it, and
    /// asking for a quarter turn asks for four of them. A stretch shorter than
    /// one chord is still one: a chord is the coarsest a curve can be, and
    /// none would be answering that it is not there.
    #[test]
    fn a_run_answers_with_the_chords_it_has() {
        let mut marchings = Marchings::default();
        let run = marchings.add(&ring(1.0, 16), 1e-3);
        assert_eq!(marchings.steps(run, TAU), 16);
        assert_eq!(marchings.steps(run, TAU / 4.0), 4);
        assert_eq!(marchings.steps(run, TAU / 64.0), 1);
        assert_eq!(marchings.steps(run, 0.0), 1);
    }

    /// **A run that does not come back has an end**, and the parameter still
    /// runs a whole turn over it.
    ///
    /// Five places of the eight-chord ring, filed on their own. Read at nought
    /// the run is where it began and at a whole turn it is where it stopped;
    /// past either it is held there, where the ring beside it carries on round.
    /// That is the one place the two kinds part company.
    #[test]
    fn an_open_run_is_held_at_its_ends_where_a_closed_one_carries_on() {
        let mut marchings = Marchings::default();
        let whole = ring(1.0, 8);
        let arc = whole[..5].to_vec();
        let shut = marchings.add(&whole, 1e-3);
        let open = marchings.add(&arc, 1e-3);
        assert!(marchings.strayed(shut).shut, "a ring was filed as open");
        assert!(!marchings.strayed(open).shut, "an arc was filed as closing");

        let [first, last] = [arc[0], arc[4]];
        for (t, want) in [(0.0, first), (TAU, last), (-TAU, first), (2.0 * TAU, last)] {
            let at = marchings.at(open, t);
            assert!(at.abs_diff_eq(want, 1e-12), "{at:?} at {t} is not {want:?}");
        }
        // The same readings on the ring carry round instead: a whole turn is
        // its own start again, and an eighth past two turns is its second
        // place.
        assert!(marchings.at(shut, TAU).abs_diff_eq(whole[0], 1e-12));
        let round = marchings.at(shut, 2.0 * TAU + TAU / 8.0);
        assert!(round.abs_diff_eq(whole[1], 1e-12), "{round:?}");
    }

    /// **Runs are filed one after another and keep their own room**, which is
    /// what a body rebuilt on every frame needs of them.
    #[test]
    fn runs_are_filed_apart_and_emptying_keeps_the_room() {
        let mut marchings = Marchings::default();
        let near = marchings.add(&ring(1.0, 8), 1e-3);
        let far = marchings.add(&ring(3.0, 8), 1e-2);
        assert_eq!((near, far), (0, 1));
        assert!((marchings.strayed(near).reach - 1.0).abs() < 1e-12);
        assert!((marchings.strayed(far).reach - 3.0).abs() < 1e-12);
        assert!((marchings.at(near, 0.0) - DVec3::X).length() < 1e-12);
        assert!((marchings.at(far, 0.0) - DVec3::X * 3.0).length() < 1e-12);

        marchings.clear();
        assert_eq!(
            marchings.add(&ring(1.0, 8), 1e-3),
            0,
            "the numbering ran on"
        );
    }
}
