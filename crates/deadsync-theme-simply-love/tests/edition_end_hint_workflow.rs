use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::Actor,
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
fn ctrl(state: &mut State, code: KeyCode, shift: bool) {
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
fn setup(hint: f64, sync: f64) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("hint.ssc");
    let bytes=format!("#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#LASTSECONDHINT:{hint};#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":{sync}}};#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#BPMS:0=60;#OFFSET:0.25;#NOTES:10000000;").into_bytes();
    fs::write(&path, &bytes).unwrap();
    let preferences = dir.path().join("prefs.json");
    fs::write(&preferences,
        r#"{"Version":1,"Scroll":{"SpacingMode":"ConstantRow","RowBasedPixelsPerRow":1.0},"Receptors":{"CenterHorizontally":true,"PositionY":110}}"#,
    ).unwrap();
    let state = State::open_with_preferences(&path, 0, &preferences).unwrap();
    (dir, state, bytes)
}
fn hints(state: &State) -> Vec<(String, f32, f32)> {
    fn walk(a: &Actor, out: &mut Vec<(String, f32, f32)>) {
        match a {
            Actor::Text {
                content, offset, z, ..
            } if *z == 8004
                && content.as_str().starts_with(
                    i18n::tr("Edition", "ChartEndHint")
                        .split("{seconds}")
                        .next()
                        .unwrap(),
                ) =>
            {
                out.push((content.to_string(), offset[0], offset[1]))
            }
            Actor::Frame { children, .. } => {
                for child in children {
                    walk(child, out)
                }
            }
            _ => {}
        }
    }
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let mut out = Vec::new();
    for a in actors {
        walk(&a, &mut out)
    }
    out
}
fn pin_both_focus_single(state: &mut State) {
    ctrl(state, KeyCode::KeyB, true);
    ctrl(state, KeyCode::Tab, false);
    assert!(state.process_chart_order(&mut Native));
    ctrl(state, KeyCode::KeyB, true);
    ctrl(state, KeyCode::PageDown, false);
    assert_eq!(state.workspace().chart(), 0);
}
#[test]
fn end_hint_maps_song_seconds_through_each_chart_offset_bpm_and_sync() {
    let _guard = SERIAL.lock().unwrap();
    for (sync, ys) in [(0.0, [208.0, 184.0]), (0.1, [198.4, 179.2])] {
        let (_dir, mut state, source) = setup(0.5, sync);
        pin_both_focus_single(&mut state);
        for language in ["en", "fr"] {
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let values = hints(&state);
            assert_eq!(values.len(), 2, "{values:?}");
            let label = i18n::tr_fmt("Edition", "ChartEndHint", &[("seconds", "0.500")]);
            // The inactive chart's hint shares the misc-widget column with
            // its initial tempo/signature: collision resolution places it just
            // below that stack, while the later focused-chart hint stays exact.
            for (x, y) in [(529.0, ys[0] - 9.0), (265.0, 183.0)] {
                assert!(
                    values.iter().any(|(text, a, b)| text == &*label
                        && (*a - x).abs() < 0.01
                        && (*b - y).abs() < 0.01),
                    "{values:?} expected{x},{y}"
                );
            }
        }
        ctrl(&mut state, KeyCode::KeyT, false);
        let expected_y = if sync == 0.0 { 301.0 } else { 271.0 };
        let timed = hints(&state);
        assert_eq!(timed.len(), 2);
        assert!(
            timed.iter().all(|(_, _, y)| (*y - expected_y).abs() < 0.01),
            "shared song instant in time spacing: {timed:?}"
        );
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            source
        );
        assert!(!state.workspace().file().session.is_dirty());

        // A later hint clears the initial-widget stack. Verify both chart
        // clocks exactly, independently of the earlier collision displacement.
        let (_later_dir, mut later, later_source) = setup(1.0, sync);
        pin_both_focus_single(&mut later);
        let later_values = hints(&later);
        assert_eq!(later_values.len(), 2);
        for (x, y) in [(529.0, ys[0] + 48.0 - 9.0), (265.0, ys[1] + 24.0 - 9.0)] {
            assert!(
                later_values
                    .iter()
                    .any(|(_, a, b)| (*a - x).abs() < 0.01 && (*b - y).abs() < 0.01),
                "{later_values:?} expected {x},{y}"
            );
        }
        assert_eq!(
            later.workspace().file().session.serialize().unwrap(),
            later_source
        );
    }
}
#[test]
fn nonpositive_hints_are_absent_and_song_draft_updates_all_views_then_cancels_or_commits() {
    let _guard = SERIAL.lock().unwrap();
    for value in [-1.0, 0.0] {
        let (_dir, mut state, _) = setup(value, 0.0);
        pin_both_focus_single(&mut state);
        assert!(hints(&state).is_empty());
    }
    let (_dir, mut state, source) = setup(0.5, 0.0);
    pin_both_focus_single(&mut state);
    let original = hints(&state);
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowRight);
    let staged = hints(&state);
    assert_eq!(staged.len(), 2);
    assert!(staged.iter().all(|(label, _, _)| label.contains("0.501")));
    assert_ne!(staged, original);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert_eq!(hints(&state), original);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
    press(&mut state, KeyCode::F11);
    press(&mut state, KeyCode::F6);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::ArrowRight);
    press(&mut state, KeyCode::Escape);
    press(&mut state, KeyCode::ArrowDown);
    press(&mut state, KeyCode::Enter);
    assert!(state.workspace().file().session.is_dirty());
    assert!(
        hints(&state)
            .iter()
            .all(|(label, _, _)| label.contains("0.501"))
    );
    ctrl(&mut state, KeyCode::KeyZ, false);
    assert_eq!(hints(&state), original);
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        source
    );
}
