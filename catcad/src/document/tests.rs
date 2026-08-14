use crate::demo;
use aperture::Scene;
use silverpoint::Solver;

/// A document puts the solids it holds into a batch, and says nothing else
/// about what a scene should contain.
///
/// What this pins is that the scene is *derived* — nothing stands in it that the
/// document does not hold, which is the whole reason saving one is enough. What
/// the document's *drawing* turns into is `paint`'s, and tested there.
#[test]
fn a_document_writes_the_solids_it_holds_and_nothing_else() {
    let document = demo::document(&mut Solver::default());
    let mut scene = Scene::default();
    document.write_solids(&mut scene.objects);

    // The slab and the three boxes standing on it.
    assert_eq!(scene.objects.len(), 4);
    // And nothing was drawn: a document describes what it holds, and leaves
    // what that looks like to whoever laid it out.
    assert!(scene.curves.is_empty());
    assert!(scene.rings.is_empty());
    assert!(scene.points.is_empty());

    // Written again into the same batch, it says the same thing rather than
    // adding to it — the document is unchanged by being looked at, and looking
    // twice leaves one set of solids rather than two on top of each other.
    document.write_solids(&mut scene.objects);
    assert_eq!(scene.objects.len(), 4);
}
