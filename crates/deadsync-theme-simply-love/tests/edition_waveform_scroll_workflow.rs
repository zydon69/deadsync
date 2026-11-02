#[path = "support/prepared_audio.rs"]
mod prepared_audio;
use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::{
    actors::{Actor, SizeSpec},
    space::{Metrics, set_current_metrics},
};
use deadsync_editor::{audio_preferences::File, waveform_spacing::ScrollMode, zoom::SpacingMode};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{
    fs,
    path::Path,
    thread,
    time::{Duration, Instant},
};
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
fn down(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::ArrowDown);
    }
}
fn open(s: &mut State) {
    key(s, KeyCode::ShiftLeft, true);
    press(s, KeyCode::KeyW);
    key(s, KeyCode::ShiftLeft, false);
}
fn mode(s: &mut State, m: SpacingMode) {
    if s.spacing_mode() == m {
        return;
    }
    key(s, KeyCode::SuperLeft, true);
    if m == SpacingMode::Variable {
        key(s, KeyCode::ShiftLeft, true);
    }
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
    for target in s.take_waveform_render_targets() {
        a.extend(target.children.iter().cloned());
    }
    a
}
fn has(s: &State, t: &str) -> bool {
    actors(s)
        .iter()
        .any(|a| matches!(a,Actor::Text{content,..}if content.contains(t)))
}
fn mean_peak(s: &State) -> f64 {
    let p = s.waveform_preferences().palette();
    let values: Vec<f64> = actors(s)
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
                (w > 10.0).then_some(f64::from(offset[1]))
            }
            _ => None,
        })
        .collect();
    assert!(!values.is_empty(), "decoded pulse missing");
    values.iter().sum::<f64>() / values.len() as f64
}
fn wav(path: &Path) {
    let pcm: Vec<i16> = (0..192000)
        .map(|n| {
            if (173136..173328).contains(&n) {
                30000
            } else {
                0
            }
        })
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
    for n in pcm {
        b.extend(n.to_le_bytes());
    }
    fs::write(path, b).unwrap();
}
fn setup() -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    wav(&dir.path().join("pulse.wav"));
    let notes = vec!["1000\n0100\n0010\n0001"; 4].join(",\n");
    let source=format!("#VERSION:0.83;#VENDOR:keep;#MUSIC:pulse.wav;#OFFSET:0;#BPMS:0=120,4=240;#SCROLLS:0=1,8=2;#SPEEDS:0=2=0=0;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{notes};").into_bytes();
    let path = dir.path().join("scroll.ssc");
    fs::write(&path, &source).unwrap();
    let mut s = State::open(&path, 0).unwrap();
    s.configure_audio_preferences(&dir.path().join("prefs.json"))
        .unwrap();
    down(&mut s, 10);
    prepared_audio::publish_main(&mut s);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !s.waveform_ready() {
        s.update();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    (dir, s, source)
}
fn select(s: &mut State, path: &Path, policy: ScrollMode) {
    let active = s.waveform_preferences().scroll_mode();
    open(s);
    // Opening is a departure barrier for unrelated pending spacing preferences.
    let before = fs::read(path).ok();
    down(s, 4);
    press(s, KeyCode::Home);
    for _ in 0..match policy {
        ScrollMode::MostCommonTempo => 0,
        ScrollMode::CurrentTempo => 1,
        ScrollMode::CurrentTempoAndRate => 2,
    } {
        press(s, KeyCode::ArrowRight);
    }
    assert_eq!(s.waveform_preferences().scroll_mode(), active);
    assert_eq!(fs::read(path).ok(), before);
    down(s, 10);
    press(s, KeyCode::Enter);
    assert_eq!(s.waveform_preferences().scroll_mode(), policy);
    assert_eq!(File::open(path).unwrap().waveform().scroll_mode(), policy);
}
#[test]
fn saved_policies_move_real_waveform_in_all_modes_and_reload_without_chart_edits() {
    for m in [
        SpacingMode::ConstantRow,
        SpacingMode::ConstantTime,
        SpacingMode::Variable,
    ] {
        let (dir, mut s, source) = setup();
        let path = dir.path().join("prefs.json");
        mode(&mut s, m);
        assert_eq!(s.spacing_mode(), m);
        let rates = match m {
            SpacingMode::ConstantRow => [384.0, 384.0, 192.0],
            SpacingMode::ConstantTime => [300.0; 3],
            SpacingMode::Variable => [240.0, 960.0, 120.0],
        };
        assert!((mean_peak(&s) - (160.0 + 0.1 * rates[2])).abs() < 2.0);
        for (policy, pps) in [
            ScrollMode::CurrentTempo,
            ScrollMode::CurrentTempoAndRate,
            ScrollMode::MostCommonTempo,
        ]
        .into_iter()
        .zip(rates)
        {
            select(&mut s, &path, policy);
            assert!(
                (mean_peak(&s) - (160.0 + 0.1 * pps)).abs() < 2.0,
                "{m:?} {policy:?}: {}",
                mean_peak(&s)
            );
        }
        select(&mut s, &path, ScrollMode::CurrentTempoAndRate);
        let normal = mean_peak(&s);
        reverse(&mut s);
        assert!((normal + mean_peak(&s) - 500.0).abs() < 1.1);
        reverse(&mut s);
        let mut reopened = State::open(&dir.path().join("scroll.ssc"), 0).unwrap();
        reopened.configure_audio_preferences(&path).unwrap();
        assert_eq!(
            reopened.waveform_preferences().scroll_mode(),
            ScrollMode::CurrentTempoAndRate
        );
        down(&mut reopened, 10);
        mode(&mut reopened, m);
        prepared_audio::publish_main(&mut reopened);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !reopened.waveform_ready() {
            reopened.update();
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        assert!((mean_peak(&reopened) - (160.0 + 0.1 * rates[1])).abs() < 2.0);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
        assert!(!s.workspace().file().session.is_dirty());
        assert_eq!(fs::read(dir.path().join("scroll.ssc")).unwrap(), source);
    }
}
#[test]
fn policy_cancel_conflict_and_numeric_shortcuts_keep_active_state_and_source() {
    let (dir, mut s, source) = setup();
    let path = dir.path().join("prefs.json");
    select(&mut s, &path, ScrollMode::CurrentTempo);
    let before = fs::read(&path).unwrap();
    open(&mut s);
    down(&mut s, 4);
    press(&mut s, KeyCode::ArrowRight);
    assert!(has(&s, "Current tempo and rate"));
    press(&mut s, KeyCode::Escape);
    assert_eq!(
        s.waveform_preferences().scroll_mode(),
        ScrollMode::CurrentTempo
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    open(&mut s);
    down(&mut s, 4);
    press(&mut s, KeyCode::Home);
    assert!(has(&s, "Most common tempo"));
    press(&mut s, KeyCode::Digit0);
    assert!(has(&s, "Waveform scroll: Current tempo"));
    press(&mut s, KeyCode::ArrowLeft);
    assert!(has(&s, "Most common tempo"));
    let external = b"{\"Version\":1,\"Other\":true,\"Waveform\":{\"RowBasedWaveFormScrollMode\":\"CurrentTempoAndRate\"}}";
    fs::write(&path, external).unwrap();
    down(&mut s, 10);
    press(&mut s, KeyCode::Enter);
    assert_eq!(
        s.waveform_preferences().scroll_mode(),
        ScrollMode::CurrentTempo
    );
    assert!(has(&s, "Most common tempo"));
    assert_eq!(fs::read(&path).unwrap(), external);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert!(!s.workspace().file().session.is_dirty());
}

#[test]
fn waveform_controls_remain_visible_when_chart_projection_reports_an_error() {
    i18n::init_for_tests();
    set_current_metrics(Metrics::centered(854.0, 480.0));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("extreme.ssc");
    fs::write(&path,"#VERSION:0.83;#BPMS:0=1.6e308;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let mut s = State::open(&path, 0).unwrap();
    mode(&mut s, SpacingMode::Variable);
    key(&mut s, KeyCode::SuperLeft, true);
    press(&mut s, KeyCode::ArrowUp);
    key(&mut s, KeyCode::SuperLeft, false);
    s.update_with_time(Instant::now() + Duration::from_secs(1));
    assert!(has(&s, "Nonfinite"));
    open(&mut s);
    assert!(has(&s, "Waveform scroll: Most common tempo"));
    press(&mut s, KeyCode::Escape);
    key(&mut s, KeyCode::SuperLeft, true);
    press(&mut s, KeyCode::Digit0);
    key(&mut s, KeyCode::SuperLeft, false);
    assert!(
        actors(&s)
            .iter()
            .any(|a| matches!(a, Actor::Sprite { z: 8004, .. }))
    );
    assert!(!s.workspace().file().session.is_dirty());
}
