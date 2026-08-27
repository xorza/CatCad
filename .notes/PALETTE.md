# A palette file, from ayu-graphite

CatCad states every colour twice: once in `look/chrome.rs` and once in
`look/drawing.rs`, both as a private `DARK` const. This proposal replaces both
with one generated data file, and makes that file a target of
`~/Projects/ayu-graphite` — the palette this machine already runs in Zed, Claude
Code, Konsole, Plasma and Brave.

## What the research found

**1. ayu-graphite is almost neutral where CatCad needs it, and tinted where it
does not.** Channel spread (R−B) of the neutral ramp:

| step | hex | R−B | tint |
| --- | --- | ---: | --- |
| `gray_950` | `#1f1e1d` | +2 | flat |
| `gray_900` | `#282726` | +2 | flat |
| `gray_850` | `#323130` | +2 | flat |
| `gray_800` | `#3d3c3b` | +2 | flat |
| `gray_750` | `#484745` | +3 | warm |
| `gray_600` | `#70747a` | −10 | cool |
| `gray_500` | `#878a8d` | −6 | cool |
| `gray_400` | `#aaaaa8` | +2 | flat |
| `gray_300` | `#c7c6bf` | +8 | warm |
| `gray_200` | `#e2dfd3` | +15 | warm |
| `gray_100` | `#f6f6f6` | 0 | flat |

The ramp runs warm, then cool, then warm again. The four darkest steps — which
are the chrome CatCad draws most of — are already flat to within two counts.
Neutralisation is therefore a correction, not a new palette.

**2. The correction is exact, not a matter of taste.** Hold each step's relative
luminance and drop its chroma to zero. The ramp keeps its spacing, so the
`tools/audit.py` layer rule is unaffected:

| step | ayu | neutral | Y |
| --- | --- | --- | ---: |
| `gray_950` | `#1f1e1d` | `#1e1e1e` | 0.0131 |
| `gray_900` | `#282726` | `#272727` | 0.0204 |
| `gray_850` | `#323130` | `#313131` | 0.0309 |
| `gray_800` | `#3d3c3b` | `#3c3c3c` | 0.0454 |
| `gray_750` | `#484745` | `#474747` | 0.0631 |
| `gray_600` | `#70747a` | `#747474` | 0.1734 |
| `gray_500` | `#878a8d` | `#8a8a8a` | 0.2525 |
| `gray_400` | `#aaaaa8` | `#aaaaaa` | 0.4012 |
| `gray_300` | `#c7c6bf` | `#c6c6c6` | 0.5629 |
| `gray_200` | `#e2dfd3` | `#dfdfdf` | 0.7365 |
| `gray_100` | `#f6f6f6` | `#f6f6f6` | 0.9216 |

**3. CatCad's chip ladder already sits on this ramp.** The current chrome is the
same ladder with blue added:

| role | today | ayu neutral |
| --- | --- | --- |
| `chip` | `#31363c` | `#313131` |
| `chip_lit` | `#3d444b` | `#3c3c3c` |
| `chip_active` | `#49515a` | `#474747` |
| `chip_held` | `#d6dde4` | `#dfdfdf` |
| `ink_lit` | `#d3dae1` | `#dfdfdf` |

The overlay changes very little. Only `ink` and `ink_dim` move far.

**4. The ramp has one hole.** Every step is spaced near 1.15:1, except
`gray_750` → `gray_600`, which is 1.975:1. CatCad's `ink_dim` falls in that gap:
today it is `#5c646d`, and ayu's `text_disabled` is `#8a8a8a`, which is much
brighter. A step at `#565656` would fill the hole. See decision 3.

**5. The ground is stated twice, in two crates, and both are blue.**
`Chrome::ground` is `linear_rgb(0.02, 0.02, 0.025)`, and
`aperture3d/src/renderer/gpu/attachments.rs` states the same triple as
`BACKGROUND`. The blue channel is higher in both. A palette cannot own the ground
until aperture takes its clear from the caller.

**6. Two serde derives are unused.** `Chrome` and `Form` derive `Serialize` and
`Deserialize`. Nothing reads them. The palette file replaces the reason they
were added.

**7. The name `Palette` is taken.** `look/mod.rs` imports `palantir::Palette`,
and `Theme::palette()` returns one. A CatCad `Palette` collides with both.

## The shape

**CatCad becomes an ayu-graphite target.** That repository's `CLAUDE.md` says
"`ayu-graphite.toml` is the only palette definition. Do not introduce a second
one", and every target is a pure transformer that reads the TOML and writes one
file. A hand-maintained copy of the colours in CatCad would be a second
definition. A generated one is not.

