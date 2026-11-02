use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::time::Instant;
fn press(s: &mut State, code: KeyCode) {
    for pressed in [true, false] {
        s.raw_key(&RawKeyboardEvent {
            code,
            pressed,
            repeat: false,
            timestamp: Instant::now(),
            host_nanos: 0,
        });
    }
}
#[test]
fn source_speed_controls_apply_persist_without_undo_and_reverse_undoes_only_its_flag() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let d = tempfile::tempdir().unwrap();
    let song = d.path().join("song.ssc");
    std::fs::write(&song,"#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let prefs = d.path().join("prefs.json");
    std::fs::write(&prefs,r#"{"Version":1,"Scroll":{"SpacingMode":"ConstantTime","TimeBasedPixelsPerSecond":450,"RowBasedPixelsPerRow":3,"VariablePixelsPerSecondAtDefaultBPM":600},"KeyBinds":{"OpenScrollSettings":[["F13"]],"Undo":[["F14"]],"Redo":[["F15"]]}}"#).unwrap();
    let mut s = State::open(&song, 0).unwrap();
    s.configure_audio_preferences(&prefs).unwrap();
    let begin = s.take_audio_commands();
    assert_eq!(begin.iter().filter(|c|matches!(c,deadsync_gameplay::GameplayAudioCommand::EditorBeginAudio{owner,..} if *owner!=0)).count(),1);
    s.update_audio(Default::default());
    s.take_audio_commands();
    let initial = s.workspace().file().session.undo_label().map(str::to_owned);
    let source = s.workspace().file().session.serialize().unwrap();
    s.configure_spacing_preference(
        deadsync_editor::scroll_preferences::spacing::Preference::ConstantTime,
    );
    press(&mut s, KeyCode::F13);
    press(&mut s, KeyCode::F12);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let label = i18n::tr_fmt("Edition", "ScrollTimeSpeed", &[("value", "450.000")]);
    assert!(
        actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..} if content.as_str()==&*label)),
        "expected {label}; text actors: {:?}",
        actors
            .iter()
            .filter_map(|a| match a {
                Actor::Text { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
    );
    for (index, value) in [(0, 460_f64), (1, 3.1), (2, 610.)] {
        // Application preference publication is the authoritative mode owner;
        // direct view configuration would be restored by the next input barrier.
        let mut preferences: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();
        preferences["Scroll"]["SpacingMode"] =
            ["ConstantTime", "ConstantRow", "Variable"][index].into();
        std::fs::write(&prefs, serde_json::to_vec(&preferences).unwrap()).unwrap();
        s.configure_audio_preferences(&prefs).unwrap();
        press(&mut s, KeyCode::ArrowRight);
        assert!((s.scroll_preferences().speed(index).unwrap() - value).abs() < 1e-10);
        assert_eq!(
            s.workspace().file().session.undo_label().map(str::to_owned),
            initial
        );
        press(&mut s, KeyCode::Tab);
    }
    press(&mut s, KeyCode::ArrowRight);
    assert!(s.display_reversed());
    assert!(s.workspace().file().session.undo_label().is_some());
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::F14);
    assert!(!s.display_reversed());
    assert_eq!(s.scroll_preferences().speed(0), Some(460.));
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    let mut reopened = State::open(&song, 0).unwrap();
    reopened.configure_audio_preferences(&prefs).unwrap();
    assert_eq!(reopened.scroll_preferences(), s.scroll_preferences());
    assert!(!reopened.display_reversed());
}
