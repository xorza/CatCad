use super::*;
use crate::demo;
use silverpoint::Solver;

/// Syncing a scene puts everything the document holds into it: the solids as
/// they were given, and the drawing as the strokes, rims and markers it turns
/// into.
///
/// What this pins is that the scene is *derived* — nothing is drawn that the
/// document does not say, which is the whole reason saving one is enough.
#[test]
fn a_document_syncs_a_scene_to_its_solids_and_its_drawing() {
    let document = demo::document(&mut Solver::default());
    let mut scene = Scene::default();
    let mut names = Names::default();
    document.sync(&mut scene, &mut names);

    // The slab and the three boxes standing on it.
    assert_eq!(scene.objects.len(), 4);
    // Seven segments — four sides, the rail, and the arm's two bars — two
    // circles, and a marker on each of the nine points.
    assert_eq!(scene.curves.len(), 7);
    assert_eq!(scene.rings.len(), 2);
    assert_eq!(scene.points.len(), 9);

    // Every drawn part is named, and named as something the drawing holds: the
    // tags the scene carries are indices into what came back.
    for point in &scene.points {
        let tag = point.tag.expect("a marker is drawn to be picked");
        assert!(names.get(tag).is_some(), "{tag:?} names nothing");
    }

    // Syncing the same scene again says the same thing rather than adding to
    // it — the document is unchanged by being looked at, and looking twice
    // leaves one drawing rather than two on top of each other.
    document.sync(&mut scene, &mut names);
    assert_eq!(scene.objects.len(), 4);
    assert_eq!(scene.curves.len(), 7);
    assert_eq!(scene.rings.len(), 2);
    assert_eq!(scene.points.len(), 9);
}
