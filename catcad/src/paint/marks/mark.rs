//! One mark: where it stands in the drawing, and where its box lands on screen.

use aperture::Turn;
use glam::{DVec2, Vec2, Vec3};
use silverpoint::Constraint;

use crate::drawing::Drawing;
use crate::lens::Lens;
use crate::paint::MARK_FONT;

/// Where one mark goes, without saying whose it is.
///
/// Apart from the handle beside it because not every mark has one: a dimension a
/// tool is half-way through placing is drawn exactly as a stated one and is not
/// in the sketch, so there is no [`ConstraintId`](silverpoint::ConstraintId) to
/// name it by — see [`Proposed`](crate::paint::marks::Proposed). Everything
/// that *draws* a mark wants this half and nothing makes it ask for the other.
///
/// **Where the pixels are.** The rules that put a mark somewhere are plane
/// arithmetic and stay that way — see the module beside this one — but where the
/// *box* around it lands is a question in logical pixels, because the type holds
/// its size on screen. So the clearance, the stack's step and the arithmetic
/// that turns them into a place in the world live here, with the thing they are
/// about, rather than with the rules that had no camera to consult.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Mark {
    pub(crate) at: DVec2,
    /// Which way the mark is set, as a unit direction in the sketch.
    ///
    /// The span a dimension measures, where it measures one — which is what a
    /// draughtsman does and what makes a number read as belonging to the line
    /// under it rather than to the drawing at large. A relation with no span to
    /// speak of runs along the sketch's own +x.
    pub(crate) along: DVec2,
    pub(crate) lane: u8,
}

/// How far a mark's own middle stands clear of the geometry it names, in
/// line-heights.
///
/// Better than a box-height, so the mark clears the stroke it is about rather
/// than sitting on it. Across, a mark is centred on what it names and there is
/// nothing to state.
///
/// Reached only through [`Mark::rise`], which is what folds the stack in — and
/// that is the whole reason it is a constant rather than a number written where
/// the run is given it. A field is drawn *instead of* a mark and must land on
/// the same pixels, so where a mark's box stands has to be one answer: it lifts
/// the run, and [`Mark::centre`] measures from it on the field's behalf.
const MARK_CLEAR: f32 = 1.1;

/// How far a mark rises above the one below it in its stack, in line-heights.
///
/// One, which is the tightest spacing two runs cannot overlap in: an anchor is
/// a fraction of the run's *own* box, so a whole one is exactly the height of
/// the type. And a fraction of the box rather than a number of pixels, which is
/// what keeps a stack the same shape at every zoom without the camera being
/// consulted.
///
/// A column rather than a row, and that is an engineering fact rather than a
/// preference: how *wide* a run comes out depends on the faces the shaper falls
/// back through, which only the renderer knows — see
/// [`Text::extent`](aperture::Text) — so nothing laying marks out could say
/// where the second one in a row would start.
pub(crate) const STACK_STEP: f32 = 1.0;

/// How far below the middle of its number a dimension's own line runs, in
/// line-heights.
///
/// Under the figure rather than through it, which is where ISO 129 puts one and
/// what a number with a stroke across it is not. Less than [`MARK_CLEAR`], so
/// the line still stands clear of the geometry it measures — a rule drawn on the
/// edge it is about is an edge drawn twice.
const RULE_DROP: f32 = 0.6;

impl Mark {
    /// Where the mark is anchored in the world.
    ///
    /// The sketch coordinate put on the plane it was measured in, which is all
    /// this is — but it is the one step between what the rules answer and what
    /// a projection can be asked about, and two callers doing it by hand would
    /// be two chances to reach for the wrong drawing's plane.
    pub(crate) fn world(self, drawing: Drawing<'_>) -> Vec3 {
        drawing.plane().point(self.at).as_vec3()
    }

    /// How far this stands clear of the geometry it names, in logical pixels
    /// along the plane's own down.
    ///
    /// [`MARK_CLEAR`] and [`STACK_STEP`] as a length rather than as fractions of
    /// a box, which is what a lift is stated in — and the one form both readers
    /// want: the run is given it outright, and what stands in the run's place
    /// measures from it. Two spellings would be a field a lane off its own
    /// number, which is a thing you can only see by opening one.
    ///
    /// The lane comes into it because a mark sharing its place with others does
    /// not sit where a lone one would.
    fn rise(self) -> f32 {
        (MARK_CLEAR + f32::from(self.lane) * STACK_STEP) * MARK_FONT.line_height_px
    }

    /// How far a dimension's own line stands clear of the geometry it measures,
    /// in logical pixels along the plane's own down.
    ///
    /// Read off [`Mark::rise`] rather than stated apart from it, and that is the
    /// whole of why it is here: a mark and the line it sits on have to move
    /// together, and a stack that raised one and not the other would leave a
    /// number floating off its own dimension. Which is a thing you would only
    /// see on a drawing whose marks had piled up.
    pub(crate) fn rule_rise(self) -> f32 {
        self.rise() - RULE_DROP * MARK_FONT.line_height_px
    }

