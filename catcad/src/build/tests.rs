use super::*;
use crate::build::bodied::Built;
use crate::document::Document;
use crate::drawing::Grip;
use crate::drawing::anchor::Anchor;
use crate::intent::change::Change;
use crate::model::{Broken, Models};
use crate::timeline::Timeline;
use crate::timeline::feature::{Datum, Feature, World};
use glam::{DVec2, Vec3};
use silverpoint::{Entity, Operation, Plane, Sector, SegmentId};
use std::f64::consts::FRAC_PI_2;

/// A square: four free points and the edges between them, which shuts one
/// region in and leaves eight degrees of freedom.
fn square() -> Sketch {
    let mut sketch = Sketch::default();
    let corners: Vec<_> = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]
        .map(|(x, y)| sketch.add_point(DVec2::new(x, y)))
        .into();
    for at in 0..corners.len() {
        sketch.add_segment(corners[at], corners[(at + 1) % corners.len()]);
    }
    sketch
}

/// One circle of `radius` about `(x, 0)`.
fn ring(x: f64, radius: f64) -> Sketch {
    let mut sketch = Sketch::default();
    let center = sketch.add_point(DVec2::new(x, 0.0));
    sketch.add_circle(center, radius);
    sketch
}

/// A drawing a revolve is made from, and the line in it to spin about.
struct Lathed {
    sketch: Sketch,
    axis: SegmentId,
}

impl Lathed {
    /// A circle of `minor` about `(major, 0)`, beside the drawing's own `y`
    /// axis — so spun about that line it traces a ring of those two radii.
    fn new(major: f64, minor: f64) -> Self {
        let mut sketch = Sketch::default();
        let center = sketch.add_point(DVec2::new(major, 0.0));
        sketch.add_circle(center, minor);
        let low = sketch.add_point(DVec2::new(0.0, -1.0));
        let high = sketch.add_point(DVec2::new(0.0, 1.0));
        let axis = sketch.add_segment(low, high);
        Self { sketch, axis }
    }
}

/// A document holding one revolve, and what it takes to read the solid back.
///
/// Everything owned, because the three are made together and a reading borrows
/// all of them: a test that kept only the document would have nothing to hand
/// [`Document::models`].
struct Spun {
    document: Document,
    build: Build,
    sketch: FeatureId,
}

impl Spun {
    /// [`Lathed`]'s ring spun through `sector`, by the whole path a press
    /// takes: a change naming a region and a segment, a step of the timeline,
    /// and a body built from it.
    fn new(sector: Sector) -> Self {
        let mut timeline = Timeline::default();
        let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
        let drawn = Lathed::new(3.0, 1.0);
        let sketch = timeline.add(Feature::Sketch {
            on: ground,
            sketch: drawn.sketch,
        });
        let mut build = Build::default();
        timeline.edit(sketch).opened(&mut build);
        let mut document = Document::new(&mut build, timeline);
        let profile = document
            .models(&build, Some(sketch))
            .at(sketch)
            .expect("a fixture names the sketch it drew")
            .profile(&[0]);
        document.apply(
            &mut build,
            Change::Revolve {
                profile,
                axis: drawn.axis,
                sector,
                operation: Operation::Join,
            },
        );
        Self {
            document,
            build,
            sketch,
        }
    }

    /// What it built, and what the drawing made of it.
    fn models(&self) -> Models<'_> {
        self.document.models(&self.build, Some(self.sketch))
    }
}

/// Two circles far enough apart to miss each other: two regions, and six
/// degrees of freedom — a centre apiece and a radius apiece.
fn two_rings() -> Sketch {
    let mut sketch = Sketch::default();
    for x in [0.0, 5.0] {
        let center = sketch.add_point(DVec2::new(x, 0.0));
        sketch.add_circle(center, 1.0);
    }
    sketch
}

/// Where the plane point `(x, y)` lands in the world.
///
/// Every position an edit names is a world one — a drag says where the cursor
/// took something and a click says where it fell — and the drawings here are
/// written in the flat coordinates a sketch keeps. One conversion, so no test
/// below has to spell out that the ground's own +y runs along world −Z.
fn world(x: f64, y: f64) -> Vec3 {
    Plane::GROUND.point(DVec2::new(x, y)).as_vec3()
}

