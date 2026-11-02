//! Localized overview captions retain every glyph with uniform fitting.
use super::*;
use deadlib_present::{
    compose,
    font::{self, FontMap},
    space::{self, Metrics},
};
use deadsync_editor::options_preferences::{General, dpi::Capture};
use glam::Vec4;
fn fonts() -> (FontMap, deadlib_assets::TextureStore<()>) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let parsed = deadlib_assets::parse_font_with_asset_context(
        &root.join("assets/fonts/miso/_miso light.ini"),
        &[root.join("assets")],
    )
    .unwrap();
    // Geometry composition requires the real page registry as well as font
    // metrics. The convenience build_screen uses NullTextureContext and drops
    // every TexturedMesh with INVALID_TEXTURE_HANDLE at finalization.
    let pages = deadlib_assets::prepare_required_font_textures(
        &parsed.font,
        &parsed.required_textures,
        &[root.join("assets")],
        |_| false,
    )
    .expect("decode all required bundled Miso pages");
    assert!(!pages.is_empty());
    let mut textures = deadlib_assets::TextureStore::<()>::new();
    for page in pages {
        assert!(page.image.width() > 0 && page.image.height() > 0);
        let sampler = page.hints.sampler_desc();
        textures.queue_texture_upload_with_sampler(page.key, page.image, sampler);
    }
    for glyph in parsed.font.glyph_map.values() {
        let binding = textures
            .bind_texture(&glyph.texture_key)
            .expect("every parsed glyph page must have a real decoded texture binding");
        assert_ne!(binding.handle, deadlib_render_core::INVALID_TEXTURE_HANDLE);
        assert!(binding.dimensions.is_some());
    }
    let mut fonts = FontMap::default();
    fonts.insert("miso", parsed.font);
    font::refresh_chain_keys(&mut fonts);
    (fonts, textures)
}
fn vertices(
    actors: &[Actor],
    fonts: &FontMap,
    metrics: &Metrics,
    textures: &deadlib_assets::TextureStore<()>,
) -> Vec<[f32; 2]> {
    let frame =
        compose::build_screen_with_texture_context(actors, [0.; 4], metrics, fonts, 0., textures);
    let mut out = Vec::new();
    for op in &frame.ops {
        let deadlib_render_core::DrawOp::TexturedMesh(run) = op else {
            continue;
        };
        let geometry = &frame.tmesh_geometries[run.geometry as usize];
        for instance in &frame.tmesh_instances
            [run.instance_start as usize..(run.instance_start + run.instance_count) as usize]
        {
            for vertex in geometry.vertices.iter() {
                let p = instance.transform()
                    * Vec4::new(vertex.pos[0], vertex.pos[1], vertex.pos[2], 1.);
                out.push([p.x, p.y]);
            }
        }
    }
    out
}

fn extent(vertices: &[[f32; 2]], axis: usize) -> f32 {
    let min = vertices
        .iter()
        .map(|p| p[axis])
        .fold(f32::INFINITY, f32::min);
    let max = vertices
        .iter()
        .map(|p| p[axis])
        .fold(f32::NEG_INFINITY, f32::max);
    max - min
}
#[test]
fn localized_overview_captions_fit_without_horizontal_glyph_distortion() {
    let (fonts, textures) = fonts();
    let root = tempfile::tempdir().unwrap();
    let mut state = State::without_song(root.path()).unwrap();
    for language in ["en", "fr"] {
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        for dpi in [1.0, 2.0] {
            let metrics = Metrics::centered(854.0, 480.0);
            space::with_metrics(metrics, || {
                state.ui_dpi = Some(Capture::new(&General::default(), dpi));
                state.ui_dpi_render_scale = dpi as f32;
                state.set_ui_dpi_viewport((854.0 * dpi) as u32, (480.0 * dpi) as u32);
                for width in [90.0, 120.0, 180.0] {
                    let mut actual = Vec::new();
                    state.overview_label(
                        &mut actual,
                        110.0,
                        width,
                        0,
                        i18n::tr("Edition", "MinimapOverview"),
                    );
                    let Actor::Text {
                        clip, max_width, ..
                    } = &mut actual[0]
                    else {
                        panic!("caption text");
                    };
                    assert!(
                        max_width.is_none(),
                        "width-only glyph compression must not be used"
                    );
                    *clip = None; // Inspect the complete mesh before the safety clip.
                    let mut original = actual.clone();
                    let Actor::Text {
                        fit_width,
                        fit_height,
                        ..
                    } = &mut original[0]
                    else {
                        unreachable!()
                    };
                    *fit_width = None;
                    *fit_height = None;
                    let fitted = vertices(&actual, &fonts, &metrics, &textures);
                    let unscaled = vertices(&original, &fonts, &metrics, &textures);
                    assert!(!fitted.is_empty());
                    assert_eq!(fitted.len(), unscaled.len(), "no glyph may be discarded");
                    let x_ratio = extent(&fitted, 0) / extent(&unscaled, 0);
                    let y_ratio = extent(&fitted, 1) / extent(&unscaled, 1);
                    assert!(
                        (x_ratio - y_ratio).abs() < 0.001,
                        "{language} dpi{dpi}: uniform glyph scaling"
                    );
                    assert!(
                        extent(&fitted, 0) <= width + 0.01,
                        "{language} dpi{dpi} width{width}: full caption width {}",
                        extent(&fitted, 0)
                    );
                    assert!(
                        fitted.iter().all(|p| p[0] - metrics.left >= 110.0 - 0.01
                            && p[0] - metrics.left <= 110.0 + width + 0.01),
                        "{language} dpi{dpi} width{width}: glyph quads must lie inside the actual column clip"
                    );
                    assert!(
                        extent(&fitted, 1) <= state.overview_vertical_layout().line_height + 0.01
                    );
                }
            });
        }
    }
}