    /// How the mark is laid in the drawing's plane.
    ///
    /// One statement, read twice: the run is given it, and whatever stands in
    /// the run's place measures from it. The two coming to different answers is
    /// a field that misses its own number, and on a plane seen at an angle it
    /// would miss it in a different direction every frame.
    pub(crate) fn turn(self, drawing: Drawing<'_>) -> Turn {
        let plane = drawing.plane();
        let along = plane.x * self.along.x + plane.y * self.along.y;
        Turn::new(along.as_vec3(), plane.normal().as_vec3()).lifted(Vec2::new(0.0, self.rise()))
    }

    /// Where the middle of the mark's box sits in the world.
    ///
    /// **What a form standing *in place of* a mark is placed against** — see
    /// [`Stands::Over`](crate::prompt::Stands). The middle rather than the
    /// anchor, because the anchor is the geometry the mark is about and the box
    /// floats clear of it; and the middle rather than a corner, because that is
    /// where the run's own width cancels: a field centring its own text here
    /// lands on the mark's pixels whatever the number measures.
    ///
    /// Read through the same [`Turn::axes`] the mark was drawn along, so the
    /// rise clears the geometry up the *run's* own frame rather than up the
    /// screen — which on a plane seen at any angle are two different directions,
    /// and on the plane the camera happens to face are one.
    ///
    /// The lens comes into it for one thing only: what a logical pixel is worth
    /// in the world at the mark, the frame being the plane's. It does not reach
    /// [`redraw`](crate::paint::redraw) — a mark is laid out against the
    /// document and sized against the screen by the *shader* — so this is on the
    /// asking half's schedule, which is where the form that reads it already is.
    pub(crate) fn centre(self, drawing: Drawing<'_>, lens: Lens) -> Vec3 {
        let anchor = self.world(drawing);
        anchor + self.turn(drawing).lift_world() * lens.world_per_pixel(anchor)
    }

    /// How far the box stands off the point it names, in the world.
    ///
    /// **What a press on a number records.** The box floats clear of the
    /// geometry so the figure can be read, and a placement says where the
    /// *point* goes — so a press lands in the box while the change writes the
    /// anchor, and the two differ by exactly this. Reading it here is what lets
    /// the grab keep the number under the cursor rather than snapping it.
    ///
    /// Its inverse is [`Mark::anchor`] and not a subtraction of this, which is
    /// the whole of what that one is for: taken at the press it is a constant,
    /// and a radius's clearance is not.
    pub(crate) fn standoff(self, drawing: Drawing<'_>, lens: Lens) -> Vec3 {
        self.centre(drawing, lens) - self.world(drawing)
    }

    /// Where the mark is anchored for its box to land on `at`.
    ///
    /// **The inverse of [`Mark::centre`], and stated beside it because a drag
    /// needs both.** A number is grabbed by its box and placed by its anchor, so
    /// a gesture that says where the box goes has to answer where the anchor
    /// goes — and taking the clearance off by subtraction is only right where the
    /// clearance does not depend on the answer.
    ///
    /// For every dimension but one it does not: the direction a mark is set
    /// along comes from the geometry it measures, which a drag on the number
    /// leaves alone. A **radius** is the one, because its leader runs out
    /// through wherever the number was *put* — so the clearance turns as the
    /// number moves, and taking off the last frame's is a drag chasing an answer
    /// that keeps changing. Held still it did not stop: at some bearings the
    /// number swapped between two places a whole clearance apart, frame after
    /// frame, for ever.
    ///
    /// So a radius is inverted rather than iterated, and it inverts in closed
    /// form. The clearance is square to the leader, so with `q` running from the
    /// circle's centre to the box, `q = p + clear · perp(p̂)` is a right
    /// triangle: Pythagoras gives `|p|` outright, and reading `q` against the
    /// basis `p̂` makes gives the direction in one line more. No search, no lag,
    /// and nothing left to be unstable.
    pub(crate) fn anchor(
        self,
        constraint: Constraint,
        drawing: Drawing<'_>,
        lens: Lens,
        at: Vec3,
    ) -> Vec3 {
        // Taken at the box rather than at the point it is solving for, which is
        // the one approximation here and is a fraction of a pixel: the two are a
        // clearance apart, and what a pixel is worth changes over that distance
        // only as far as the perspective divide does.
        let step = lens.world_per_pixel(at);
        let Constraint::Radius { circle, .. } = constraint else {
            // Square to a direction the geometry decides, so taking it off is
            // exact.
            return at - self.turn(drawing).lift_world() * step;
        };
        let sketch = drawing.sketch();
        let centre = sketch.point(sketch.circle(circle).center).position;
        let plane = drawing.plane();
        let clear = f64::from(self.rise() * step);
        let q = plane.flatten(at.as_dvec3()) - centre;
        let across = q.length_squared();
        let reach = (across - clear * clear).max(0.0).sqrt();
        let point = if across > 0.0 {
            (q * reach - q.perp() * clear) * (reach / across)
        } else {
            DVec2::ZERO
        };
        plane.point(centre + point).as_vec3()
    }
}
