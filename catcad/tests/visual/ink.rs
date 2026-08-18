//! Reading a frame: what a stroke deposited, what is lit, and where the ink of
//! a run landed.

use crate::harness::Frame;
use glam::{UVec2, Vec2, Vec3};

/// A sketch stroke crossing `column`, as the width it actually deposited.
///
/// The slab is neutral and the drawing never is, so saturation — how far a
/// pixel's strongest channel stands above its weakest — isolates the strokes
/// from the shading. Saturation rather than any one pair of channels because
/// the drawing is coloured by how constrained it is, cool where it is settled
/// and warm where it is free, and a measure built around one hue would see
/// only half of it.
///
/// Total ink over peak intensity is the covered width in pixels however
/// multisampling spreads the edges: a stroke of width `n` drawn at peak `p`
/// deposits `n * p` whether that lands on two pixels or four. That only holds
/// where ink is proportional to coverage, which is why the channels are
/// linearised first — see [`linear`].
///
/// And where `p` is what a *covered* pixel is worth, which is the floor under
/// what this can measure. A stroke narrower than a pixel never fully covers
/// one, so the brightest it reaches is itself a partial reading and dividing by
/// it reports more width than is there: authored at 1.0 the demo's curves
/// measure 1.538 and its rings 1.291, both of which are the estimator and not
/// the geometry. From about 1.5 up some pixel is covered whatever the stroke's
/// phase against the grid, and the number means what it says. Below that, widen
/// what you are measuring rather than trusting it.
#[derive(Debug, PartialEq)]
pub(crate) struct Stroke {
    pub(crate) row: u32,
    pub(crate) width: f32,
}

pub(crate) fn strokes(frame: &Frame, column: u32) -> Vec<Stroke> {
    let signal: Vec<f32> = (0..frame.size.y)
        .map(|y| {
            let [r, g, b, _] = frame.pixel(UVec2::new(column, y));
            let (r, g, b) = (linear(r), linear(g), linear(b));
            r.max(g).max(b) - r.min(g).min(b)
        })
        .collect();

    // The slab's own blue tint drifts with the shading, so each row is judged
    // against its neighbourhood rather than against zero. That also keeps the
    // coloured cubes out of it: a broad flat region is its own baseline, and
    // only something thin against its surroundings lifts clear.
    let reach = 12usize;
    let baseline = |y: usize| {
        let lo = y.saturating_sub(reach);
        let hi = (y + reach + 1).min(signal.len());
        let mut window: Vec<f32> = signal[lo..hi].to_vec();
        window.sort_by(f32::total_cmp);
        window[window.len() / 2]
    };
    let lifted: Vec<f32> = (0..signal.len()).map(|y| signal[y] - baseline(y)).collect();

    let mut out = Vec::new();
    let mut y = 0usize;
    while y < lifted.len() {
        if lifted[y] <= EDGE_OF_INK {
            y += 1;
            continue;
        }
        let start = y;
        while y < lifted.len() && lifted[y] > EDGE_OF_INK {
            y += 1;
        }
        let peak = lifted[start..y].iter().copied().fold(0.0, f32::max);
        // Reach past the run for the multisampled shoulders it faded into.
        let lo = start.saturating_sub(3);
        let hi = (y + 3).min(lifted.len());
        let ink: f32 = lifted[lo..hi].iter().map(|v| v.max(0.0)).sum();
        out.push(Stroke {
            row: start as u32,
            width: ink / peak,
        });
    }
    out
}

/// A pixel this much more saturated than its neighbourhood is stroke rather
/// than slab. In linear light, and well under the least saturated thing the
/// drawing paints.
const EDGE_OF_INK: f32 = 0.01;

/// One sRGB byte as the linear intensity it stands for.
///
/// The frame comes back sRGB-encoded, and coverage is blended before that
/// encoding: a pixel a stroke half covers carries half the light, not half the
/// byte. Measuring ink in bytes therefore weighs a stroke by its colour as much
/// as by its width, which is exactly what a width measurement must not do.
fn linear(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// How wide anything lit is in `row`, in pixels.
///
/// The drawing's whole silhouette across that row, whatever is standing in it:
/// what the one caller wants is a width in the world measured at two depths, and
/// which piece of the demo happens to be widest there is no part of the claim.
pub(crate) fn lit_span(frame: &Frame, row: u32) -> u32 {
    let mut first = None;
    let mut last = 0;
    for x in 0..frame.size.x {
        if frame.lit(UVec2::new(x, row)) {
            first.get_or_insert(x);
            last = x;
        }
    }
    first.map_or(0, |first| last - first + 1)
}

/// A colour nothing else in these scenes wears, so finding it finds glyph
/// coverage and nothing else.
pub(crate) const INK: Vec3 = Vec3::new(1.0, 0.0, 1.0);

/// Where a frame's ink landed: how much of it there is, and the box it fills.
#[derive(Debug)]
pub(crate) struct Ink {
    pub(crate) count: u32,
    pub(crate) min: Vec2,
    pub(crate) max: Vec2,
}

impl Ink {
    pub(crate) fn found_in(frame: &Frame) -> Self {
        let mut found = Self {
            count: 0,
            min: Vec2::splat(f32::MAX),
            max: Vec2::splat(f32::MIN),
        };
        for y in 0..frame.size.y {
            for x in 0..frame.size.x {
                let [r, g, b, _] = frame.pixel(UVec2::new(x, y));
                if r > 150 && b > 150 && g < 90 {
                    let at = Vec2::new(x as f32, y as f32);
                    found.count += 1;
                    found.min = found.min.min(at);
                    found.max = found.max.max(at);
                }
            }
        }
        found
    }

    /// The centroid, which is where the run sits as a whole.
    pub(crate) fn centre(&self) -> Vec2 {
        // Only ever asked of a frame with ink in it; the guard is so a failing
        // assertion prints a number rather than dividing by zero on the way.
        (self.min + self.max) * 0.5
    }
}

/// How many pixels two frames disagree about.
///
/// Differenced rather than counted against a threshold, because type drawn over
/// a lit solid lands on pixels that were already lit — a count would see none of
/// exactly the ink this is about.
pub(crate) fn differing(a: &Frame, b: &Frame) -> u32 {
    let mut n = 0;
    for y in 0..a.size.y {
        for x in 0..a.size.x {
            if a.pixel(UVec2::new(x, y)) != b.pixel(UVec2::new(x, y)) {
                n += 1;
            }
        }
    }
    n
}
