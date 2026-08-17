//! What a click on the drawing means.
//!
//! **Nothing here is the view's.** What a click comes to is decided out of what
//! the pointer found, what is in hand and what the drawing holds; the view is
//! only what happened to be holding the press. So none of this reads a field of
//! anything — it is handed what the frame resolved and answers in
//! [`Intent`](crate::intent::Intent)s, which is the same shape every other
//! reading of a gesture takes.

use glam::Vec3;
use silverpoint::Entity;

use crate::document::Document;
use crate::drawing::anchor::Anchor;
use crate::intent::{Change, Choice, Intents, Opening};
use crate::part::Part;
use crate::session::Session;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use crate::tool::dimensioning::Dimensioning;

/// What one click of the left button found, as everything deciding what it
/// means reads it.
///
/// The pointer's half of a click and nothing else: what is in hand and which
/// sketch is open are the session's, and the drawing under it is the document's,
/// so a click carries only what the *response* said and what the scene answered
/// about it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Click {
    /// Whether this was the second press of a double, which is a click of its
    /// own — see [`clicked`].
    pub(super) double: bool,
    /// Whether shift was held, which adds to what is picked out where a plain
    /// click starts over.
    pub(super) adding: bool,
    /// What it landed on, or `None` where it landed on nothing the layout names.
    pub(super) under: Option<Part>,
    /// Where it landed on the open sketch's plane, and `None` on a plane seen
    /// edge-on — the frame's one ray, resolved at the top of
    /// [`SceneView::poll`](crate::scene_view::SceneView).
    pub(super) at: Option<Vec3>,
}

/// What a click on the drawing asks for.
///
/// A tool in hand takes every click, whatever it landed on: what was clicked is
/// what the new geometry is *held to*, so a click on the drawing is worth more
/// to a tool than one beside it. Nothing is picked out by a click a tool took —
/// selecting is the pointer's, and a tool that placed a point and picked it out
/// would be arguing with the hand that placed it.
///
/// The dimension tool is the exception, and the difference is what it clicks
/// *for*: its picks are the whole of what it has done until the second one
/// lands, so they are what a reader has to be shown. See its arm below.
pub(super) fn clicked(click: Click, document: &Document, session: &Session, intents: &mut Intents) {
    let sketch = session.editing();
    let drawing = document.drawing_at(sketch);
    let Click {
        double,
        adding,
        under,
        at,
    } = click;
    // A second click on a dimension opens it for typing. Raised before the arms
    // below rather than instead of them, because the click is still an ordinary
    // click: it picks the dimension out, which is what makes the constraint bar
    // agree with the field about what is being worked on. Palantir reports the
    // second press of a double as a click of its own, so the first one already
    // did the picking.
    //
    // A dimension and nothing else. Every other part has no number to type, and
    // a double-click on one should mean whatever a double-click comes to mean
    // next rather than nothing-in-particular now.
    if double && let Some(typed) = under.and_then(|part| dimension(part, document, sketch)) {
        intents.push(Choice::Ask(Some(typed)));
    } else {
        // Any other click puts away whatever was open. Committing would be the
        // other reading, and it is the wrong one: a click that landed somewhere
        // else was about somewhere else, and a number half-typed should not be
        // written to the document by a gesture that never mentioned it.
        intents.push(Choice::Ask(None));
    }
    match (session.tool(), anchor(at, sketch, under)) {
        // One click. On a point already there it adds nothing, and the drawing
        // comes out of it unchanged.
        (Tool::Point, Some(at)) => {
            intents.push(Change::AddPoint { sketch, at });
        }
        // Two clicks each. The first is remembered in the tool and reaches the
        // document not at all; the second commits the whole shape as one step.
        (Tool::Line { from: None }, Some(start)) => {
            intents.push(Choice::Hold(Tool::Line { from: Some(start) }));
        }
        (Tool::Line { from: Some(from) }, Some(to)) => {
            intents.push(Change::AddSegment { sketch, from, to });
            intents.push(Choice::Hold(Tool::Line { from: None }));
        }
        (Tool::Circle { center: None }, Some(middle)) => {
            intents.push(Choice::Hold(Tool::Circle {
                center: Some(middle),
            }));
            // The radius can be typed as well as clicked, so the form stands
            // from the moment there is a centre to measure one from. Both ways
            // finish the circle; whichever is used first is the one that does.
            intents.push(Choice::Ask(Some(Opening::Circle {
                sketch,
                center: middle,
            })));
        }
        (
            Tool::Circle {
                center: Some(center),
            },
            Some(rim),
        ) => {
            intents.push(Change::AddCircle {
                sketch,
                center,
                rim,
            });
            intents.push(Choice::Hold(Tool::Circle { center: None }));
            // The click answered what the form was asking, so the form has
            // nothing left to ask.
            intents.push(Choice::Ask(None));
        }
        // The one tool whose clicks name geometry rather than a place, so what
        // it reads is what was *under* the cursor rather than what an anchor
        // made of it. Before the catch-all below, which would otherwise take the
        // click on a plane seen edge-on and pick something out with it.
        (Tool::Dimension(dimensioning), _) => {
            // Placing takes the click outright: what it commits is what the
            // preview has been showing, wherever that click landed. Clicking
            // geometry to *finish* a dimension is how the gesture ends
            // everywhere else, and dropping the number on the thing it measures
            // is a fair place to want it.
            let placed = at
                .map(|at| drawing.plane().flatten(at.as_dvec3()))
                .and_then(|at| dimensioning.proposed(drawing.sketch(), at));
            match placed {
                Some(constraint) => {
                    intents.push(Change::Constrain { sketch, constraint });
                    // Ready for another, which is what a modeller expects of a
                    // tool it took a trip to the bar to pick up.
                    intents.push(Choice::Hold(Tool::Dimension(Dimensioning::Empty)));
                    // And holding nothing, because what was picked has been
                    // said: a selection left standing would offer the bar a
                    // relation over geometry the user has finished with.
                    intents.push(Choice::Select(None));
                }
                // Still picking. A click on nothing, or on something the pair
                // cannot be measured against, leaves what has been picked where
                // it is.
                None => {
                    if let Some(part) = under.filter(|part| part.sketch() == Some(sketch))
                        && let Some(entity) = part.entity()
                        && let Some(next) = dimensioning.picked(drawing.sketch(), entity)
                    {
                        intents.push(Choice::Hold(Tool::Dimension(next)));
                        // **Picked out as well as picked up**, which is the one
                        // place a tool's click selects anything. Every other
                        // tool *places* geometry, and what it has done is on the
                        // screen the moment it does it; this one says something
                        // about geometry already there, and between the first
                        // click and the second there is nothing else to see. A
                        // selection is what the drawing already has for "these
                        // are the ones", lights and all.
                        intents.push(match dimensioning {
                            Dimensioning::Empty => Choice::Select(Some(part)),
                            _ => Choice::Include(part),
                        });
                    }
                }
            }
        }
        // Nothing in hand — or a plane seen so nearly edge-on that a click names
        // nowhere on it, where there is nothing to build from and picking out
        // what was clicked is all that is left.
        (Tool::Pointer, _) | (_, None) => {
            match under {
                // Shift adds to what is picked out.
                Some(entity) if adding => intents.push(Choice::Include(entity)),
                // A shift-click on empty space adds nothing, and clearing is the
                // plain click's business.
                None if adding => {}
                // A plain click starts over with whatever is under the cursor —
                // which is nothing, when it is over nothing.
                _ => intents.push(Choice::Select(under)),
            }
        }
    }
}