```
ayu-graphite.toml  ──  catcad/build.py  ──▶  catcad/src/look/palette/ayu-graphite.ron
                       (resolves, neutralises)          │
                                                        │ include_str!
                                                        ▼
                                                    Palette
                                                        │ Theme::from_palette
                                                        ▼
                                          Chrome · Drawing · Form · Lighting
```

The builder does three things. It reads the resolved semantic palette through
`load_palette`. It neutralises every grey role by the luminance rule above. It
writes CatCad's roles as hex.

Colours only. Sizes stay in Rust: a stroke width and a chip side are facts about
the interface, not about the palette, and ayu-graphite has no place to put them.

Opacity also stays in Rust. How much of the drawing shows through a pill is a
decision about the overlay, so `Chrome` fades the palette's `pill`, `pill_edge`
and `rule` itself.

### The Rust side

`catcad/src/look/palette/mod.rs` holds `Palette`: a flat struct of 36 named
`Swatch` fields, one per role, deserialized straight from the file.

`catcad/src/look/palette/swatch.rs` holds `Swatch(u32)` — packed `0xRRGGBB`,
which is what the file says and what round-trips exactly. It hands out both
currencies:

```rust
pub(crate) const fn color(self) -> Color;   // palantir, linear
pub(crate) fn ink(self) -> Vec3;            // aperture, linear
pub(crate) const fn fade(self, alpha: u8) -> Color;
```

`Serialize` and `Deserialize` are written by hand, as `"#rrggbb"`. A float triple
does not round-trip to the hex a designer reads, and `Color` quantises with a
loss of up to one count per channel.

`Palette::default()` parses the embedded file and calls `.expect()`. A malformed
shipped file is a logic error, and a test that parses it turns the crash into a
test failure.

## The mapping

Every CatCad role, and the ayu-graphite semantic role it is taken from. Greys
pass through the neutralisation. Hues do not.

### Chrome

| CatCad | ayu role | primitive | value | today |
| --- | --- | --- | --- | --- |
| `ground` | `bg` | `gray_950` | `#1e1e1e` | `#27272c` |
| `pill` | `panel` | `gray_900` | `#272727` | `#181c21` |
| `pill_edge` | `text` | `gray_200` | `#dfdfdf` | `#becddc` |
| `rule` | `text` | `gray_200` | `#dfdfdf` | `#becddc` |
| `chip` | `elem` | `gray_850` | `#313131` | `#31363c` |
| `chip_lit` | `elem_hover` | `gray_800` | `#3c3c3c` | `#3d444b` |
| `chip_active` | `elem_active` | `gray_750` | `#474747` | `#49515a` |
| `chip_held` | `text` | `gray_200` | `#dfdfdf` | `#d6dde4` |
| `on_held` | `bg` | `gray_950` | `#1e1e1e` | `#141a20` |
| `ink` | `text_muted` | `gray_400` | `#aaaaaa` | `#8b949e` |
| `ink_lit` | `text` | `gray_200` | `#dfdfdf` | `#d3dae1` |
| `ink_dim` | `text_disabled` | `gray_500` | `#8a8a8a` | `#5c646d` |
| `cube_low` | `panel` | `gray_900` | `#272727` | `#2c3138` |
| `cube_high` | `line_number` | `gray_600` | `#747474` | `#59626c` |
| `focus` | — | `gray_200` | `#dfdfdf` | `#d6dde4` |

`pill_edge` and `rule` take the light ink rather than ayu's `border`. The pill is
translucent, so its rim is a light hairline at low alpha. ayu's `border`
(`#3c3c3c`) is meant for an opaque panel and would draw nothing here.

### Drawing

| CatCad | ayu role | primitive | value | hue |
| --- | --- | --- | --- | ---: |
| `solid` | `text_muted` | `gray_400` | `#aaaaaa` | — |
| `determined` | `syn_type` | `cyan_400` | `#7adcf3` | 191° |
| `partly` | `warning` | `yellow_400` | `#ffd44a` | 46° |
| `free` | `syn_keyword` | `orange_400` | `#ffa63d` | 33° |
| `pinned` | `error` | `red_400` | `#ff6b52` | 9° |
| `dormant` | `text_disabled` | `gray_500` | `#8a8a8a` | — |
| `dormant_face` | `hint_bg` | `blue_900` | `#0f3e58` | 203° |
| `face` | `selection_bg` | `blue_750` | `#123a5c` | 206° |
| `ghost` | `syn_punctuation` | `gray_300` | `#c6c6c6` | — |
| `sheet_ground` | `ansi_dim_green` | `green_500` | `#75a228` | 82° |
| `sheet_front` | `ansi_blue` | `blue_500` | `#5985a3` | 200° |
| `sheet_side` | `ansi_dim_red` | `red_500` | `#b05043` | 7° |
| `sheet_datum` | `line_number` | `gray_600` | `#747474` | — |
| `mark` | `syn_number` | `purple_400` | `#d897ff` | 278° |
| `redundant` | `ansi_bright_red` | `red_300` | `#ff8f7a` | 10° |
| `depth_arrow` | `scrollbar_thumb` | `gray_400` | `#aaaaaa` | — |

