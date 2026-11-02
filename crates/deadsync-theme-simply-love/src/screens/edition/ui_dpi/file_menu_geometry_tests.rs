//! Full pages, actual Miso glyph geometry and actual composed cameras.
use super::*;
use deadlib_platform::input::RawKeyboardEvent;
use deadlib_present::{
    compose,
    font::FontMap,
    space::{self, Metrics},
};
use deadlib_render_core::{DrawOp, RenderFrame};
use deadsync_editor::options_preferences::General;
use std::{collections::BTreeSet, time::Instant};
const NO_SONG: [&str; 18] = [
    "FileNew",
    "FileOpen",
    "FileRecent",
    "KeyBindingSettingsOpen",
    "DensitySettingsOpen",
    "DarkSettingsOpen",
    "StreamSettingsOpen",
    "ReceptorSettingsOpen",
    "NoteColorSettingsOpen",
    "KeyBindingActionOpenWaveformSettings",
    "KeyBindingActionOpenScrollSettings",
    "Close",
    "KeyBindingActionExit",
    "KeepEditing",
    "LogOpenPanel",
    "EditorHelpOpen",
    "PerformanceOpen",
    "ProfileManagerOpen",
];
const WITH_SONG: [&str; 9] = [
    "FileNew",
    "FileOpen",
    "FileRecent",
    "FileReload",
    "Save",
    "SaveAsOpen",
    "FileFolderOpen",
    "FileCloseSong",
    "KeepEditing",
];
fn press(state: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        state.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
fn labels(actors: &[Actor], out: &mut BTreeSet<String>) {
    for actor in actors {
        match actor {
            Actor::Text { content, .. } => {
                out.insert(content.as_str().to_owned());
            }
            Actor::Frame { children, .. } | Actor::Camera { children, .. } => labels(children, out),
            Actor::SharedFrame { children, .. }
            | Actor::SharedTransform { children, .. }
            | Actor::SharedCamera { children, .. } => labels(children, out),
            _ => {}
        }
    }
}
fn physical(frame: &RenderFrame, camera: u8, world: glam::Vec4, surface: [f32; 2]) -> [f32; 2] {
    let clip = frame.cameras[usize::from(camera)] * world;
    assert!(clip.w.is_finite() && clip.w.abs() > 1e-8);
    [
        (clip.x / clip.w + 1.) * 0.5 * surface[0],
        (1. - clip.y / clip.w) * 0.5 * surface[1],
    ]
}
fn contained(frame: &RenderFrame, surface: [f32; 2], context: &str) {
    let cards: Vec<_> = frame
        .ops
        .iter()
        .filter_map(|op| {
            if let DrawOp::Sprite(run) = op {
                Some(run)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        cards.len(),
        1,
        "only the modal Solid background belongs in this composed subtree: {context}"
    );
    let run = cards[0];
    assert_eq!(run.instance_count, 1);
    let card = &frame.sprite_instances[run.instance_start as usize];
    let corners = [[-1., -1.], [-1., 1.], [1., -1.], [1., 1.]].map(|sign| {
        physical(
            frame,
            run.camera,
            glam::Vec4::new(
                card.center[0] + sign[0] * card.size[0] * 0.5,
                card.center[1] + sign[1] * card.size[1] * 0.5,
                card.center[2],
                1.,
            ),
            surface,
        )
    });
    let low = [
        corners.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min),
        corners.iter().map(|p| p[1]).fold(f32::INFINITY, f32::min),
    ];
    let high = [
        corners
            .iter()
            .map(|p| p[0])
            .fold(f32::NEG_INFINITY, f32::max),
        corners
            .iter()
            .map(|p| p[1])
            .fold(f32::NEG_INFINITY, f32::max),
    ];
    assert!(
        low[0] >= -0.05
            && low[1] >= -0.05
            && high[0] <= surface[0] + 0.05
            && high[1] <= surface[1] + 0.05,
        "card outside viewport: {context} {low:?}..{high:?}"
    );
    let mut glyph_vertices = 0;
    for op in &frame.ops {
        let DrawOp::TexturedMesh(run) = op else {
            continue;
        };
        let geometry = &frame.tmesh_geometries[run.geometry as usize].vertices;
        let end = run.instance_start + run.instance_count;
        for instance in &frame.tmesh_instances[run.instance_start as usize..end as usize] {
            for vertex in geometry.iter() {
                let point = physical(
                    frame,
                    run.camera,
                    instance.transform()
                        * glam::Vec4::new(vertex.pos[0], vertex.pos[1], vertex.pos[2], 1.),
                    surface,
                );
                glyph_vertices += 1;
                assert!(
                    point[0] >= low[0] - 0.05
                        && point[0] <= high[0] + 0.05
                        && point[1] >= low[1] - 0.05
                        && point[1] <= high[1] + 0.05,
                    "actual Miso glyph outside card: {context} point{point:?} card{low:?}..{high:?}"
                );
            }
        }
    }
    assert!(
        glyph_vertices > 0,
        "no real Miso text was composed: {context}"
    );
}
#[test]
fn every_file_menu_entry_and_page_is_inside_real_composed_card_with_actual_miso_en_fr() {
    // Unit tests do not initialize the application-global asset paths.
    // Resolve the real repository assets from this compiled crate instead.
    let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let asset_roots = [project_root.join("assets")];
    let specs = crate::resources::FONT_ASSETS
        .into_iter()
        .filter(|spec| matches!(spec.name, "miso" | "game" | "cjk" | "emoji"));
    let parsed =
        deadlib_assets::parse_font_asset_specs(specs, &asset_roots, |path| project_root.join(path))
            .unwrap();
    let mut textures = deadlib_assets::TextureStore::<()>::new();
    // Register the same real generated Solid texture used by startup, without
    // creating a GPU. Missing handles would make compose discard the card.
    let white = deadlib_assets::white_texture_image();
    textures.queue_texture_upload(white.key.to_owned(), white.image);
    let mut fonts = FontMap::default();
    for asset in parsed {
        // Decode all actual PNG pages with their real font texture hints.
        // TextureStore assigns genuine asset identities and metadata before
        // backend upload; no synthetic constant handles or dimensions.
        let pages = deadlib_assets::prepare_required_font_textures(
            &asset.font,
            &asset.required_textures,
            &asset_roots,
            |key| textures.has_texture_key(key),
        )
        .unwrap();
        for page in pages {
            textures.queue_texture_upload_with_sampler(
                page.key,
                page.image,
                page.hints.sampler_desc(),
            );
        }
        for glyph in asset.font.glyph_map.values() {
            assert!(
                textures.has_texture_key(&glyph.texture_key),
                "missing actual font page {}",
                glyph.texture_key
            );
        }
        fonts.insert(asset.name, asset.font);
    }
    deadlib_present::font::refresh_chain_keys(&mut fonts);
    for language in ["en", "fr"] {
        i18n::init_for_tests();
        i18n::set_locale(deadsync_assets::language::load_for_tests(language));
        for with_song in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("menu.ssc");
            std::fs::write(&path,"#VERSION:0.83;#TITLE:Menu;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;").unwrap();
            let mut state = if with_song {
                State::open(&path, 0).unwrap()
            } else {
                State::without_song(root.path()).unwrap()
            };
            let source = state.workspace.file().session.serialize().unwrap();
            let keys: &[&str] = if with_song { &WITH_SONG } else { &NO_SONG };
            for (width, height) in [(854., 480.), (1280., 720.), (1920., 1080.), (3440., 1440.)] {
                for dpi in [1., 2.] {
                    let metrics = Metrics::centered(width / height * 480., 480.);
                    space::with_metrics(metrics, || {
                        state.set_ui_dpi_viewport(width as u32, height as u32);
                        state.ui_dpi = Some(Capture::new(&General::default(), dpi));
                        state.ui_dpi_render_scale = dpi as f32;
                        state.ui_dpi_supported = true;
                        state.open_file_menu();
                        let mut seen = BTreeSet::new();
                        for selected in 0..keys.len() {
                            assert!(matches!(state.panel,Some(Panel::FileMenu(i))if i==selected));
                            let mut actors = Vec::new();
                            state.panel_actors(Panel::FileMenu(selected), &mut actors);
                            labels(&actors, &mut seen);
                            // DPI adds its global page counter as a separate sibling outside the
                            // modal. Compose only the modal subtree; without a transform all actors
                            // here are File-menu-owned, including its own position footer.
                            let modal =
                                if matches!(actors.first(), Some(Actor::SharedTransform { .. })) {
                                    &actors[..1]
                                } else {
                                    actors.as_slice()
                                };
                            let frame = compose::build_screen_with_texture_context(
                                modal, [0.; 4], &metrics, &fonts, 0., &textures,
                            );
                            contained(
                                &frame,
                                [width, height],
                                &format!(
                                    "{language} song{with_song} {width}x{height} dpi{dpi} selected{selected} labels{seen:?}"
                                ),
                            );
                            press(&mut state, KeyCode::ArrowDown);
                        }
                        assert!(
                            matches!(state.panel, Some(Panel::FileMenu(0))),
                            "all entries stay reachable and wrap through the real key handler"
                        );
                        for key in keys {
                            let label = i18n::tr("Edition", key);
                            assert!(
                                seen.contains(label.as_ref()),
                                "entry {key} was never rendered across all pages, {language} song{with_song} {width}x{height} dpi{dpi}"
                            );
                        }
                    });
                }
            }
            assert_eq!(state.workspace.file().session.serialize().unwrap(), source);
            assert!(state.workspace.file().session.undo_label().is_none());
        }
    }
}
