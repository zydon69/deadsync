use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::minimap_palette::{Family, Palette, Preferences};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Arc, time::Instant};
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
fn chord(state: &mut State, code: KeyCode) {
    state.raw_key(&RawKeyboardEvent {
        code: KeyCode::SuperLeft,
        pressed: true,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
    press(state, code);
    state.raw_key(&RawKeyboardEvent {
        code: KeyCode::SuperLeft,
        pressed: false,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn setup(notes: &str, prefs: serde_json::Value) -> (tempfile::TempDir, State) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    fs::write(&song,format!("#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#WARPS:3=1;#NOTES:{notes};")).unwrap();
    let path = dir.path().join("preferences.json");
    fs::write(&path, serde_json::to_vec(&prefs).unwrap()).unwrap();
    let mut state = State::open(&song, 0).unwrap();
    state.configure_audio_preferences(&path).unwrap();
    state.configure_spacing_preference(
        deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
    );
    (dir, state)
}
fn render(state: &State) -> Vec<Actor> {
    let mut actors = Vec::new();
    state.actors(&mut actors);
    actors
}
fn mesh(actors: &[Actor]) -> Arc<[deadlib_render_core::MeshVertex]> {
    actors
        .iter()
        .find_map(|actor| match actor {
            Actor::Mesh {
                vertices, z: 8011, ..
            } => Some(Arc::clone(vertices)),
            _ => None,
        })
        .expect("minimap retained mesh")
}
fn rgb(color: u32) -> [f32; 4] {
    [
        (color & 255) as f32 / 255.0,
        ((color >> 8) & 255) as f32 / 255.0,
        ((color >> 16) & 255) as f32 / 255.0,
        (color >> 24) as f32 / 255.0,
    ]
}
#[test]
fn state_draws_crossing_hold_roll_and_mine_with_original_palette_and_reuses_mesh() {
    let (_dir, mut state) = setup(
        "2000\n0100\n0000\n0004,\n3000\n0000\n0003\n00M0",
        serde_json::json!({"Version":1,"MiniMap":{"MiniMapVisibleRowRange":192}}),
    );
    for _ in 0..5 {
        press(&mut state, KeyCode::ArrowDown);
    }
    let before = state.workspace().file().session.serialize().unwrap();
    let actors = render(&state);
    let first = mesh(&actors);
    let progress = state.minimap_content_progress().unwrap();
    assert_eq!(
        progress,
        (3, true, true, true),
        "two crossing sustains and one visible mine"
    );
    let palette = Palette::new(&Preferences::default()).unwrap();
    let colors = palette.colors(Family::Row, 0, 0, 0, false).unwrap();
    assert!(
        first.iter().any(|vertex| vertex.color == rgb(colors.hold)),
        "crossing hold body present"
    );
    assert!(
        first.iter().any(|vertex| vertex.color == rgb(colors.roll)),
        "crossing roll body present"
    );
    let geometry = state.minimap_viewport_geometry().unwrap();
    let raster = deadsync_editor::minimap_content::Raster::new(
        90,
        geometry.projection.visible_height as u32,
        4,
        2,
        3,
        false,
        0,
    )
    .unwrap();
    let mine_x = raster.lane_positions()[2] as f32;
    assert!(
        first.iter().any(|vertex| vertex.pos[0] >= mine_x
            && vertex.pos[0] <= mine_x + 2.0
            && vertex.color[0] > vertex.color[1] + 0.2
            && vertex.color[0] > vertex.color[2] + 0.2),
        "antialiased mine color stays in its lane"
    );
    let again = mesh(&render(&state));
    assert!(Arc::ptr_eq(&first, &again));
    press(&mut state, KeyCode::Space);
    assert!(
        state.minimap_content_progress().is_none(),
        "selection changed before a new frame"
    );
    let selected = mesh(&render(&state));
    assert!(
        !Arc::ptr_eq(&first, &selected),
        "selection invalidates colors/cache"
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        before
    );
    assert!(!state.workspace().file().session.is_dirty());
    press(&mut state, KeyCode::Digit1);
    assert!(
        state.minimap_content_progress().is_none(),
        "edited content cannot certify an older frame"
    );
    let edited = mesh(&render(&state));
    assert!(!Arc::ptr_eq(&selected, &edited));
    press(&mut state, KeyCode::F11);
    assert!(
        state.minimap_content_progress().is_none(),
        "panel hides the minimap immediately"
    );
    assert!(
        !render(&state)
            .iter()
            .any(|actor| matches!(actor, Actor::Mesh { z: 8011, .. }))
    );
}
fn warp_notes() -> String {
    let mut rows = vec!["0000"; 48];
    rows[36] = "1000";
    rows[40] = "0100";
    rows[45] = "0010";
    format!("{},\n0001", rows.join("\n"))
}
#[test]
fn state_time_minimap_keeps_warp_heads_and_reports_source_budget_partial_in_each_language() {
    for maximum in [6144, 1] {
        let (_dir, mut state) = setup(
            &warp_notes(),
            serde_json::json!({"Version":1,"MiniMap":{"MiniMapVisibleTimeRange":1},"Options":{"MiniMapMaxNotesToDraw":maximum}}),
        );
        for _ in 0..3 {
            press(&mut state, KeyCode::ArrowDown);
        }
        chord(&mut state, KeyCode::KeyT);
        let actors = render(&state);
        let progress = state.minimap_content_progress().unwrap();
        assert_eq!(progress.0, if maximum == 1 { 2 } else { 4 });
        assert_eq!(progress.1, maximum != 1);
        assert!(progress.2 && progress.3);
        let partial = i18n::tr("Edition", "MinimapPartial");
        assert_eq!(
            actors.iter().any(
                |actor| matches!(actor,Actor::Text{content,..}if content.contains(partial.as_ref()))
            ),
            maximum == 1
        );
        assert!(mesh(&actors).len() > 0);
        assert!(!state.workspace().file().session.is_dirty());
    }
}
#[test]
fn state_reads_shared_source_color_groups_and_visibility_without_changing_song() {
    let (dir, mut state) = setup(
        "1000\n0100\n0010\n0001",
        serde_json::json!({"Version":1,"NoteColor":{"HoldColor":{"X":0.2,"Y":0.4,"Z":0.6},"HeldColorMultiplier":1.25,"ArrowUIColorMultiplier":1.0,"UseRowBasedColoringForPiu":true,"Unknown":55},"Options":{"StepColorMethodValue":"Beat"},"MiniMap":{"ShowLabels":false,"ShowPreview":false,"ShowPatterns":false}}),
    );
    let before = state.workspace().file().session.serialize().unwrap();
    let settings = state.note_color_preferences();
    assert_eq!(settings.colors.hold, [0.2, 0.4, 0.6]);
    assert_eq!(settings.held_multiplier, 1.25);
    assert!(settings.piu_row_based);
    assert!(state.minimap_content_progress().is_none());
    mesh(&render(&state));
    assert!(state.minimap_content_progress().unwrap().1);
    let path = dir.path().join("preferences.json");
    let mut prefs: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    prefs["MiniMap"]["ShowMiniMap"] = false.into();
    fs::write(&path, serde_json::to_vec(&prefs).unwrap()).unwrap();
    state.configure_audio_preferences(&path).unwrap();
    state.configure_spacing_preference(
        deadsync_editor::scroll_preferences::spacing::Preference::ConstantRow,
    );
    assert!(
        state.minimap_content_progress().is_none(),
        "ShowMiniMap hides a retained frame before rendering"
    );
    let actors = render(&state);
    assert!(state.minimap_content_progress().is_none());
    assert!(
        !actors
            .iter()
            .any(|actor| matches!(actor, Actor::Mesh { z: 8011, .. }))
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        before
    );
    assert!(!state.workspace().file().session.is_dirty());
}
