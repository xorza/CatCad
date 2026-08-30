use super::*;

/// The shipped file parses, and says what the mapping says it says.
///
/// One value out of each roster, chosen where a mistake would be quiet: a
/// ground that came back a shade off is a seam, and a hue off the freedom
/// ladder is a drawing making a claim it does not mean.
///
/// **Written out rather than read back off the table**, which is the only
/// way it says anything: a check that fetched its own expectation would
/// pass whatever the generator emitted. So these six move by hand whenever
/// the palette upstream is regenerated, and that is the moment somebody
/// looks at what moved.
#[test]
fn the_shipped_palette_parses_and_holds_the_mapped_colours() {
    let palette = Palette::default();
    assert_eq!(palette.ground, Swatch::of(0x1f1f1f));
    assert_eq!(palette.chip, Swatch::of(0x383838));
    assert_eq!(palette.determined, Swatch::of(0x59bafe));
    assert_eq!(palette.pinned, Swatch::of(0xff9f8f));
    assert_eq!(palette.selected, Swatch::of(0x83c774));
    assert_eq!(palette.goes, Swatch::of(0x3e7f2f));
}

/// Every grey in the table is grey.
///
/// The whole reason the palette is generated through a neutralisation
/// rather than copied: ayu's own ramp runs cool at `gray_600` and warm at
/// `gray_200`, and a step that arrived here still tinted would be the
/// tint nobody asked for, in the one place it is most visible — a large
/// flat surface.
#[test]
fn every_neutral_role_has_no_tint_left() {
    let palette = Palette::default();
    for (role, swatch) in [
        ("ground", palette.ground),
        ("pill", palette.pill),
        ("pill_edge", palette.pill_edge),
        ("rule", palette.rule),
        ("chip", palette.chip),
        ("chip_lit", palette.chip_lit),
        ("chip_active", palette.chip_active),
        ("chip_held", palette.chip_held),
        ("on_held", palette.on_held),
        ("ink", palette.ink),
        ("ink_lit", palette.ink_lit),
        ("ink_dim", palette.ink_dim),
        ("cube_low", palette.cube_low),
        ("cube_high", palette.cube_high),
        ("focus", palette.focus),
        ("solid", palette.solid),
        ("dormant", palette.dormant),
        ("ghost", palette.ghost),
        ("sheet_datum", palette.sheet_datum),
        ("depth_arrow", palette.depth_arrow),
    ] {
        let color = swatch.color();
        assert!(
            color.r == color.g && color.g == color.b,
            "{role} is {} and still carries a tint",
            ron::to_string(&swatch).unwrap()
        );
    }
}

/// The table survives being written back out, which is what says the file
/// and this struct agree on all thirty-six roles rather than on the six the
/// test above names.
#[test]
fn the_palette_round_trips_through_its_own_format() {
    let palette = Palette::default();
    let text = ron::to_string(&palette).unwrap();
    assert_eq!(ron::from_str::<Palette>(&text).unwrap(), palette);
    assert_eq!(text.matches("\"#").count(), 36);
}

/// The table is the generator's output and not a hand-written copy of it.
///
/// The one property this whole arrangement rests on: the palette lives in
/// `ayu-graphite.toml` and reaches here through `ayu-graphite/catcad/build.py`.
/// A table somebody edited in place would be a second definition of every
/// colour, and two definitions of a colour are two colours as soon as either
/// is touched.
#[test]
fn the_table_is_generated_rather_than_written_here() {
    let banner = TABLE.lines().next().expect("the table is not empty");
    assert!(banner.contains("generated"), "{banner}");
    assert!(
        TABLE.contains("ayu-graphite/catcad/build.py"),
        "the table does not say what wrote it"
    );
}

/// A role the generator renamed is a hard error rather than a colour
/// quietly left at whatever the struct happened to say.
#[test]
fn a_table_with_a_role_this_crate_does_not_know_is_refused() {
    let stale = TABLE.replacen("ground:", "backdrop:", 1);
    assert!(ron::from_str::<Palette>(&stale).is_err());
}

/// The suite's table is a table of its own rather than a copy of the
/// shipped one.
///
/// What it is for: a probe that happened to equal the shipped palette would
/// pin nothing, and would go on pinning nothing right up until the
/// generator moved a colour. That all thirty-six roles are answered needs
/// no test — the literal is checked by the compiler.
#[test]
fn the_suites_palette_stands_apart_from_the_shipped_one() {
    assert_ne!(Palette::probe(), Palette::default());
}

/// Every colour a stripped frame can carry, which is what the sweeps below
/// have to tell apart.
///
/// The chrome is left out because the suite paints the drawing through a
/// bare pane: a chip, a pill and the ink on them reach no frame it
/// measures. See `harness::shown`.
fn drawn(palette: &Palette) -> [(&'static str, Swatch); 19] {
    [
        ("ground", palette.ground),
        ("solid", palette.solid),
        ("determined", palette.determined),
        ("partly", palette.partly),
        ("free", palette.free),
        ("pinned", palette.pinned),
        ("dormant", palette.dormant),
        ("dormant_face", palette.dormant_face),
        ("face", palette.face),
        ("ghost", palette.ghost),
        ("sheet_ground", palette.sheet_ground),
        ("sheet_front", palette.sheet_front),
        ("sheet_side", palette.sheet_side),
        ("sheet_datum", palette.sheet_datum),
        ("mark", palette.mark),
        ("redundant", palette.redundant),
        ("depth_arrow", palette.depth_arrow),
        ("hovered", palette.hovered),
        ("selected", palette.selected),
    ]
}

/// The three roles the suite sweeps a frame for stand clear of every other
/// colour the drawing paints.
///
/// A sweep asks whether a pixel wears a role, and allows the pixel a few
/// counts for its trip through an sRGB target. What that costs is that two
/// roles inside one window are one role as far as the sweep is concerned —
/// a background it counted as geometry, or a rim it counted twice.
///
/// **Four times the widest window any sweep opens**, which is what makes
/// this a floor rather than a copy of the harness's own number. The three
/// are the roles the suite has a reason to find: the ground says what is
/// drawing and what is not, a pinned point is located by its colour alone,
/// and a free rim is measured by the ink it puts down.
#[test]
fn the_roles_a_sweep_reads_stand_clear_of_every_other() {
    const APART: u8 = 32;
    let probe = Palette::probe();
    let roster = drawn(&probe);
    for swept in ["ground", "pinned", "free"] {
        let (_, mine) = roster.iter().find(|(role, _)| *role == swept).unwrap();
        for (role, other) in roster.iter().filter(|(role, _)| *role != swept) {
            let apart = std::iter::zip(mine.srgb(), other.srgb())
                .map(|(a, b)| a.abs_diff(b))
                .max()
                .unwrap();
            assert!(
                apart >= APART,
                "{swept} and {role} are {apart} counts apart, so a sweep for \
                 {swept} answers for both",
            );
        }
    }
}

/// Nothing the drawing paints is the magenta the suite stages.
///
/// `ink::INK` is pushed into a scene as a colour that scene cannot already
/// wear, and then counted to find where a highlight or a run of type
/// landed. A role inside the same window would be counted as ink, and the
/// count would say a highlight was drawn wherever the palette happened to
/// reach.
#[test]
fn no_role_the_drawing_paints_can_be_taken_for_staged_ink() {
    for (role, swatch) in drawn(&Palette::probe()) {
        let [r, g, b] = swatch.srgb();
        assert!(
            !(r > 150 && b > 150 && g < 90),
            "{role} is {} and would be counted as staged ink",
            ron::to_string(&swatch).unwrap()
        );
    }
}