/// A square of side two with its near corner at `(x, y)`.
fn square_at(x: f64, y: f64) -> Sketch {
    let mut sketch = Sketch::default();
    let corners: Vec<_> = [(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]
        .map(|(u, v)| sketch.add_point(DVec2::new(x + u, y + v)))
        .into();
    for at in 0..corners.len() {
        sketch.add_segment(corners[at], corners[(at + 1) % corners.len()]);
    }
    sketch
}

/// **A second extrude joins the solid the first left standing**, which is what
/// makes a timeline a recipe rather than a pile of prisms.
///
/// Two squares on the ground, overlapping in a one-by-one corner, each carried
/// two up. What comes back is one body, and it is the second step's — the first
/// is the workings.
///
/// **Read off the names its faces carry**, which is the claim this layer owns:
/// twelve of them, six grown by each step, because the union is made of both
/// blocks and every face of both survives some part of itself. A second step
/// that had merely replaced the first would carry six, all its own. How much
/// the union shuts in is the boolean's claim and is asserted where the boolean
/// is — what this adds is that the *document* asks for it, the second step
/// building on what the first left and nothing reaching the kernel except
/// through the timeline.
#[test]
fn a_second_extrude_joins_the_solid_the_first_one_left_standing() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let near = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square_at(0.0, 0.0),
    });
    let far = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square_at(1.0, 1.0),
    });

    let mut build = Build::default();
    timeline.edit(near).opened(&mut build);
    timeline.edit(far).opened(&mut build);
    let profile = |timeline: &Timeline, build: &Build, of| {
        Models::new(timeline, build, Some(of))
            .open()
            .expect("a fixture opens the sketch it names")
            .profile(&[0])
    };
    let one = profile(&timeline, &build, near);
    let two = profile(&timeline, &build, far);
    let first = timeline.add(Feature::Extrude {
        profile: one,
        distance: 2.0,
        operation: Operation::Join,
    });
    let second = timeline.add(Feature::Extrude {
        profile: two,
        distance: 2.0,
        operation: Operation::Join,
    });

    let document = Document::new(&mut build, timeline);
    let models = document.models(&build, Some(near));
    assert_eq!(models.lost(), 0, "a step went wrong");
    assert_eq!(build.bodied(first).built(), Built::Made);
    assert_eq!(build.bodied(second).built(), Built::Made);

    // One body, and it is the second step's: the first is the workings.
    let solids: Vec<_> = models.solids().map(|(at, _)| at).collect();
    assert_eq!(solids, [second], "the union came back in pieces");

    let (_, body) = models.solids().next().expect("the union");
    let names: Vec<_> = body.names().collect();
    let by = |of: FeatureId| names.iter().filter(|it| it.by == of.step()).count();
    assert_eq!(names.len(), 12, "{names:?}");
    assert_eq!(
        by(first),
        6,
        "the first block is not in the union: {names:?}"
    );
    assert_eq!(by(second), 6, "the second block is not in it: {names:?}");
}

