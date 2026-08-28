//! Finding a root there is no closed form for.

/// The one place `at` crosses nought between `lo` and `hi`, or `None` where it
/// does not cross there.
///
/// **The two ends must bracket at most one**, which every caller shows before
/// asking: the interval is fenced at the roots of the derivative, so the
/// function only goes one way across it.
///
/// **An end that comes to nought is a graze and not a crossing.** Those ends are
/// turning places, and a function that turns *on* nought touches it rather than
/// passing through — the policy
/// [`quadratic::roots`](crate::math::quadratic::roots) states, applied wherever
/// a root has to be walked down to instead of written down.
///
/// Halved until the middle is one of the two ends, which is the last bit an
/// `f64` holds between them. No tolerance anywhere in it: what comes back is
/// the root to the precision the numbers hold, not a place within some bound of
/// it.
pub(crate) fn crossed(lo: f64, hi: f64, at: impl Fn(f64) -> f64) -> Option<f64> {
    let (mut lo, mut hi) = (lo, hi);
    let (under, over) = (at(lo), at(hi));
    if under == 0.0 || over == 0.0 || under.is_sign_negative() == over.is_sign_negative() {
        return None;
    }
    let negative = under.is_sign_negative();
    loop {
        let middle = 0.5 * (lo + hi);
        if middle <= lo || middle >= hi {
            return Some(middle);
        }
        if at(middle).is_sign_negative() == negative {
            lo = middle;
        } else {
            hi = middle;
        }
    }
}
