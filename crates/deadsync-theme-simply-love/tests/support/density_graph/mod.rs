use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn init() {
    if super::FRENCH {
        i18n::init(deadsync_assets::language::load_for_tests("fr"));
    } else {
        i18n::init_for_tests();
    }
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn ctrl(s: &mut State, code: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, code);
    key(s, KeyCode::SuperLeft, false);
}
fn serialized(s: &State) -> Vec<u8> {
    s.workspace().file().session.serialize().unwrap()
}
const SOURCE:&[u8]=b"#VERSION:0.83;#TITLE:Density;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1100\n1000\n0000\n0000,\n1000;";
fn prefs(s: &State) -> deadsync_editor::density_preferences::Preferences {
    s.density_preferences()
}
fn down(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::ArrowDown);
    }
}
fn settings(s: &mut State) {
    press(s, KeyCode::F1);
    down(s, 26);
    press(s, KeyCode::Enter);
}
fn contains(s: &State, key: &str) -> bool {
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let expected = i18n::tr("Edition", key);
    actors.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..}if content.as_ref()==expected.as_ref()))
}
fn finish_density(s: &mut State) {
    let deadline = Instant::now() + std::time::Duration::from_secs(20);
    while s.is_density_loading() {
        s.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
#[test]
fn density_settings_save_persist_and_undo_independently_of_simfile() {
    use deadsync_editor::{audio_preferences::File, density_preferences::Accumulation};
    init();
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    let preferences = dir.path().join("preferences.json");
    fs::write(&song, SOURCE).unwrap();
    let mut s = State::open(&song, 0).unwrap();
    s.configure_audio_preferences(&preferences).unwrap();
    settings(&mut s);
    assert!(contains(&s, "DensitySettingsHelp"));
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowRight);
    down(&mut s, 2);
    press(&mut s, KeyCode::Enter);
    assert_eq!(prefs(&s).accumulation, Accumulation::Row);
    assert_eq!(
        File::open(&preferences).unwrap().density().accumulation,
        Accumulation::Row
    );
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(serialized(&s), SOURCE);
    assert_eq!(fs::read(&song).unwrap(), SOURCE);
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(prefs(&s).accumulation, Accumulation::Step);
    ctrl(&mut s, KeyCode::KeyY);
    assert_eq!(prefs(&s).accumulation, Accumulation::Row);
    let mut reopened = State::open(&song, 0).unwrap();
    reopened.configure_audio_preferences(&preferences).unwrap();
    assert_eq!(prefs(&reopened).accumulation, Accumulation::Row);
}
#[test]
fn density_defaults_and_cancel_restore_all_mount_offsets_and_color_channels() {
    use deadsync_editor::{
        audio_preferences::File,
        density_preferences::{ColorMode, Position},
    };
    init();
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    let preferences = dir.path().join("preferences.json");
    fs::write(&song, SOURCE).unwrap();
    let mut f = File::open(&preferences).unwrap();
    let mut p = f.density();
    p.position = Position::TopOfFocusedChart;
    p.position_offsets[4] = 23;
    p.width_offsets[5] = 33;
    p.high[3] = 0.4;
    p.background[0] = 0.25;
    p.color_mode = ColorMode::ColorByDensity;
    f.set_density(p).unwrap();
    f.save().unwrap();
    let mut s = State::open(&song, 0).unwrap();
    s.configure_audio_preferences(&preferences).unwrap();
    settings(&mut s);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::ArrowLeft);
    press(&mut s, KeyCode::Escape);
    assert_eq!(prefs(&s), p);
    assert_eq!(File::open(&preferences).unwrap().density(), p);
    settings(&mut s);
    down(&mut s, 2);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(prefs(&s), Default::default());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(prefs(&s), p);
    assert_eq!(serialized(&s), SOURCE);
}
#[test]
fn source_density_mesh_is_finite_readonly_and_rssp_analysis_remains_accessible() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    let preferences = dir.path().join("preferences.json");
    fs::write(&song, SOURCE).unwrap();
    let mut s = State::open(&song, 0).unwrap();
    s.configure_audio_preferences(&preferences).unwrap();
    finish_density(&mut s);
    let r = s.source_density_report().expect("source density finished");
    assert_eq!(r.series.measures[0].steps, 3);
    assert_eq!(r.series.measures[0].rows, 2);
    assert_eq!(r.series.measures[1].steps, 1);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let meshes: Vec<_> = actors
        .iter()
        .filter_map(|a| {
            if let deadlib_present::actors::Actor::Mesh {
                vertices,
                offset,
                z,
                ..
            } = a
            {
                if *z == 8005 && offset[0] == 10.0 {
                    Some(vertices)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    assert!(!meshes.is_empty());
    assert!(meshes.iter().all(|v| {
        v.len() <= 12288
            && v.iter()
                .all(|v| v.pos.iter().chain(v.color.iter()).all(|v| v.is_finite()))
    }));
    assert_eq!(serialized(&s), SOURCE);
    press(&mut s, KeyCode::F1);
    down(&mut s, 14);
    press(&mut s, KeyCode::Enter);
    assert!(s.is_analyzing());
    press(&mut s, KeyCode::F6);
    assert!(contains(&s, "DensitySettingsHelp"));
    assert!(s.source_density_report().is_some());
    assert_eq!(serialized(&s), SOURCE);
}
