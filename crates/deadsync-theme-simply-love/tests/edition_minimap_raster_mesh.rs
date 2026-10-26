#[path = "../src/screens/edition/minimap_content_render.rs"]
mod minimap_content_render;
use deadlib_present::actors::Actor;
use minimap_content_render::Mesh;
#[test]
fn runs_merge_into_one_retained_mesh_and_reflect_once() {
    let pixels = vec![0xff112233; 16 * 16];
    let mesh = Mesh::build(16, 16, &pixels, 32).unwrap();
    assert!(mesh.complete);
    assert_eq!(mesh.rectangles, 1);
    assert_eq!(mesh.vertices.len(), 6);
    match (
        mesh.actor(10.0, 20.0, false, 1),
        mesh.actor(10.0, 20.0, false, 1),
    ) {
        (Actor::Mesh { vertices: a, .. }, Actor::Mesh { vertices: b, .. }) => {
            assert!(std::sync::Arc::ptr_eq(&a, &b))
        }
        _ => panic!(),
    }
    match mesh.actor(10.0, 20.0, true, 1) {
        Actor::Mesh { vertices, .. } => {
            assert_eq!(vertices[0].pos, [0.0, 16.0]);
            assert_eq!(vertices[2].pos, [16.0, 0.0]);
        }
        _ => panic!(),
    }
}
#[test]
fn checkerboard_budget_is_explicit_and_invalid_dimensions_rejected() {
    let pixels: Vec<_> = (0..16)
        .flat_map(|y| {
            (0..16).map(move |x| {
                if (x + y) % 2 == 0 {
                    0xff000001
                } else {
                    0xff000002
                }
            })
        })
        .collect();
    let partial = Mesh::build(16, 16, &pixels, 10).unwrap();
    assert!(!partial.complete);
    assert_eq!(partial.rectangles, 10);
    assert_eq!(partial.vertices.len(), 60);
    let full = Mesh::build(16, 16, &pixels, 256).unwrap();
    assert!(full.complete);
    assert_eq!(full.rectangles, 256);
    assert!(Mesh::build(16, 16, &pixels[..255], 256).is_err());
    assert!(Mesh::build(0, 16, &[], 1).is_err());
}
