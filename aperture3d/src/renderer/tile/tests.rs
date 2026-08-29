use super::*;

/// A tile of the view, for a test that names them in pixels.
fn tile(min: (i32, i32), size: (u32, u32)) -> Tile {
    Tile {
        min: IVec2::new(min.0, min.1),
        size: UVec2::new(size.0, size.1),
    }
}

const VIEW: Tile = Tile {
    min: IVec2::ZERO,
    size: UVec2::new(800, 600),
};

/// A hundred and twenty pixels square, in the view's bottom-right corner.
const CORNER: Tile = Tile {
    min: IVec2::new(660, 460),
    size: UVec2::splat(120),
};

/// Where `ndc` of the pane lands on the target, in the target's own pixels,
/// through the matrix.
fn landed(pane: Tile, target: Tile, ndc: Vec2) -> Vec2 {
    let out = pane.onto(target) * Vec4::new(ndc.x, ndc.y, 0.0, 1.0);
    target
        .viewport()
        .pixel_from_ndc(Vec2::new(out.x / out.w, out.y / out.w))
}

/// The same, worked out in pixels instead: across the pane's own tile, then off
/// the target's corner.
///
/// The other half of the cross-check, and deliberately no part of the matrix —
/// the y-flip is written here as a sign on the way *in*, where the matrix
/// writes it as a sign on a translation, so the two agree only if both are
/// right.
fn want(pane: Tile, target: Tile, ndc: Vec2) -> Vec2 {
    let across = (Vec2::new(ndc.x, -ndc.y) * 0.5 + 0.5) * pane.size.as_vec2();
    pane.min.as_vec2() + across - target.min.as_vec2()
}

/// **A pane lands where the target shows its tile of the view**, whichever part
/// of the view either of them is.
///
/// Two answers cross-checked: the clip-space matrix, and the same placement
/// done in pixels. The identity case is the one that guards every frame there
/// is — almost no view is clipped and almost every pane is the whole of one —
/// and the four crops are what catch the y-flip, which is the error this
/// arithmetic invites: a tile is measured down from the view's top and NDC
/// counts up from its middle, so a sign dropped there is a picture that is
/// right until something clips it and then upside down about the wrong axis.
///
/// The corner cases are the ones a pinned pane is, including the one where it
/// lies wholly outside the target — a scroll can do that, and the placement has
/// to keep answering rather than fold onto an edge.
#[test]
fn a_pane_lands_where_the_target_shows_its_tile_of_the_view() {
    let left = tile((0, 0), (400, 600));
    let right = tile((400, 0), (400, 600));
    let top = tile((0, 0), (800, 300));
    let bottom = tile((0, 300), (800, 300));

    for (pane, target) in [
        (VIEW, VIEW),
        (VIEW, left),
        (VIEW, right),
        (VIEW, top),
        (VIEW, bottom),
        (CORNER, VIEW),
        (CORNER, right),
        (CORNER, left),
    ] {
        for (x, y) in [
            (0.0, 0.0),
            (-1.0, -1.0),
            (1.0, 1.0),
            (1.0, -1.0),
            (-1.0, 1.0),
        ] {
            let ndc = Vec2::new(x, y);
            let (was, should) = (landed(pane, target, ndc), want(pane, target, ndc));
            assert!(
                (was - should).length() < 1e-3,
                "{pane:?} in {target:?} put {ndc:?} at {was:?}, not {should:?}",
            );
        }
    }
}

/// The corner pane's own numbers, worked by hand, so the pair above is anchored
/// to something outside both of them.
///
/// The pane's middle is at 720, 520 of an 800×600 view. Across, that is
/// `720 / 800 × 2 − 1`, or `0.8`. Down, NDC counts the other way, so it is
/// `1 − 520 / 600 × 2`, or `−0.7333`. Its top-right corner is at 780, 460,
/// which the same two sums put at `0.95` and `−0.5333`.
#[test]
fn a_corner_pane_is_skewed_to_the_corner_it_sits_in() {
    let onto = CORNER.onto(VIEW);
    for (ndc, want) in [
        (Vec2::ZERO, Vec2::new(0.8, -22.0 / 30.0)),
        (Vec2::ONE, Vec2::new(0.95, -16.0 / 30.0)),
    ] {
        let out = onto * Vec4::new(ndc.x, ndc.y, 0.0, 1.0);
        let was = Vec2::new(out.x / out.w, out.y / out.w);
        assert!(
            (was - want).length() < 1e-6,
            "{ndc:?} of the corner landed at {was:?}, not {want:?}",
        );
    }
}

/// **What a pane is clipped to is the part of it the target holds**, in the
/// target's own pixels.
///
/// The three cases a scissor has: wholly inside, cut by an edge, and wholly
/// outside. The last is the one that would otherwise reach wgpu as a rect
/// leaving the attachment, which is refused rather than ignored.
///
/// Worked by hand. The corner pane runs 660..780 across and 460..580 down. The
/// right half of the view starts at 400, so inside it the pane begins at
/// `660 − 400`, or 260, and keeps its full 120. Cut against the left half,
/// which ends at 400, nothing of it survives at all.
#[test]
fn a_pane_is_cut_to_the_part_of_the_target_it_reaches() {
    let right = tile((400, 0), (400, 600));
    let left = tile((0, 0), (400, 600));
    assert_eq!(CORNER.within(VIEW), Some(CORNER), "the whole view cut it");
    assert_eq!(
        CORNER.within(right),
        Some(tile((260, 460), (120, 120))),
        "the pane was not re-based on the target it lands in",
    );
    assert_eq!(
        CORNER.within(left),
        None,
        "a pane off the target was cut to"
    );

    // An edge crossed, rather than a corner cleared or missed: the view's
    // left half keeps the first 400 of a pane that starts at 300 and runs 200
    // across, which is the 100 up to the edge.
    let straddling = tile((300, 100), (200, 200));
    assert_eq!(
        straddling.within(left),
        Some(tile((300, 100), (100, 200))),
        "the part past the edge was kept",
    );
    // And a pane that begins left of the view keeps only what is on it, with
    // its corner brought back to the target's own.
    let over = tile((-50, -30), (100, 100));
    assert_eq!(
        over.within(VIEW),
        Some(tile((0, 0), (50, 70))),
        "a pane reaching off the top-left was not brought back",
    );
}
