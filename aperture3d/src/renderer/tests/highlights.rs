//! What a highlight does to what it names, and to what it does not.

use crate::curve::Curve;
use crate::highlight::Highlight;
use crate::mesh::Mesh;
use crate::object::Object;
use crate::point::Point;
use crate::renderer::pane::{Pane, Placement};
use crate::renderer::tests::harness::facing_quad;
use crate::renderer::*;
use crate::ring::Ring;
use crate::scene::Scene;
use crate::styled::Styled;
use crate::tag::Tag;
use glam::Vec3;

/// A lift brightens what an object is drawn in, and an ink replaces it.
///
/// The distinction a control depends on. A datum's axes say *which axis they
/// are* by their colour, so a hover cannot spend it — where for a sketch entity
/// spending it is the entire point, since what a selection means is that
/// everything in it reads alike.
///
/// Asked of the flattened vertices rather than of [`Tint::over`], which is what
/// makes it a test of anything: the question is whether the mesh path reaches
/// for the tint at all, and a flatten that wrote the object's colour straight
/// through would pass every test of the tint by itself.
#[test]
fn a_lifted_highlight_brightens_what_an_object_is_drawn_in_and_an_inked_one_replaces_it() {
    const OWN: Vec3 = Vec3::new(0.4, 0.1, 0.2);
    /// Ink sharing no channel with [`OWN`] and with none at zero, so a flatten
    /// that multiplied the two where it should replace cannot come out right by
    /// accident.
    const INK: Vec3 = Vec3::new(1.0, 0.8, 0.2);

    let mut scene = Scene::default();
    scene
        .solids
        .push(Object::new(facing_quad()).colored(OWN).tagged(Tag::new(1)));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));

    // Unlit first, so what the lift is measured against is what the flatten
    // actually wrote rather than what the object was built with.
    renderer.refresh(1.0);
    let flat = |renderer: &Renderer| renderer.mirrors[0].cpu.solids.vertices[0].color;
    assert_eq!(flat(&renderer), OWN.to_array());

    // Twice as bright, channel for channel, so the hue is untouched and only
    // the brightness moved. Doubling is exact in binary, so this is an equality
    // rather than a tolerance.
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(1),
            look: Highlight::lifted(2.0),
        },
    );
    renderer.refresh(1.0);
    assert_eq!(
        flat(&renderer),
        [0.8, 0.2, 0.4],
        "a lift recoloured the object instead of brightening it"
    );

    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(1),
            look: Highlight::new(INK),
        },
    );
    renderer.refresh(1.0);
    assert_eq!(
        flat(&renderer),
        INK.to_array(),
        "an ink was blended with what it should have replaced"
    );
}

/// A highlight doubles the primitive it names — same geometry, different look
/// — and touches nothing else.
#[test]
fn a_highlight_repeats_only_what_its_tag_names() {
    let mut scene = Scene::default();
    scene.curves.push(
        Curve::new(vec![Vec3::ZERO, Vec3::X, Vec3::Y])
            .width(2.0)
            .tagged(Tag::new(1)),
    );
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::Z).tagged(Tag::new(2)));
    scene.rings.push(
        Ring::new(Vec3::ZERO, 1.0, Vec3::Y)
            .width(3.0)
            .tagged(Tag::new(1)),
    );
    scene.points.push(Point::new(Vec3::X).tagged(Tag::new(2)));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));

    // Nothing named, nothing doubled.
    renderer.refresh(1.0);
    let cpu = &renderer.mirrors[0].cpu;
    assert!(cpu.curves.lit.is_empty() && cpu.rings.lit.is_empty() && cpu.points.lit.is_empty());

    let look = Highlight::new(Vec3::new(1.0, 0.0, 0.0)).scale(3.0);
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(1),
            look,
        },
    );
    renderer.refresh(1.0);
    let cpu = &renderer.mirrors[0].cpu;

    // Tag 1 is the three-point curve and the ring: two segments and one rim.
    // The curve tagged 2 and the marker tagged 2 are left alone.
    assert_eq!(cpu.curves.lit.len(), 2);
    assert_eq!(cpu.rings.lit.len(), 1);
    assert!(cpu.points.lit.is_empty());

    // The look replaces the colour, multiplies the width, and adds to the
    assert!(
        cpu.curves
            .lit
            .iter()
            .all(|i| i.paint.color == [1.0, 0.0, 0.0])
    );
    assert!(cpu.curves.lit.iter().all(|i| i.paint.spread == 3.0)); // 2.0/2 × 3
    assert_eq!(cpu.rings.lit[0].paint.spread, 4.5); // 3.0/2 × 3

    // The geometry is the primitive's own, untouched. Copied out first: the
    // records are held on the renderer now, so flattening another one needs
    // it back.
    let doubled = cpu.curves.lit[0];
    renderer.refresh(1.0);
    let plain = &renderer.mirrors[0].cpu.curves.ordinary;
    assert_eq!(doubled.start, plain[0].start);
    assert_eq!(doubled.end, plain[0].end);

    // Naming a tag again replaces its look rather than stacking a second one,
    // so a hover reads over a selection and both still draw once.
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(1),
            look: Highlight::new(Vec3::Y).scale(1.0),
        },
    );
    renderer.refresh(1.0);
    let cpu = &renderer.mirrors[0].cpu;
    assert_eq!(cpu.curves.lit.len(), 2, "still doubled once, not twice");
    assert_eq!(cpu.rings.lit[0].paint.spread, 1.5);
    assert_eq!(cpu.rings.lit[0].paint.color, [0.0, 1.0, 0.0]);

    // Lighting one thing alone drops the rest, and clearing drops everything.
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(2),
            look,
        },
    );
    renderer.refresh(1.0);
    let cpu = &renderer.mirrors[0].cpu;
    assert!(cpu.curves.lit.len() == 1 && cpu.points.lit.len() == 1 && cpu.rings.lit.is_empty());
    renderer.highlight_all(0, &[]);
    renderer.refresh(1.0);
    let cpu = &renderer.mirrors[0].cpu;
    assert!(cpu.curves.lit.is_empty() && cpu.rings.lit.is_empty() && cpu.points.lit.is_empty());
}

