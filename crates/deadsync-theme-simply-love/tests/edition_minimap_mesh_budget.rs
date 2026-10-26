//! CPU mesh/resource contract; no GPU execution is claimed by this test.
#[path = "../src/screens/edition/minimap_content_render.rs"]
mod minimap_content_render;
use minimap_content_render::Mesh;
#[test]
fn maximum_rectangles_use_exactly_six_vertices_each_and_truncation_is_explicit() {
    for (side, limit) in [(128u32, 16384usize), (256, 65536)] {
        let pixels: Vec<_> = (0..side)
            .flat_map(|y| {
                (0..side).map(move |x| {
                    if (x + y) % 2 == 0 {
                        0xff010101
                    } else {
                        0xff020202
                    }
                })
            })
            .collect();
        {
            let full = Mesh::build(side, side, &pixels, limit).unwrap();
            assert!(full.complete);
            assert_eq!(full.rectangles, limit);
            assert_eq!(full.vertices.len(), limit * 6);
        }
        let partial = Mesh::build(side, side, &pixels, limit - 1).unwrap();
        assert!(!partial.complete);
        assert_eq!(partial.rectangles, limit - 1);
        assert_eq!(partial.vertices.len(), (limit - 1) * 6);
    }
    assert!(Mesh::build(1, 1, &[0xff010101], 65537).is_err());
}