/// What a click landing `at` would build on: what it landed on and where.
///
/// The one rule the drawing tools share, and it is that a click on something
/// already drawn is worth *more* than one beside it. A point is shared
/// outright; an edge or a rim is something the new geometry is held to by a
/// constraint, so it stays there however either is dragged afterwards; and bare
/// plane is the only click that leaves anything free.
///
/// A constraint is the one thing that can be under the cursor and offer nothing
/// to build on. It is a statement about geometry rather than a place on the
/// drawing, so a click that lands on one is a click on the plane behind it —
/// which is what leaves it free to be *selected* while a tool is down without
/// the tool treating it as somewhere to put a point.
///
/// `None` only where the plane cannot be resolved at all — seen edge-on, there
/// is nowhere on it for a click to mean.
///
/// Both of its answers are handed in rather than asked for, because the caller
/// has already asked both. What a click picks out and what it builds on are the
/// same question about the same pixel; where it landed is the frame's one ray,
/// resolved once at the top of
/// [`SceneView::poll`](crate::scene_view::SceneView).
fn anchor(at: Option<Vec3>, editing: FeatureId, under: Option<Part>) -> Option<Anchor> {
    // Only what the open sketch holds is something to build *on*: a point of
    // another names a handle this sketch would read as one of its own, so
    // anything else is a click on the bare plane behind it.
    let under = under.filter(|part| part.sketch() == Some(editing));
    match under.and_then(Part::entity) {
        Some(Entity::Point(id)) => Some(Anchor::On(id)),
        Some(Entity::Segment(segment)) => at.map(|at| Anchor::OnSegment { segment, at }),
        Some(Entity::Circle(circle)) => at.map(|at| Anchor::OnCircle { circle, at }),
        // A constraint is a statement rather than a place, and a face is what
        // the curves enclose rather than one of them — so a click on either
        // builds on the bare plane behind it.
        Some(Entity::Constraint(_)) | None => at.map(Anchor::At),
    }
}

/// `part` as a dimension to open for typing, or `None` where it is not one.
///
/// A dimension is a constraint that *measures* something — a length, a radius,
/// an angle. The rest state a relation and carry no number, so there is nothing
/// to type into one, and that is what the value being `Some` answers.
///
/// Read by a press as well as by a click, from the other side: what a number
/// offers to be *dragged* by is
/// [`label`](crate::scene_view::gesture::label)'s, and the two walk the same
/// constraints — which is why one test asks both.
pub(super) fn dimension(part: Part, document: &Document, sketch: FeatureId) -> Option<Opening> {
    let Some(Entity::Constraint(id)) = part.entity() else {
        return None;
    };
    // Off the sketch the part names rather than the one open, which are the
    // same on the frame a click lands and need not stay so.
    let at = part.sketch().unwrap_or(sketch);
    let from = document.drawing_at(at).sketch().constraint(id).value()?;
    Some(Opening::Dimension { part, from })
}
