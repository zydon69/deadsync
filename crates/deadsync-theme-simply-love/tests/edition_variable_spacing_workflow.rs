#[path = "support/transport_host.rs"]
mod transport_host;
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
fn projection_fixture(path: &Path, chart: usize) -> Result<State, deadsync_editor::Error> {
    let mut state = State::open(path, chart)?;
    transport_host::configure_projection(
        &mut state,
        &path.parent().unwrap().join("prefs.json"),
        false,
    );
    transport_host::publish(&mut state);
    Ok(state)
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
fn arrows(state: &State) -> Vec<f32> {
    transport_host::heads(state)
        .into_iter()
        .map(|head| head[1])
        .filter(|y| (105.0..=395.0).contains(y))
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
    let s = projection_fixture(&path, 0).unwrap();
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
    s.take_audio_commands();
    let prepared = deadlib_audio_decode::prepared::PreparedAudioSource::load_editor(
        &dir.path().join("pulse.wav"),
        deadlib_audio_decode::DecodeOptions::STEP_MANIA_TIMELINE,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    let registration = s.editor_asset_registration();
    s.observe_editor_main_source(
        s.editor_asset_owner(),
        registration,
        Some((registration, 0)),
        Some(prepared),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !s.waveform_ready() {
        s.update();
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
    let peaks = |state: &State| {
        let palette = state.waveform_preferences().palette();
        let rendered = actors(state);
        let display_width = rendered
            .iter()
            .find_map(|actor| match actor {
                Actor::Sprite {
                    source: deadlib_present::actors::SpriteSource::RenderTarget { .. },
                    z: 8001,
                    size: [SizeSpec::Px(width), _],
                    scale,
                    ..
                } => Some(width * scale[0]),
                _ => None,
            })
            .expect("visible waveform composite");
        let passes = state.take_waveform_render_targets();
        let source = passes
            .iter()
            .find(|pass| pass.postprocess.is_none())
            .expect("source waveform pass");
        let ratio = display_width / source.logical_size[0];
        source
            .children
            .iter()
            .filter_map(|actor| match actor {
                Actor::Sprite {
                    tint,
                    offset,
                    size: [SizeSpec::Px(width), _],
                    scale,
                    ..
                } if *tint == palette.resolved_sparse().rgba().map(|v| v.clamp(0., 1.))
                    || *tint == palette.dense().rgba().map(|v| v.clamp(0., 1.)) =>
                {
                    let width = if *width == 0. {
                        scale[0]
                    } else {
                        width * scale[0]
                    } * ratio;
                    (width > 10.).then_some(offset[1])
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(arrows(&s), vec![160.0; 4]);
    let a = peaks(&s);
    assert!(!a.is_empty());
    let expected = 160.0 + (0.15 - deadsync_editor::song_properties::DEFAULT_SYNC_OFFSET) * 240.0;
    assert!(
        a.iter().all(|y| (f64::from(*y) - expected).abs() < 4.0),
        "waveform peaks {a:?}, expected {expected}"
    );
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
fn labels_returning_to_the_same_y_keep_distinct_readable_event_widgets() {
    init();
    let (_dir, mut s, source) =
        fixture("#SCROLLS:0=1,2=-1,3=0,4=1;#LABELS:1=Premiere,4=Retour;", 2);
    variable(&mut s);
    let labels = |s: &State| {
        let mut result = actors(s)
            .into_iter()
            .filter_map(|actor| match actor {
                Actor::Text {
                    content, offset, ..
                } if content.contains("Premiere") || content.contains("Retour") => {
                    Some((content.to_string(), offset))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            result.len(),
            2,
            "each source label owns its event widget: {result:?}"
        );
        assert!(result[0].0.contains("Premiere"));
        assert!(result[1].0.contains("Retour"));
        assert_ne!(
            result[0].1, result[1].1,
            "coincident labels must remain individually readable"
        );
        result
    };
    let normal = labels(&s);
    reverse(&mut s);
    let reversed = labels(&s);
    for ((name, before), (after_name, after)) in normal.iter().zip(&reversed) {
        assert_eq!(name, after_name);
        assert_ne!(
            before[1], after[1],
            "reverse moves both coincident label widgets"
        );
    }
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}