/// **A step the kernel will not merge stands beside the model rather than
/// vanishing**, which is the whole of what a refusal costs.
///
/// Two rods driven through each other at a right angle, one drawn on the ground
/// and one on the plane square to it. Their walls are two cylinders *across*
/// each other, which meet in a quartic nothing yet parameterizes — so joining
/// the second onto the first is refused, and what the document shows is both
/// solids side by side: exactly the picture it showed before there were
/// booleans at all. The tree says which step could not be merged, because
/// [`Models::unmerged`] counts a refusal apart from a lost profile.
///
/// Across, and of *unequal* radius, which is the whole of the fixture. Two
/// cylinders whose axes run parallel meet in ruling lines; two of one radius on
/// crossing axes meet in two ellipses; and the kernel writes both of those down
/// and carries them. What is left is the quartic, and this is it.
#[test]
fn a_step_the_kernel_will_not_merge_stands_beside_the_model() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let upright = timeline.add(Feature::Plane(Datum::World(World::Front)));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: ring(0.0, 1.0),
    });
    let round = timeline.add(Feature::Sketch {
        on: upright,
        sketch: ring(0.0, 1.5),
    });

    let mut build = Build::default();
    timeline.edit(drawn).opened(&mut build);
    timeline.edit(round).opened(&mut build);
    let open = |timeline: &Timeline, build: &Build, at| {
        Models::new(timeline, build, Some(at))
            .open()
            .expect("a fixture opens the sketch it names")
            .profile(&[0])
    };
    let one = open(&timeline, &build, drawn);
    let two = open(&timeline, &build, round);
    let first = timeline.add(Feature::Extrude {
        profile: one,
        distance: 2.0,
        operation: Operation::Join,
    });
    let second = timeline.add(Feature::Extrude {
        profile: two,
        distance: 2.0,
        operation: Operation::Join,
    });

    let document = Document::new(&mut build, timeline);
    let models = document.models(&build, Some(drawn));
    assert_eq!(build.bodied(first).built(), Built::Made);
    assert_eq!(
        build.bodied(second).built(),
        Built::Refused,
        "a quartic crossing was carried through",
    );

    // Both on screen, each still its own solid, and each still knowing which
    // step grew it — which is what keeps a face of either pickable.
    let solids: Vec<_> = models.solids().map(|(at, _)| at).collect();
    assert_eq!(
        solids,
        [first, second],
        "a refused step left nothing behind"
    );
    for (at, body) in models.solids() {
        assert!(
            body.names().all(|named| named.by == at.step()),
            "a solid standing alone holds a face another step grew",
        );
    }
    // Reported as what it is. A refusal is not a lost profile — both profiles
    // are intact and still name their rings — and the two are counted apart so
    // the status line can say which happened.
    assert_eq!(models.unmerged(), 1, "the refusal went unreported");
    assert_eq!(models.lost(), 0, "a refusal was counted as a lost profile");
    assert_eq!(models.broken_at(second), Some(Broken::Unmerged));
    assert_eq!(models.broken_at(first), None);
}

/// **A profile holds while the geometry moves, and is lost when the region is
/// cut.**
///
/// The two halves of what naming a region by its boundary is for, and the pair
/// has to be asked together: a name that survived everything would be one that
/// had stopped meaning anything, and a name that survived nothing would be a
/// position by another spelling.
///
/// A drag is the first half because it is what a modeller does all day — every
/// corner of the square moves, the region covers something else afterwards, and
/// it is the same region. Drawing a line across it is the second, and it is the
/// one case where `None` is the answer: neither piece the cut left is bounded by
/// what the whole was, so there is nothing to prefer between them.
#[test]
fn a_profile_holds_through_a_drag_and_is_lost_when_the_region_is_cut() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });

    // Settled before the region is named, because what a drawing encloses is
    // what the solve decides — see `demo::document`, which does the same thing
    // through a raised document.
    let mut build = Build::default();
    timeline.edit(drawn).opened(&mut build);
    let corner = timeline
        .drawn(drawn)
        .sketch()
        .points()
        .next()
        .expect("the square draws four corners")
        .0;
    let profile = Models::new(&timeline, &build, Some(drawn))
        .open()
        .expect("a fixture opens the sketch it names")
        .profile(&[0]);
    let solid = timeline.add(Feature::Extrude {
        profile,
        distance: 1.0,
        operation: Operation::Join,
    });

    let mut document = Document::new(&mut build, timeline);
    // How much the region the extrude is grown from covers, asked through the
    // name rather than by position — which is the whole claim, so it is what
    // every assertion below goes through. Both halves are handed in rather than
    // captured, so the closure holds no borrow across the edits between calls.
    let covered = |document: &Document, build: &Build| {
        let &[at] = build.bodied(solid).regions() else {
            return None;
        };
        let faces = document
            .models(build, Some(drawn))
            .open()
            .expect("a fixture opens the sketch it names")
            .arrangement()
            .faces();
        Some(faces[at].area())
    };
    // Two by two, and the one region the square shuts in.
    assert_eq!(build.bodied(solid).regions(), [0]);
    assert_eq!(covered(&document, &build), Some(4.0));

    // The corner at the origin dragged out to (-1, -1). Nothing here constrains
    // the other three, so that corner is the only one that moves and the square
    // becomes the quadrilateral (-1,-1), (2,0), (2,2), (0,2) — whose shoelace is
    // (2 + 4 + 4 + 2) / 2 = 6.
    document.apply(
        &mut build,
        Change::Drag {
            sketch: drawn,
            grip: Grip::Point(corner),
            to: world(-1.0, -1.0),
        },
    );
    // Within a tolerance, unlike the four above: a drag reaches for the cursor
    // *through* the constraints rather than writing the position, so where it
    // lands is a solve's answer and not arithmetic.
    let after = covered(&document, &build).expect("the drag lost the region");
    assert!(
        (after - 6.0).abs() < 1e-9,
        "the region covers {after} rather than 6, so the drag did something else"
    );
    assert_eq!(
        document.models(&build, Some(drawn)).lost(),
        0,
        "moving the geometry lost the region"
    );
    assert_eq!(build.bodied(solid).built(), Built::Made);

    // **Coming to nothing is not failing.** Carried no distance at all, the
    // extrude builds and encloses nothing — which is what a depth somebody is
    // still typing looks like, so it leaves no solid and is not counted among
    // what went wrong. Then carried back, because the rest of this is about
    // the region rather than the depth.
    document.apply(
        &mut build,
        Change::Carry {
            extrude: solid,
            to: 0.0,
        },
    );
    assert_eq!(build.bodied(solid).built(), Built::Empty);
    let models = document.models(&build, Some(drawn));
    assert_eq!(
        models.lost(),
        0,
        "a depth of nothing was counted as a step that failed"
    );
    // **And a reader listing the steps can say so.** Coming to nothing is not
    // being broken, so the fault reading answers `None` — which left a row that
    // built nothing reading exactly like one that built. What a step *came to*
    // is the wider question, and it is what the recipe words.
    assert_eq!(
        models.broken_at(solid),
        None,
        "an empty step read as broken"
    );
    assert_eq!(
        models.came_at(solid),
        Some(Built::Empty),
        "an empty step read as one that built",
    );
    document.apply(
        &mut build,
        Change::Carry {
            extrude: solid,
            to: 1.0,
        },
    );
    assert_eq!(build.bodied(solid).built(), Built::Made);

    // Now a line straight across it, from outside to outside. Both halves are
    // bounded by some of what the whole was and by the cut besides, so the name
    // fits neither.
    document.apply(
        &mut build,
        Change::AddSegment {
            sketch: drawn,
            from: Anchor::At(world(-2.0, 0.5)),
            to: Anchor::At(world(3.0, 0.5)),
        },
    );
    assert_eq!(
        document
            .models(&build, Some(drawn))
            .open()
            .expect("a fixture opens the sketch it names")
            .arrangement()
            .faces()
            .len(),
        2,
        "the line did not cut the region in two"
    );
    assert!(build.bodied(solid).regions().is_empty());
    assert_eq!(document.models(&build, Some(drawn)).lost(), 1);
    assert_eq!(build.bodied(solid).built(), Built::Lost);
}

