//! The real retained font and composer, including parent UI-DPI transforms.
use super::*;
use deadlib_present::{
    compose,
    font::{self, FontMap},
    space::{self, Metrics},
};
use deadsync_editor::options_preferences::{General, dpi::Capture};
use glam::Vec4;
const URL: &str = "https://github.com/pnn64/deadsync/releases/tag/v0.5.2475";
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
fn keep_link(actor: &mut Actor, url: &str, reference: Option<f32>) -> bool {
    match actor {
        Actor::Text {
            content,
            scale,
            max_width,
            max_height,
            ..
        } if content.as_str() == url => {
            if let Some(ratio) = reference {
                *scale = [15. / 24. * ratio; 2];
                *max_width = None;
                *max_height = None;
            }
            true
        }
        Actor::Frame { children, .. } => {
            children.retain_mut(|a| keep_link(a, url, reference));
            !children.is_empty()
        }
        Actor::SharedTransform { children, .. } | Actor::SharedFrame { children, .. } => {
            let mut kept = children.to_vec();
            kept.retain_mut(|a| keep_link(a, url, reference));
            *children = kept.into();
            !children.is_empty()
        }
        _ => false,
    }
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
                // SharedTransform is carried in the draw camera, not in
                // the mesh instance. Include it before checking screen bounds.
                let camera = frame
                    .cameras
                    .get(run.camera as usize)
                    .copied()
                    .unwrap_or_else(|| metrics.projection());
                let p = metrics.projection().inverse() * camera * p;
                out.push([p.x / p.w, p.y / p.w]);
            }
        }
    }
    out
}
#[test]
fn actual_release_url_has_source_font_geometry_in_real_composer_en_fr_and_retina() {
    let (fonts, textures) = fonts();
    assert_eq!(fonts["miso"].line_spacing, 24);
    assert_eq!(
        font::measure_line_width_logical(&fonts["miso"], URL, &fonts),
        403
    );
    for language in ["en", "fr"] {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let mut s = State::without_song(root.path()).unwrap();
        for (width, height) in [(854., 480.), (1280., 720.), (2560., 1506.)] {
            for dpi in [1., 2.] {
                let metrics = Metrics::centered(width / height * 480., 480.);
                space::with_metrics(metrics, || {
                    s.set_ui_dpi_viewport(width as u32, height as u32);
                    s.ui_dpi = Some(Capture::new(&General::default(), dpi));
                    s.ui_dpi_render_scale = dpi as f32;
                    s.ui_dpi_supported = true;
                    s.notifications.update = Some(("v0.5.2475".into(), URL.into()));
                    s.panel = Some(Panel::UpdateNotice(0));
                    let before = s.workspace.file().session.serialize().unwrap();
                    let mut actors = Vec::new();
                    s.panel_actors(Panel::UpdateNotice(0), &mut actors);
                    let ratio = s.ui_dpi.unwrap().font_pixels().unwrap() as f32 / 15. / dpi as f32;
                    let mut reference = actors.clone();
                    reference.retain_mut(|a| keep_link(a, URL, Some(ratio)));
                    actors.retain_mut(|a| keep_link(a, URL, None));
                    let actual = vertices(&actors, &fonts, &metrics, &textures);
                    let expected = vertices(&reference, &fonts, &metrics, &textures);
                    assert!(!actual.is_empty(), "real URL glyph meshes must exist");
                    assert_eq!(actual.len(), expected.len());
                    for (a, e) in actual.iter().zip(&expected) {
                        for axis in 0..2 {
                            assert!(
                                (a[axis] - e[axis]).abs() < 0.02,
                                "{language} {width}x{height} dpi{dpi} glyph axis{axis}: {} != {}",
                                a[axis],
                                e[axis]
                            );
                        }
                    }
                    assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
                    assert!(s.take_audio_commands().iter().all(|c| !matches!(
                        c,
                        deadsync_gameplay::GameplayAudioCommand::EditorStartMusic { .. }
                    )));
                });
            }
        }
    }
}
#[test]
fn long_url_is_clipped_without_glyph_compression_and_open_keeps_original_target() {
    let (fonts, textures) = fonts();
    let root = tempfile::tempdir().unwrap();
    let mut s = State::without_song(root.path()).unwrap();
    space::with_metrics(Metrics::centered(854., 480.), || {
        let url = format!("{URL}?{}", "x".repeat(1000));
        s.notifications.update = Some(("v0.5.2475".into(), url.clone()));
        let mut actors = Vec::new();
        s.update_notice_actors(0, &mut actors);
        let Actor::Text {
            content,
            scale,
            max_width,
            max_height,
            clip,
            ..
        } = &actors[2]
        else {
            panic!("URL actor")
        };
        assert_eq!(content.as_str(), url);
        assert_eq!(*scale, [15. / 24.; 2]);
        assert!(max_width.is_none() && max_height.is_none());
        assert_eq!(*clip, Some([20., 172., 814., 26.]));
        let mut reference = actors.clone();
        reference.retain_mut(|a| keep_link(a, &url, Some(1.)));
        actors.retain_mut(|a| keep_link(a, &url, None));
        let actual = vertices(&actors, &fonts, &Metrics::centered(854., 480.), &textures);
        let expected = vertices(
            &reference,
            &fonts,
            &Metrics::centered(854., 480.),
            &textures,
        );
        assert!(!actual.is_empty());
        assert_eq!(actual, expected);
        s.update_notice_key(1, KeyCode::Enter);
        assert_eq!(s.take_notification_url(), Some(url));
    });
}

