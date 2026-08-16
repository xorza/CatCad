use super::*;
use glam::UVec2;

/// A window over the whole view changes nothing, and one over part of it
/// brings that part out to fill the target.
///
/// Hand-computed against a 800×600 view, in clip space with `w = 1` so the
/// numbers are NDC directly. Each case names a point of the *view* and where
/// in the target it should land: the window's own middle always lands dead
/// centre, and the view's centre lands wherever the window puts it.
///
/// The identity case is the one that guards every frame there is — almost
/// no view is clipped — and the vertical cases are the ones that catch the
/// y-flip, which is the error this arithmetic invites: the window is
/// measured down from the view's top and NDC counts up from its middle, so
/// a sign dropped there is a picture that is right until something clips it
/// and then upside down about the wrong axis.
#[test]
fn a_window_brings_its_own_part_of_the_view_out_to_fill_the_target() {
    let viewport = Viewport::new(UVec2::new(800, 600));
    let ndc = |m: Mat4, x: f32, y: f32| {
        let out = m * Vec4::new(x, y, 0.0, 1.0);
        Vec2::new(out.x / out.w, out.y / out.w)
    };

    // The whole view: every point stays where it was.
    let whole = Window {
        min: Vec2::ZERO,
        size: Vec2::new(800.0, 600.0),
    }
    .onto(viewport);
    for (x, y) in [(0.0, 0.0), (-1.0, -1.0), (1.0, 1.0), (0.3, -0.7)] {
        let at = ndc(whole, x, y);
        assert!(
            (at - Vec2::new(x, y)).length() < 1e-6,
            "the whole view moved {x},{y} to {at:?}"
        );
    }

    struct Case {
        min: Vec2,
        size: Vec2,
        /// Where the view's own centre lands.
        centre_to: Vec2,
    }
    let cases = [
        // The left half: the view's centre is its right edge.
        Case {
            min: Vec2::new(0.0, 0.0),
            size: Vec2::new(400.0, 600.0),
            centre_to: Vec2::new(1.0, 0.0),
        },
        // The right half: the view's centre is its left edge.
        Case {
            min: Vec2::new(400.0, 0.0),
            size: Vec2::new(400.0, 600.0),
            centre_to: Vec2::new(-1.0, 0.0),
        },
        // The bottom half — offset *down* the view, so the view's centre is
        // the top of the target, which in NDC is +1.
        Case {
            min: Vec2::new(0.0, 300.0),
            size: Vec2::new(800.0, 300.0),
            centre_to: Vec2::new(0.0, 1.0),
        },
        // And the top half, the other way about.
        Case {
            min: Vec2::new(0.0, 0.0),
            size: Vec2::new(800.0, 300.0),
            centre_to: Vec2::new(0.0, -1.0),
        },
    ];
    for Case {
        min,
        size,
        centre_to,
    } in cases
    {
        let m = Window { min, size }.onto(viewport);
        let centre = ndc(m, 0.0, 0.0);
        assert!(
            (centre - centre_to).length() < 1e-6,
            "window at {min:?} of {size:?} put the view's centre at {centre:?}, not {centre_to:?}"
        );
        // And the window's own middle is the middle of the target, whatever
        // else moved — the one point every window agrees about.
        let middle = (min + size * 0.5) / Vec2::new(800.0, 600.0) * 2.0 - 1.0;
        let landed = ndc(m, middle.x, -middle.y);
        assert!(
            landed.length() < 1e-6,
            "window at {min:?} did not centre its own middle: {landed:?}"
        );
    }
}