/// Re-asking for a look already in force leaves the records alone, which is
/// what lets a caller drive highlighting straight off a pointer that is not
/// moving.
#[test]
fn re_lighting_what_is_already_lit_dirties_nothing() {
    let mut scene = Scene::default();
    scene
        .curves
        .push(Curve::segment(Vec3::ZERO, Vec3::X).tagged(Tag::new(1)));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));
    let lit = Lit {
        tag: Tag::new(1),
        look: Highlight::new(Vec3::Y),
    };

    // `new` starts everything outstanding, so the flag says nothing until it
    // has been cleared once.
    renderer.mirrors[0].relight = false;
    renderer.highlight_only(0, lit);
    assert!(renderer.mirrors[0].relight, "the first look is a change");

    renderer.mirrors[0].relight = false;
    renderer.highlight_only(0, lit);
    renderer.highlight_only(0, lit);
    assert!(
        !renderer.mirrors[0].relight,
        "neither call changed anything"
    );

    // A different look for the same tag is a change, and so is dropping it.
    renderer.highlight_only(
        0,
        Lit {
            look: Highlight::new(Vec3::X),
            ..lit
        },
    );
    assert!(renderer.mirrors[0].relight);
    renderer.mirrors[0].relight = false;
    renderer.highlight_all(0, &[]);
    assert!(renderer.mirrors[0].relight);

    // And clearing what is already clear is the same nothing: a pointer over
    // empty space says so every frame it does not move.
    renderer.mirrors[0].relight = false;
    renderer.highlight_all(0, &[]);
    assert!(!renderer.mirrors[0].relight, "nothing was lit to drop");
}

/// A highlighted mesh is written in the colour it was given, where it stands.
///
/// The one kind that cannot be *doubled*: an overlay grows and rides forward of
/// what it repeats, and a mesh has neither a width nor a bias to do it with. So
/// a highlight reaches it by changing the colour flattened into its vertices,
/// which means it has to be re-flattened when the highlights move and not only
/// when the geometry does.
#[test]
fn a_highlighted_mesh_is_recoloured_where_it_stands() {
    let plain = Vec3::new(0.2, 0.3, 0.4);
    let mut scene = Scene::default();
    scene.faces.push(
        Object::new(Mesh::cube(1.0))
            .colored(plain)
            .tagged(Tag::new(1)),
    );
    // Untagged, so it is scenery and never lit however the highlights move.
    scene
        .faces
        .push(Object::new(Mesh::cube(1.0)).colored(plain));
    let mut renderer = Renderer::new(Pane::new(scene, Placement::Fill));

    renderer.refresh(1.0);
    let corners = renderer.mirrors[0].cpu.faces.vertices.len();
    assert!(corners > 0, "the faces were never flattened");
    assert!(
        renderer.mirrors[0]
            .cpu
            .faces
            .vertices
            .iter()
            .all(|v| v.color == plain.to_array()),
        "a mesh was lit before anything named it"
    );

    // Named, and re-flattened though not one vertex moved — which is the whole
    // of what this pins. A `refresh` that only watched the batch's own mark
    // would leave the colour where it was.
    let look = Highlight::new(Vec3::new(1.0, 0.0, 0.0)).scale(3.0);
    renderer.highlight_only(
        0,
        Lit {
            tag: Tag::new(1),
            look,
        },
    );
    renderer.refresh(1.0);

    let lit: Vec<[f32; 3]> = renderer.mirrors[0]
        .cpu
        .faces
        .vertices
        .iter()
        .map(|v| v.color)
        .collect();
    assert_eq!(lit.len(), corners, "the flatten dropped geometry");
    // Half the corners are the named cube and half the untagged one, and only
    // the first half moved: `scale` has nothing to act on here, so
    // the colour is the whole of what a highlight does to a mesh.
    let (named, scenery) = lit.split_at(corners / 2);
    assert!(
        named.iter().all(|&color| color == [1.0, 0.0, 0.0]),
        "the named mesh kept its own colour"
    );
    assert!(
        scenery.iter().all(|&color| color == plain.to_array()),
        "an untagged mesh was lit"
    );

    // And it goes back when the highlight does, rather than staying lit for
    // the rest of the run.
    renderer.highlight_all(0, &[]);
    renderer.refresh(1.0);
    assert!(
        renderer.mirrors[0]
            .cpu
            .faces
            .vertices
            .iter()
            .all(|v| v.color == plain.to_array()),
        "a mesh stayed lit after nothing named it"
    );
}
