use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SizeSpec},
    space::{Metrics, set_current_metrics},
};
use deadsync_editor::zoom::SpacingMode;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};
fn init() {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
}
fn key(s: &mut State, c: KeyCode, p: bool) {
    s.raw_key(&RawKeyboardEvent {
        code: c,
        pressed: p,
        timestamp: Instant::now(),
        repeat: false,
        host_nanos: 0,
    });
}
fn press(s: &mut State, c: KeyCode) {
    key(s, c, true);
    key(s, c, false);
}
fn variable(s: &mut State) {
    key(s, KeyCode::SuperLeft, true);
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyT);
    key(s, KeyCode::ShiftLeft, false);
    key(s, KeyCode::SuperLeft, false);
}
fn reverse(s: &mut State) {
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyV);
    key(s, KeyCode::ShiftLeft, false);
}
fn actors(s: &State) -> Vec<Actor> {
    let mut a = Vec::new();
    s.actors(&mut a);
    a
}
fn arrows(s: &State) -> Vec<f32> {
    actors(s)
        .into_iter()
        .filter_map(|a| match a {
            Actor::Sprite {
                z: 8004, offset, ..
            } => Some(offset[1]),
            _ => None,
        })
        .collect()
}
fn sorted(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(f32::total_cmp);
    v
}
fn fixture(tags: &str, measures: usize) -> (tempfile::TempDir, State, Vec<u8>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("variable.ssc");
    let notes = vec!["1000\n0100\n0010\n0001"; measures].join(",\n");
    let source = format!(
        "#VERSION:0.83;#BPMS:0=240;{tags}#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{notes};"
    )
    .into_bytes();
    fs::write(&path, &source).unwrap();
    let s = State::open(&path, 0).unwrap();
    (dir, s, source)
}
#[test]
fn folded_scrolls_keep_later_notes_and_reverse_mirrors_every_visible_head() {
    init();
    let (dir, mut s, source) = fixture("#SCROLLS:0=1,4=-1,8=1;#SPEEDS:0=1=0=0;", 3);
    variable(&mut s);
    assert_eq!(s.spacing_mode(), SpacingMode::Variable);
    let a = arrows(&s);
    assert_eq!(
        sorted(a.clone()),
        vec![
            160.0, 160.0, 220.0, 220.0, 220.0, 280.0, 280.0, 280.0, 340.0, 340.0, 340.0
        ]
    );
    reverse(&mut s);
    let b = arrows(&s);
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        assert!((a + b - 500.0).abs() < 0.001);
    }
    variable(&mut s);
    assert_eq!(s.spacing_mode(), SpacingMode::ConstantRow);
    assert!(s.display_reversed());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(fs::read(dir.path().join("variable.ssc")).unwrap(), source);
}
#[test]
fn collapsed_scroll_and_speed_keep_all_small_chart_heads_and_signed_factors_compose() {
    init();
    for tags in ["#SCROLLS:0=0;", "#SPEEDS:0=0=0=0;"] {
        let (_dir, mut s, source) = fixture(tags, 4);
        variable(&mut s);
        assert_eq!(arrows(&s), vec![160.0; 16]);
        reverse(&mut s);
        assert_eq!(arrows(&s), vec![340.0; 16]);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    }
    let (_dir, mut s, _) = fixture("#SCROLLS:0=-1;#SPEEDS:0=-1=0=0;", 2);
    variable(&mut s);
    assert_eq!(sorted(arrows(&s)), vec![160.0, 220.0, 280.0, 340.0]);
}
#[test]
fn row_speed_interpolation_recomputes_at_cursor_without_editing_source() {
    init();
    for tags in [
        "#SPEEDS:0=1=0=0,2=3=2=0;",
        "#DELAYS:2=0.125;#SPEEDS:0=1=0=0,2=3=0.5=1;",
    ] {
        let (_dir, mut s, source) = fixture(tags, 3);
        for _ in 0..2 {
            press(&mut s, KeyCode::ArrowDown);
        }
        variable(&mut s);
        assert_eq!(s.workspace().row(), 96);
        assert_eq!(sorted(arrows(&s)), vec![160.0, 220.0, 280.0, 340.0]);
        press(&mut s, KeyCode::ArrowDown);
        assert_eq!(s.workspace().row(), 144);
        assert_eq!(sorted(arrows(&s)), vec![160.0, 280.0]);
        key(&mut s, KeyCode::SuperLeft, true);
        press(&mut s, KeyCode::ArrowUp);
        key(&mut s, KeyCode::SuperLeft, false);
        s.update_with_time(Instant::now() + Duration::from_secs(1));
        assert_eq!(sorted(arrows(&s)), vec![160.0, 304.0]);
        key(&mut s, KeyCode::SuperLeft, true);
        press(&mut s, KeyCode::Digit0);
        key(&mut s, KeyCode::SuperLeft, false);
        assert_eq!(sorted(arrows(&s)), vec![160.0, 280.0]);
        press(&mut s, KeyCode::F1);
        variable(&mut s);
        assert_eq!(s.spacing_mode(), SpacingMode::Variable);
        press(&mut s, KeyCode::Escape);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        assert!(!s.workspace().file().session.is_dirty());
    }
}
fn wav(p: &Path) {
    let pcm: Vec<i16> = (0..48000)
        .map(|i| if (7104..7296).contains(&i) { 30000 } else { 0 })
        .collect();
    let size = pcm.len() as u32 * 2;
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + size).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(48000u32.to_le_bytes());
    b.extend(96000u32.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend(size.to_le_bytes());
    for v in pcm {
        b.extend(v.to_le_bytes());
    }
    fs::write(p, b).unwrap();
}
#[test]
fn temporal_waveform_remains_defined_when_variable_chart_speed_is_zero() {
    init();
    let (dir, mut s, source) = fixture(
        "#MUSIC:pulse.wav;#OFFSET:0;#SCROLLS:0=0;#SPEEDS:0=0=0=0;",
        1,
    );
    wav(&dir.path().join("pulse.wav"));
    let prefs = dir.path().join("prefs.json");
    let mut file = deadsync_editor::audio_preferences::File::open(&prefs).unwrap();
    let mut settings = file.waveform();
    settings.set_show(false);
    settings.set_scroll_mode(deadsync_editor::waveform_spacing::ScrollMode::CurrentTempoAndRate);
    file.set_waveform(settings);
    file.save().unwrap();
    s.configure_audio_preferences(&prefs).unwrap();
    variable(&mut s);
    press(&mut s, KeyCode::KeyW);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !s.waveform_ready() {
        s.update();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let peaks = |s: &State| {
        let p = s.waveform_preferences().palette();
        actors(s)
            .into_iter()
            .filter_map(|a| match a {
                Actor::Sprite {
                    tint,
                    offset,
                    size: [SizeSpec::Px(w), _],
                    scale,
                    ..
                } if tint == p.resolved_sparse().rgba() || tint == p.dense().rgba() => {
                    let w = if w == 0.0 { scale[0] } else { w * scale[0] };
                    (w > 10.0).then_some(offset[1])
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(arrows(&s), vec![160.0; 4]);
    let a = peaks(&s);
    assert!(!a.is_empty());
    let expected = 160.0 + (0.15 - deadsync_editor::song_properties::DEFAULT_SYNC_OFFSET) * 240.0;
    assert!(a.iter().all(|y| (f64::from(*y) - expected).abs() < 4.0));
    reverse(&mut s);
    let b = peaks(&s);
    assert!(!b.is_empty());
    assert!(
        b.iter()
            .all(|y| (f64::from(*y) - (500.0 - expected)).abs() < 4.0)
    );
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn labels_returning_to_the_same_y_are_grouped_in_logical_row_order() {
    init();
    let (_dir, mut s, source) =
        fixture("#SCROLLS:0=1,2=-1,3=0,4=1;#LABELS:1=Premiere,4=Retour;", 2);
    variable(&mut s);
    let label = |s: &State| {
        actors(s)
            .into_iter()
            .find_map(|a| match a {
                Actor::Text {
                    content, offset, ..
                } if content.to_string() == "Premiere / Retour" => Some(offset[1]),
                _ => None,
            })
            .expect("both coincident labels remain readable")
    };
    assert_eq!(label(&s), 220.0);
    reverse(&mut s);
    assert_eq!(label(&s), 280.0);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