#[test]
fn missing_page_registry_cannot_claim_visible_font_geometry() {
    let (fonts, textures) = fonts();
    let actors = [crate::act!(text:font("miso"):settext(URL):xy(427.,240.):zoom(15./24.))];
    let metrics = Metrics::centered(854., 480.);
    let missing = deadlib_assets::TextureStore::<()>::new();
    assert!(
        vertices(&actors, &fonts, &metrics, &missing).is_empty(),
        "unbound real font pages are culled, not fabricated"
    );
    assert!(
        !vertices(&actors, &fonts, &metrics, &textures).is_empty(),
        "decoded pages must reach real composed meshes"
    );
}

#[test]
fn release_url_stays_inside_small_high_dpi_window_for_every_selected_action() {
    let (fonts, textures) = fonts();
    let root = tempfile::tempdir().unwrap();
    for language in ["en", "fr"] {
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        let mut state = State::without_song(root.path()).unwrap();
        let metrics = Metrics::centered(854., 480.);
        space::with_metrics(metrics, || {
            state.set_ui_dpi_viewport(854, 480);
            state.ui_dpi = Some(Capture::new(&General::default(), 2.));
            state.ui_dpi_render_scale = 2.;
            state.ui_dpi_supported = true;
            state.notifications.update = Some(("v0.5.2475".into(), URL.into()));
            for selected in 0..3 {
                let mut actors = Vec::new();
                state.panel_actors(Panel::UpdateNotice(selected), &mut actors);
                actors.retain_mut(|a| keep_link(a, URL, None));
                let glyphs = vertices(&actors, &fonts, &metrics, &textures);
                let mut reference = Vec::new();
                state.update_notice_actors(selected, &mut reference);
                reference.retain_mut(|a| keep_link(a, URL, None));
                let complete = vertices(&reference, &fonts, &metrics, &textures);
                assert!(!glyphs.is_empty());
                assert_eq!(
                    glyphs.len(),
                    complete.len(),
                    "{language} selected{selected}: every URL glyph must survive the DPI viewport transform"
                );
                assert!(
                    glyphs
                        .iter()
                        .all(|p| p[0] >= metrics.left && p[0] <= metrics.right),
                    "{language} selected{selected}: the complete release URL must remain within the window"
                );
            }
        });
    }
}