/// What a closed document built is gone too.
///
/// The other half of [`Build::reopened`], and the same argument as its
/// neighbour below: everything here is keyed by
/// [`FeatureId`](crate::timeline::FeatureId), and a document read from a file
/// numbers its steps from zero — so an answer left behind would be one about an
/// extrude that no longer exists, filed under the name of one that does.
#[test]
#[should_panic(expected = "this sweep has not been built")]
fn reopening_forgets_what_the_last_document_built() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });

    let mut build = Build::default();
    timeline.edit(drawn).opened(&mut build);
    let profile = Models::new(&timeline, &build, Some(drawn))
        .open()
        .expect("a fixture opens the sketch it names")
        .profile(&[0]);
    let solid = timeline.add(Feature::Extrude {
        profile,
        distance: 1.0,
        operation: Operation::Join,
    });
    let _document = Document::new(&mut build, timeline);
    // Built, so this answers.
    let _ = build.bodied(solid).regions();

    build.reopened();
    let _ = build.bodied(solid).regions();
}

/// Two sketches settle into two answers, and neither overwrites the other.
///
/// The whole of what keying the build by feature buys, and the one failure it
/// is there to prevent: a single shared report would leave whichever sketch was
/// settled last describing both. The four numbers below are hand-checkable and
/// all different, so a slot found by the wrong handle cannot pass by
/// coincidence.
#[test]
fn two_sketches_settle_into_two_answers_that_do_not_overwrite_each_other() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let boxy = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });
    let rings = timeline.add(Feature::Sketch {
        on: ground,
        sketch: two_rings(),
    });

    // **Settled newest first**, which is what catches the filing. A build's
    // answers are searched by halving, so one filed where it was asked for
    // rather than where its handle belongs reads back as another sketch's or as
    // none — and settling in timeline order, which is what raising a document
    // does, would put every entry right by luck.
    let mut build = Build::default();
    timeline.edit(rings).opened(&mut build);
    timeline.edit(boxy).opened(&mut build);

    // Four free corners are eight degrees of freedom, and the square shuts one
    // region in. Two centres and two radii are six, shutting in two.
    assert_eq!(build.settled(boxy).outcome().degrees_of_freedom(), 8);
    assert_eq!(build.settled(boxy).arrangement().faces().len(), 1);
    assert_eq!(build.settled(rings).outcome().degrees_of_freedom(), 6);
    assert_eq!(build.settled(rings).arrangement().faces().len(), 2);

    // Editing one leaves the other's report exactly where it was. The square
    // loses an edge, so it encloses nothing and drops to seven — a point that
    // ends no edge is still free, and one of its two freedoms went with the
    // edge that named it.
    let edge = timeline
        .drawn(boxy)
        .sketch()
        .segments()
        .next()
        .expect("the square draws four edges")
        .0;
    timeline
        .edit(boxy)
        .remove(&mut build, Entity::Segment(edge));

    assert_eq!(build.settled(boxy).arrangement().faces().len(), 0);
    assert_eq!(build.settled(rings).outcome().degrees_of_freedom(), 6);
    assert_eq!(build.settled(rings).arrangement().faces().len(), 2);
}

