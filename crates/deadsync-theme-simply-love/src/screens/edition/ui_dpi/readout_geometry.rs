//! Test-only card measurements use the actual renderer composition boundary.
//! resolve_sprite_size_like_sm supplies Solid's unit native dimensions for
//! Px(0)/Px(0); explicit two-axis sizes are already final and ignore scale.
use deadlib_present::{
    actors::{Actor, SpriteSource},
    compose,
    font::FontMap,
    space::Metrics,
};
pub(super) fn composed_solid_size(actor: &Actor, metrics: &Metrics) -> [f32; 2] {
    // Solid excludes texture-native dimensions and RenderTarget native_size
    // overrides. Do not silently use this helper for an image-backed modal.
    assert!(
        matches!(
            actor,
            Actor::Sprite {
                source: SpriteSource::Solid,
                ..
            }
        ),
        "readout card must be an actual Solid quad"
    );
    let frame = compose::build_screen(
        std::slice::from_ref(actor),
        [0.; 4],
        metrics,
        &FontMap::default(),
        0.,
    );
    assert_eq!(
        frame.sprite_instances.len(),
        1,
        "card must compose to exactly one visible quad"
    );
    frame.sprite_instances[0].size
}
#[test]
fn card_measurement_uses_composer_unit_default_and_builder_sizes_without_double_scale() {
    let metrics = Metrics::centered(854., 480.);
    let default = crate::act!(quad:xy(427.,240.):zoomto(760.,456.));
    assert_eq!(composed_solid_size(&default, &metrics), [760., 456.]);
    let explicit = crate::act!(quad:xy(427.,240.):setsize(100.,50.):zoom(2.));
    // SpriteBuilder folds zoom into explicit size before composition
    // (dsl.rs::build_with_native_dims); the composer must not multiply again.
    assert_eq!(composed_solid_size(&explicit, &metrics), [200., 100.]);
    let mut raw = explicit;
    if let Actor::Sprite { size, scale, .. } = &mut raw {
        *size = [
            deadlib_present::actors::SizeSpec::Px(100.),
            deadlib_present::actors::SizeSpec::Px(50.),
        ];
        *scale = [2., 2.];
    }
    assert_eq!(composed_solid_size(&raw, &metrics), [100., 50.]);
}