`face` takes ayu's own selection fill, which is what a region is.

The freedom ladder keeps its separation. Today `partly` and `free` differ by
13.5° of hue; here they differ by 13.2°.

### Lighting and form

| CatCad | ayu role | primitive | value |
| --- | --- | --- | --- |
| `hovered` | `ansi_bright_yellow` | `yellow_250` | `#ffe79b` |
| `selected` | `success` | `lime_400` | `#daff58` |
| `goes` | `success_border` | `green_700` | `#668a2f` |
| `stops` | `error_border` | `red_700` | `#703130` |
| `doing` | `info_border` | `blue_600` | `#4a8ab0` |

## Stages

### Stage 1 — `Swatch` and `Palette`

Add `look/palette/{mod.rs, swatch.rs, ayu-graphite.ron}`. Write the RON by hand
from the table above. `Palette::default()` parses it.

Nothing reads it yet, so nothing renders differently.

Tests: a `Swatch` round-trips through hex; the shipped file parses; the six
chrome layers that stack in one view clear 1.10:1, which is the same rule
`tools/audit.py` applies upstream.

### Stage 2 — the ctors

Add `Chrome::from_palette`, `Drawing::from_palette`, `Form::from_palette`,
`Lighting::from_palette`, and `Theme::from_palette`. `Default for Theme` calls
`Theme::from_palette(&Palette::default())`.

Delete the four `DARK` consts. Delete the two unused serde derives. Rename
`Theme::palette()` to `Theme::roles()`, because the word now means the other
thing.

The render changes here. Regenerate the three goldens in
`catcad/tests/visual/golden/` in a commit that changes nothing else.

Tests: the existing theme tests already take their values from
`Theme::default()`, so they follow. Add one that a role changed in the palette
reaches the built theme, which is what proves the ctor is a derivation and not a
copy.

### Stage 3 — the builder

Add `ayu-graphite/catcad/build.py`, next to its siblings. Add `"catcad"` to
`TARGETS` in `build.py`, a `catcad:` rule to the `Makefile`, its output to
`clean`, and a line to the README table.

The builder holds the neutralisation and the mapping. It writes CatCad's RON.

Acceptance: the builder's output equals the file stage 1 wrote, byte for byte.
That is the check that the two halves agree, and it fails loudly if either
drifts.

### Stage 4 — the ground, once

`aperture3d` states its clear in `renderer/gpu/attachments.rs`. Give the renderer
the clear colour, and `Chrome::ground` stops being a second opinion about the
same sliver.

This reaches a second crate, so it verifies `-p aperture3d -p catcad`.

## Decisions

**1. Where the neutralisation runs.**

- *In `catcad/build.py`* (recommended). The other targets keep the warm ramp.
  There is still one palette definition. Moving it upstream later is a one-line
  change.
- *In `ayu-graphite.toml`*. Zed, Claude Code, Plasma, Konsole, Brave and Terminal
  all go neutral too. The cost is that `gray_200` — the body text of the editor —
  loses its cream, which is the most visible change in the whole set.

**2. What the focus ring wears.**

- *The inversion* (recommended). `Chrome` argues that the drawing spends every
  hue on a meaning, so chrome says "this one" by going light instead of coloured.
  ayu's accent is `cyan_500`, and this proposal spends `cyan_400` on
  `determined`, so a cyan ring would be a second meaning in one hue.
- *ayu's `accent`*, `#59d4ff`. A focus ring only appears under keyboard
  navigation and never on the drawing, so the collision is narrow.

**3. What `ink_dim` wears.** ayu's `text_disabled` is `#8a8a8a`, against today's
`#5c646d`. That is bright for a mark on a viewport.

- *Accept it*. It is what the audit passed, and nothing CatCad draws is disabled.
- *Add `gray_700` = `#565656` upstream*, filling the 1.975:1 hole, and take that.
  This is a TOML edit, so it interacts with decision 1.

## Cost

The drawing gets more saturated. Today's colours are pastel — `determined` lands
at `#a0c4e7` and `free` at `#f1bc59` — and ayu's are not. For a 1.6-pixel stroke
read against a shaded solid, that is likely an improvement, but it is the change
you will see first.

Three golden images change. Nothing else in the test suite measures a colour.
