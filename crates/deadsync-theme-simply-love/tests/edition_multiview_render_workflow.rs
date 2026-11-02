#[path = "support/row_geometry.rs"]
mod row_geometry;
#[path = "support/source_sprite_geometry.rs"]
mod source_sprite_geometry;
// Actor geometry uses production keys and Session grids. It does not claim GPU QA.
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SpriteSource},
    space::{Metrics, set_current_metrics},
};
use deadsync_editor::{
    Error,
    chart_order::{Orderer, Snapshot},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, sync::Mutex, time::Instant};
static SERIAL: Mutex<()> = Mutex::new(());
fn key(state: &mut State, code: KeyCode, pressed: bool) {
    state.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(state: &mut State, code: KeyCode) {
    key(state, code, true);
    key(state, code, false);
}
fn control(state: &mut State, code: KeyCode, shift: bool) {
    key(state, KeyCode::SuperLeft, true);
    if shift {
        key(state, KeyCode::ShiftLeft, true);
    }
    press(state, code);
    key(state, KeyCode::ShiftLeft, false);
    key(state, KeyCode::SuperLeft, false);
}
struct Native;
impl Orderer for Native {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error> {
        snapshot.indices_with(str::cmp)
    }
}
fn next(state: &mut State) {
    control(state, KeyCode::Tab, false);
    assert!(state.process_chart_order(&mut Native));
}
fn pin(state: &mut State) {
    control(state, KeyCode::KeyB, true);
}
fn setup(layout: &str) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("render.ssc");
    let source=format!("#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:2000\n0100\n3000\n0000;#NOTEDATA:;#STEPSTYPE:dance-double;#BPMS:0=60;#OFFSET:0.25;#NOTES:00002000\n10000000\n00003000\n00000000;#NOTEDATA:;#STEPSTYPE:{layout};#NOTES:00000100\n00000000\n00000000\n00000000\n&\n00000020\n00000000\n00000030\n00000000;").into_bytes();
    fs::write(&path, &source).unwrap();
    let mut state = State::open(&path, 0).unwrap();
    row_geometry::configure_centered(&mut state, dir.path());
    (dir, state, source)
}
#[derive(Debug)]
struct Glyph {
    x: f32,
    y: f32,
    color: [f32; 4],
    z: i16,
    atlas: bool,
    uv: Option<[f32; 4]>,
    crop_top: f32,
}
fn glyphs(state: &State) -> Vec<Glyph> {
    fn visit(actor: &Actor, out: &mut Vec<Glyph>) {
        match actor {
            Actor::Sprite {
                offset,
                tint,
                z,
                source,
                uv_rect,
                croptop,
                ..
            } if *z == 8004 || *z == 8003 => out.push(Glyph {
                x: source_sprite_geometry::center(actor).unwrap_or(*offset)[0],
                y: source_sprite_geometry::center(actor).unwrap_or(*offset)[1],
                color: *tint,
                z: *z,
                atlas: match source {
                    SpriteSource::TextureStatic(k) => k.ends_with("editor-grooveauthor/atlas.png"),
                    SpriteSource::Texture(k) | SpriteSource::TextureHandle { key: k, .. } => {
                        k.ends_with("editor-grooveauthor/atlas.png")
                    }
                    _ => false,
                },
                uv: *uv_rect,
                crop_top: *croptop,
            }),
            Actor::Frame { children, .. } => {
                for child in children {
                    visit(child, out)
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let mut out = Vec::new();
    for actor in actors {
        visit(&actor, &mut out)
    }
    out
}
fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.01
}
fn has(glyphs: &[Glyph], x: f32, y: f32, z: i16) -> bool {
    glyphs
        .iter()
        .any(|glyph| near(glyph.x, x) && near(glyph.y, y) && glyph.z == z)
}
fn part(g: &Glyph, [x, y, w, h]: [u32; 4]) -> bool {
    let Some(uv) = g.uv else {
        return false;
    };
    g.atlas
        && uv[0] >= x as f32 / 1280.0 - 1e-6
        && uv[2] <= (x + w) as f32 / 1280.0 + 1e-6
        && uv[1] >= y as f32 / 1280.0 - 1e-6
        && uv[3] <= (y + h) as f32 / 1280.0 + 1e-6
}
fn atlas_at(glyphs: &[Glyph], x: f32, y: f32, rect: [u32; 4]) -> bool {
    glyphs
        .iter()
        .any(|g| near(g.x, x) && near(g.y, y) && g.z == 8004 && part(g, rect))
}
const BODY: [u32; 4] = [1, 457, 128, 128];
const HEAD: [u32; 4] = [589, 1, 128, 128];
const CAP: [u32; 4] = [1, 1051, 128, 65];
#[test]
fn inactive_and_focused_notes_use_distinct_live_chart_clocks_at_the_same_musical_instant() {
    let _guard = SERIAL.lock().unwrap();
    let (dir, mut state, source) = setup("dance-routine");
    press(&mut state, KeyCode::ArrowDown);
    assert_eq!(state.workspace().row(), 48);
    pin(&mut state);
    next(&mut state);
    pin(&mut state);
    assert_eq!(state.workspace().chart(), 1);
    assert_eq!(state.workspace().row(), 36);
    let row = glyphs(&state);
    assert!(has(&row, 451.0, 124.0, 8004), "focused Double head {row:?}");
    assert!(
        has(&row, 715.0, 112.0, 8004),
        "inactive Single head {row:?}"
    );
    // The source tiles each body upward from its own tail; cap begins at the
    // tail row and extends65×(48/128)=24.375px beyond it.
    assert!(atlas_at(&row, 451.0, 124.0, HEAD));
    assert!(atlas_at(&row, 715.0, 112.0, HEAD));
    for y in [148.0, 196.0] {
        assert!(atlas_at(&row, 451.0, y, BODY), "Double body {row:?}");
    }
    for y in [136.0, 184.0] {
        assert!(atlas_at(&row, 715.0, y, BODY), "Single body {row:?}");
    }
    assert!(atlas_at(&row, 451.0, 232.1875, CAP));
    assert!(atlas_at(&row, 715.0, 220.1875, CAP));
    for variable in [false, true] {
        control(&mut state, KeyCode::KeyT, variable);
        let values = glyphs(&state);
        assert!(
            has(&values, 763.0, 160.0, 8004),
            "inactive note at the shared instant {values:?}"
        );
        assert!(
            values
                .iter()
                .any(|glyph| near(glyph.x, 259.0) && glyph.y > 160.0 && glyph.z == 8004)
        );
    }
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert_eq!(fs::read(dir.path().join("render.ssc")).unwrap(), source);
    assert!(!state.workspace().file().session.is_dirty());
}
#[test]
fn couple_and_routine_inactive_views_show_both_participants_and_crossing_sustains() {
    let _guard = SERIAL.lock().unwrap();
    for layout in ["dance-couple", "dance-routine"] {
        let (_dir, mut state, _source) = setup(layout);
        press(&mut state, KeyCode::ArrowDown);
        pin(&mut state);
        next(&mut state);
        pin(&mut state);
        next(&mut state);
        pin(&mut state);
        control(&mut state, KeyCode::PageDown, false);
        assert_eq!(state.workspace().chart(), 1);
        let values = glyphs(&state);
        let p1 = values
            .iter()
            .find(|glyph| near(glyph.x, 43.0) && near(glyph.y, 112.0) && glyph.z == 8004)
            .unwrap_or_else(|| panic!("missing inactive P1 {layout}: {values:?}"));
        let p2 = values
            .iter()
            .find(|glyph| near(glyph.x, 91.0) && near(glyph.y, 112.0) && glyph.z == 8004)
            .unwrap_or_else(|| panic!("missing inactive P2 {layout}: {values:?}"));
        let grid = state.workspace().file().session.loaded_grid(2).unwrap();
        assert_eq!(
            grid.get(deadsync_editor::notes::Position {
                player: 0,
                lane: 5,
                row: 0
            })
            .unwrap()
            .kind,
            deadsync_editor::notes::Kind::Tap
        );
        assert_eq!(
            grid.get(deadsync_editor::notes::Position {
                player: 1,
                lane: 6,
                row: 0
            })
            .unwrap()
            .end,
            Some(96)
        );
        if layout == "dance-routine" {
            assert_ne!(p1.color, p2.color);
        }
        let body = values
            .iter()
            .find(|g| near(g.x, 91.0) && near(g.y, 136.0) && part(g, BODY))
            .unwrap();
        let expected = if layout == "dance-routine" {
            [28.0 / 255.0, 169.0 / 255.0, 169.0 / 255.0, 1.0]
        } else {
            [68.0 / 255.0, 68.0 / 255.0, 68.0 / 255.0, 1.0]
        };
        assert_eq!(body.color, expected);
        assert!(atlas_at(&values, 91.0, 184.0, BODY));
        assert!(atlas_at(&values, 91.0, 220.1875, CAP));
        // Advance the focused60BPM chart from row36 to48. The inactive120BPM
        // sustain now starts above the viewport but still crosses its top edge.
        press(&mut state, KeyCode::ArrowDown);
        assert_eq!(state.workspace().row(), 48);
        let crossing = glyphs(&state);
        assert!(
            crossing
                .iter()
                .any(|g| near(g.x, 91.0) && near(g.y, 112.0) && part(g, BODY) && g.crop_top > 0.0),
            "crossing {layout}: {crossing:?}"
        );
        assert!(atlas_at(&crossing, 91.0, 196.1875, CAP));
        let grid = state.workspace().file().session.loaded_grid(2).unwrap();
        assert_eq!(
            grid.get(deadsync_editor::notes::Position {
                player: 1,
                lane: 6,
                row: 0
            })
            .unwrap()
            .end,
            Some(96)
        );
    }
}
#[test]
fn changing_view_focus_repositions_natural_fields_and_keyboard_edits_only_the_focused_grid() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, source) = setup("dance-routine");
    press(&mut state, KeyCode::ArrowDown);
    pin(&mut state);
    next(&mut state);
    pin(&mut state);
    let original = state
        .workspace()
        .file()
        .session
        .loaded_grid(0)
        .unwrap()
        .clone();
    control(&mut state, KeyCode::PageDown, false);
    assert_eq!(state.workspace().chart(), 0);
    let values = glyphs(&state);
    assert!(has(&values, 355.0, 112.0, 8004));
    assert!(has(&values, 91.0, 124.0, 8004));
    let inactive = state
        .workspace()
        .file()
        .session
        .loaded_grid(1)
        .unwrap()
        .clone();
    press(&mut state, KeyCode::Digit3);
    assert_eq!(
        state.workspace().file().session.loaded_grid(1).unwrap(),
        &inactive
    );
    assert_ne!(
        state.workspace().file().session.loaded_grid(0).unwrap(),
        &original
    );
    control(&mut state, KeyCode::KeyZ, false);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    assert_eq!(state.workspace().chart(), 0);
}
#[test]
fn rendering_work_is_culled_by_visible_chart_geometry_not_total_dedicated_tab_count() {
    let _guard = SERIAL.lock().unwrap();
    let (_dir, mut state, _source) = setup("dance-routine");
    pin(&mut state);
    for _ in 0..20 {
        control(&mut state, KeyCode::KeyD, false);
        pin(&mut state);
    }
    assert_eq!(state.open_chart_views().unwrap().entries().len(), 21);
    let values = glyphs(&state);
    assert!(
        values.len() < 100,
        "offscreen dedicated charts should not create note actors: {}",
        values.len()
    );
    assert!(
        values
            .iter()
            .all(|glyph| glyph.x >= 0.0 && glyph.x <= 854.0)
    );
}