/// The revision counts every settle, whichever sketch it was about.
///
/// One number for the document rather than one per sketch: what compares it is
/// a picture of the whole of it, so a settle anywhere has to move it or that
/// picture goes unrepainted.
#[test]
fn any_sketch_settling_moves_the_documents_one_revision() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let boxy = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });
    let rings = timeline.add(Feature::Sketch {
        on: ground,
        sketch: two_rings(),
    });

    let mut build = Build::default();
    let fresh = build.revision();
    timeline.edit(boxy).opened(&mut build);
    let after_first = build.revision();
    assert_ne!(after_first, fresh, "settling one sketch went uncounted");

    timeline.edit(rings).opened(&mut build);
    let settled = build.revision();
    assert_ne!(
        settled, after_first,
        "settling the other sketch went uncounted"
    );

    // Opening a document counts as a move of it, and counts *on*. A fresh
    // `Build` would start over at the number this one began at, and a view
    // compares the revision it last drew against this — so a document opened
    // into a reset counter could land on one the view believes it has already
    // drawn and leave the old picture on screen.
    build.reopened();
    assert_ne!(build.revision(), settled, "reopening went uncounted");
    assert_ne!(
        build.revision(),
        fresh,
        "reopening restarted the count, so a view could miss the change"
    );
}

/// What a closed document settled is gone rather than left to be read.
///
/// The half of [`Build::reopened`] a value cannot show. Everything it holds is
/// keyed by [`FeatureId`](crate::timeline::FeatureId), and a document opened
/// from a file numbers its steps from zero like any other — so a report left
/// behind is not stale so much as *wrong*, an answer about a sketch that no
/// longer exists filed under the name of one that does. Settling the new sketch
/// would overwrite it, which is exactly why the reach that has to be caught is
/// the one *before* it is settled: that is the moment a leftover would answer
/// instead of admitting it has nothing to say.
#[test]
#[should_panic(expected = "this sketch has not been settled")]
fn reopening_forgets_what_the_last_document_settled() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let boxy = timeline.add(Feature::Sketch {
        on: ground,
        sketch: square(),
    });

    let mut build = Build::default();
    timeline.edit(boxy).opened(&mut build);
    // Settled, so this answers.
    let _ = build.settled(boxy).outcome();

    build.reopened();
    let _ = build.settled(boxy).outcome();
}

