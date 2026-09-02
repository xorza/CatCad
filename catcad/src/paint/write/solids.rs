//! Every solid of the model, as the renderer holds one.

use aperture::{Batch, Object, Precedence, Vertex};
use glam::Mat4;
use silverpoint::{Body, Named};

use crate::look::Theme;
use crate::model::models::Models;
use crate::paint::growing::{Deciding, Growing, Raising, UNTAKEN};
use crate::paint::layout::Sheets;
use crate::paint::names::Names;
use crate::part::Part;

use crate::paint::write::remesh;

/// The two batches a body can be written into.
///
/// A bundle on the terms [`Raising`] states: both are lent for the length of
/// one call, and two `&mut`s in a row at the call site would be two chances to
/// hand over the wrong one. Which of them a body goes in is what that body
/// *is* — see [`write`](fn@write), where that is decided.
#[derive(Debug)]
pub(crate) struct Shaping<'a> {
    /// The model, and an answer that already holds it.
    pub(crate) solid: &'a mut Batch<Object>,
    /// A proposal standing inside the model, drawn through it rather than
    /// hidden by it.
    pub(crate) ghost: &'a mut Batch<Object>,
}

/// An object per face of every solid the document has grown.
///
/// One object per *face* rather than one per solid, which is what makes a solid
/// something you can point at: a tag names a primitive, so a face that is to be
/// hovered, picked out and later built on has to be a primitive of its own.
///
/// Named by what each face was grown from — see [`Grown`](silverpoint::Grown) —
/// rather than by where
/// it fell in this frame's list. That is the same durable vocabulary the region
/// underneath was named in, so a selection survives the drawing moving under it
/// exactly as a sketch entity's does.
///
/// Modelled rather than drawn, so unlike everything else here there is no
/// appearance to decide beyond the one colour: what a solid *is* is the shape,
/// and shading it is the renderer's.
pub(crate) fn write(
    models: Models<'_>,
    theme: &Theme,
    names: &mut Names,
    sheets: &mut Sheets,
    growing: Option<Growing>,
    sagitta: f64,
    shaping: Shaping<'_>,
) {
    let Shaping { solid, ghost } = shaping;
    let Sheets {
        mesher,
        patch,
        deciding,
        builder,
        putting,
        raised,
        regions,
        ..
    } = sheets;
    // Worked out here rather than borrowed off the document, unlike every solid
    // there is a step for: the one being decided has no step to be held
    // against. See [`Growing::body`](crate::paint::growing::Growing).
    let showing = growing.map_or(Deciding::Nothing, |growing| {
        let raising = Raising {
            builder,
            putting,
            raised,
            regions,
        };
        growing.body(models, raising, deciding)
    });
    // **The answer stands in for the document's own solids**, because it
    // already holds them: drawing both would put the model on screen twice,
    // two copies of one surface fighting for one depth. Every other answer
    // leaves the document drawn as it always is.
    let standing = (showing != Deciding::Answer).then(|| models.solids());
    // **Which batch the one being decided goes in is what it *is*.** An answer
    // holds the model and stands where the model stands, so it is a solid. A
    // tool standing beside it is a proposal about material that is not there
    // yet — and it sits *inside* the part, whether because it is the cut it
    // would make or because a frame had no time to combine it. Drawn as a
    // solid it would be hidden by the very part it is about. See
    // [`Scene::ghosts`](aperture::Scene).
    let (answered, faintly) = match showing {
        Deciding::Answer => (Some(&*deciding), None),
        Deciding::Beside => (None, Some(&*deciding)),
        Deciding::Nothing => (None, None),
    };
    // One walk and nothing gathered: a body hands out its faces as an iterator,
    // so the whole of a document's solids is written straight into the batch. A
    // list of them first would be an allocation a frame, which is exactly what
    // a rubber band's redraw would pay every frame it lasts. The one being
    // decided is chained on rather than pushed after, so a depth typed a digit
    // at a time rewrites the batch it is already in — see `Batch::refill`.
    // Last, so the tags of everything the document holds come out the same
    // whether or not a form is open.
    let faces = standing
        .into_iter()
        .flatten()
        .map(|(_, body)| body)
        .chain(answered)
        .flat_map(per_face);
    let mut shape = |object: &mut Object, (body, face): (&Body, Named)| {
        mesher.cut(body, face, sagitta, patch);
        remesh(
            &mut object.mesh,
            patch
                .corners
                .iter()
                .zip(&patch.normals)
                .map(|(&corner, &normal)| Vertex {
                    position: corner.as_vec3(),
                    normal: normal.as_vec3(),
                }),
            &patch.triangles,
        );
        object.transform = Mat4::IDENTITY;
        object.color = theme.geometry.solid;
        object.precedence = Precedence::Shaped;
        // **A face carries the step that grew it, so the tag follows the
        // name.** What is being decided has no step — see [`UNTAKEN`] — so it
        // cannot be hovered, picked out or built on, there being nothing yet
        // to name; what is grabbable is the arrow carrying it, which is a
        // control rather than the solid. Every face the model brought through
        // the boolean keeps the tag it always had, so a form open on a depth
        // does not take the rest of the part out of reach.
        object.tag = (face.by != UNTAKEN).then(|| {
            names.tag(Part::Solid {
                of: face.by.into(),
                face: face.grown,
            })
        });
    };
    solid.refill(faces, &mut shape);
    // Refilled whether there is a ghost or not, so a form closing takes the
    // last one away rather than leaving it standing over the model.
    ghost.refill(faintly.into_iter().flat_map(per_face), &mut shape);
}

/// Every face of `body`, each carrying the body it was read off.
///
/// A face knows what grew it and nothing about what it is part of, and cutting
/// one wants both — so the pair is made here rather than at each of the two
/// walks that wants it.
fn per_face(body: &Body) -> impl Iterator<Item = (&Body, Named)> {
    body.names().map(move |face| (body, face))
}
