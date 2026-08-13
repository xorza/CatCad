use super::*;
use crate::demo;
use aperture::Projection;

/// Raising a document lays out everything it holds: the solids as they were
/// given, and the drawing as the strokes, rims and markers it turns into.
///
/// What this pins is that the scene is *derived* — nothing is drawn that the
/// document does not say, which is the whole reason saving one is enough.
#[test]
fn raising_a_document_lays_out_its_solids_and_its_drawing() {
    let mut document = demo::document();
    let scene = document.raise();

    // The slab and the three boxes standing on it.
    assert_eq!(scene.objects.len(), 4);
    // Seven segments — four sides, the rail, and the arm's two bars — two
    // circles, and a marker on each of the nine points.
    assert_eq!(scene.curves.len(), 7);
    assert_eq!(scene.rings.len(), 2);
    assert_eq!(scene.points.len(), 9);
    assert_eq!(scene.camera, document.camera());

    // Raising it again says the same thing: the document is unchanged by being
    // looked at, which is what makes it the thing worth saving.
    let again = document.raise();
    assert_eq!(again.objects.len(), scene.objects.len());
    assert_eq!(again.points.len(), scene.points.len());
    assert_eq!(again.camera, scene.camera);
}

/// A document opens looking at itself rather than at wherever the default
/// camera happened to point.
#[test]
fn framing_a_document_points_its_camera_at_what_it_draws() {
    let mut document = demo::document();
    let scene = document.raise();
    let unframed = document.camera();

    document.frame(&scene);

    let framed = document.camera();
    assert_ne!(framed, unframed, "framing left the camera where it was");
    // It is aimed at the middle of what it holds, which for the demo is the
    // slab: twelve wide and nine deep, centred on the sketch's own origin.
    let bounds = scene.bounds().expect("the demo draws something");
    assert!(
        framed.target.abs_diff_eq(bounds.centre(), 1e-4),
        "aimed at {:?} rather than the middle of {:?}",
        framed.target,
        bounds
    );
    // And far enough back to take it all in.
    assert!(
        framed.distance > bounds.radius(),
        "{framed:?} vs {bounds:?}"
    );
}

/// The camera the document holds is the one the renderer paints through.
///
/// The round trip the projection toggle makes: the overlay reads the document,
/// writes back what was asked for, and the frame has to be drawn through it.
/// Handing the renderer a copy every frame is what closes that loop, and a copy
/// that were only refreshed on change is what would leave it open.
#[test]
fn the_renderer_is_aimed_through_the_documents_own_camera() {
    let mut document = demo::document();
    let scene = document.raise();
    document.frame(&scene);
    let mut renderer = Renderer::new(scene);

    document.aim(&mut renderer);
    assert_eq!(*renderer.camera(), document.camera());

    // Turn the camera the way a gesture would, and the renderer follows.
    document.camera_mut().orbit(0.4, 0.2);
    let turned = document.camera();
    assert_ne!(*renderer.camera(), turned, "nothing to prove otherwise");
    document.aim(&mut renderer);
    assert_eq!(*renderer.camera(), turned);

    // The projection rides along with it, which is the toggle's whole path.
    let was = document.camera().projection;
    document.camera_mut().projection = was.toggled();
    document.aim(&mut renderer);
    assert_eq!(renderer.camera().projection, was.toggled());
    assert_ne!(renderer.camera().projection, was);
    assert!(matches!(
        renderer.camera().projection,
        Projection::Orthographic | Projection::Perspective
    ));
}