/// A rebuild leaves every extrude findable, whatever order the walk arrived in.
///
/// **The trap the recipe order sets, asserted before anything can spring it.**
/// `modelled` is read by halving it, which an unsorted list answers wrongly
/// rather than slowly — and the walk that fills it is the order the steps are
/// *built* in, which is the order they were taken in only until something moves
/// one. Nothing moves one yet, so this hands the walk over reversed to ask the
/// question a reorder will ask for real.
///
/// Two regions and not one, because a list of one is sorted whatever it holds.
/// Each named through its own profile, so a lookup landing on its neighbour is a
/// wrong region rather than a missing one — and one landing nowhere is the
/// panic a halved search makes of a list that is not in order.
#[test]
fn a_rebuild_files_every_extrude_by_handle_whatever_order_it_walked_them_in() {
    let mut timeline = Timeline::default();
    let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
    let drawn = timeline.add(Feature::Sketch {
        on: ground,
        sketch: two_rings(),
    });
    let mut build = Build::default();
    timeline.edit(drawn).opened(&mut build);

    // A disc apiece, named by what bounds each — so which of the two a lookup
    // answers with is a question with a right answer.
    // Named before any of them is added, because naming one borrows the very
    // timeline the addition writes.
    let profiles = {
        let models = Models::new(&timeline, &build, Some(drawn));
        let open = models.open().expect("a fixture opens the sketch it names");
        [open.profile(&[0]), open.profile(&[1])]
    };
    let grown: Vec<FeatureId> = profiles
        .map(|profile| {
            timeline.add(Feature::Extrude {
                profile,
                distance: 1.0,
                operation: Operation::Join,
            })
        })
        .into();

    // Forwards first, which is what every rebuild does today.
    let walk: Vec<_> = timeline.swept().collect();
    build.rebuild(walk.iter().copied());
    let found: Vec<Vec<usize>> = grown
        .iter()
        .map(|&at| build.bodied(at).regions().to_vec())
        .collect();
    assert_eq!(
        found,
        [vec![0], vec![1]],
        "the two discs are not two regions"
    );

    // And backwards, which is what a reordered recipe will hand it. The same
    // two answers, against the same two handles: what order the walk arrives in
    // is the recipe's business and no part of what an extrude resolves to.
    build.rebuild(walk.iter().rev().copied());
    let reversed: Vec<Vec<usize>> = grown
        .iter()
        .map(|&at| build.bodied(at).regions().to_vec())
        .collect();
    assert_eq!(
        reversed, found,
        "an extrude resolved differently for having been walked later"
    );
}

/// **A circle spun about a line of its own drawing reaches the model as a
/// ring**, which is what `.notes/KERNEL.md` §10's first rule owes M6: until a
/// step of a document can make one, only a test raises a torus.
///
/// The whole path in one call — an intent naming a region and a segment, a
/// durable name minted from the drawing, a step of the timeline, a body built
/// from it and a model to show. What the ring *is* is the kernel's own to
/// prove, and it does: see `a_circle_spun_about_a_line_beside_it_is_the_ring
/// _it_traces`, where the volume is Pappus.
///
/// **And it is not exact**, a torus being the fitted tier's own surface — the
/// one thing here no other solid a document can hold would say. Its edges are
/// exact all the same, every one of them a circle, so nothing about it was
/// marched.
#[test]
fn a_circle_spun_about_a_line_of_its_own_drawing_reaches_the_model_as_a_ring() {
    let whole = Spun::new(Sector::WHOLE);
    let models = whole.models();
    assert_eq!(models.lost(), 0, "the revolve lost its footing");
    let (_, body) = models.solids().next().expect("the revolve raised no solid");
    assert!(!body.exact(), "a ring stands on a torus");
    // One name over the faces a whole turn is cut into, a wall being named by
    // the curve it came off rather than by how the kernel had to cut it.
    assert_eq!(body.names().count(), 1, "the ring is more than one wall");
    assert_eq!(body.strays(), 0.0, "a ring's own edges are all circles");

    // **And a step says how much of a turn**, which is what carrying a sector
    // buys. Asked by the names, which is what a document sees: a whole turn is
    // the one wall, and a quarter is that wall and the two caps a part of a
    // turn has ends to raise.
    let quarter = Spun::new(Sector {
        from: 0.0,
        sweep: FRAC_PI_2,
    });
    let models = quarter.models();
    let (_, body) = models.solids().next().expect("the revolve raised no solid");
    assert_eq!(body.names().count(), 3, "a quarter turn raised no caps");

    // **A turn of nothing comes to nothing, and is not lost.** The two are
    // different answers about a step: a name that stopped fitting is a step
    // with nothing to stand on, where this one stands on a region it still
    // finds and sweeps no space — which is what an extrude of no depth is.
    let none = Spun::new(Sector {
        from: 0.0,
        sweep: 0.0,
    });
    let models = none.models();
    assert_eq!(models.lost(), 0, "a turn of nothing lost its footing");
    assert!(
        models.solids().next().is_none(),
        "a turn of nothing swept a solid",
    );
}
