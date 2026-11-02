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
const SOURCE:&[u8]=b"#VERSION:0.83;#TITLE:Dark;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1100\n1000\n0000\n0000,\n1000;";
fn down(s: &mut State, n: usize) {
    for _ in 0..n {
        press(s, KeyCode::ArrowDown);
    }
}
fn settings(s: &mut State) {
    press(s, KeyCode::F1);
    down(s, 27);
    press(s, KeyCode::Enter);
}
fn contains(s: &State, key: &str) -> bool {
    let mut a = Vec::new();
    s.actors(&mut a);
    let text = i18n::tr("Edition", key);
    a.iter().any(|a|matches!(a,deadlib_present::actors::Actor::Text{content,..} if content.as_ref()==text.as_ref()))
}
fn state(path: &std::path::Path, prefs: &std::path::Path) -> State {
    fs::write(path, SOURCE).unwrap();
    let mut s = State::open(path, 0).unwrap();
    s.configure_audio_preferences(prefs).unwrap();
    s
}
#[test]
fn dark_keyboard_save_persist_undo_redo_preserves_chart() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    let path = dir.path().join("prefs.json");
    let mut s = state(&song, &path);
    settings(&mut s);
    assert!(contains(&s, "DarkHelp"));
    press(&mut s, KeyCode::ArrowRight);
    down(&mut s, 8);
    press(&mut s, KeyCode::Enter);
    assert!(s.dark_preferences().show());
    assert!(
        deadsync_editor::audio_preferences::File::open(&path)
            .unwrap()
            .dark()
            .show()
    );
    assert!(!s.workspace().file().session.is_dirty());
    assert_eq!(serialized(&s), SOURCE);
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(!s.dark_preferences().show());
    ctrl(&mut s, KeyCode::KeyY);
    assert!(s.dark_preferences().show());
    let mut reopened = State::open(&song, 0).unwrap();
    reopened.configure_audio_preferences(&path).unwrap();
    assert_eq!(reopened.dark_preferences(), s.dark_preferences());
    assert_eq!(fs::read(&song).unwrap(), SOURCE);
}
#[test]
fn dark_preview_cancel_and_defaults_are_undoable() {
    use deadsync_editor::dark_preferences::{Order, Preferences, Size};
    init();
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    let path = dir.path().join("prefs.json");
    let mut f = deadsync_editor::audio_preferences::File::open(&path).unwrap();
    let mut p = Preferences::default();
    p.set_show(true);
    p.set_size(Size::Charts);
    p.set_order(Order::AfterWaveForm);
    p.set_color([0.2, 0.3, 0.4, 0.5]).unwrap();
    f.set_dark(p).unwrap();
    f.save().unwrap();
    let mut s = state(&song, &path);
    settings(&mut s);
    press(&mut s, KeyCode::ArrowRight);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.dark_preferences(), p);
    assert_eq!(
        deadsync_editor::audio_preferences::File::open(&path)
            .unwrap()
            .dark(),
        p
    );
    settings(&mut s);
    down(&mut s, 7);
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.dark_preferences(), Preferences::default());
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(s.dark_preferences(), p);
    assert_eq!(serialized(&s), SOURCE);
}
#[test]
fn dark_overlay_order_size_and_clamping_preserve_note_layers() {
    use deadlib_present::actors::{Actor, SizeSpec, SpriteSource};
    use deadsync_editor::dark_preferences::{Order, Preferences, Size};
    init();
    let dir = tempfile::tempdir().unwrap();
    let song = dir.path().join("song.ssc");
    let path = dir.path().join("prefs.json");
    let mut s = state(&song, &path);
    // Solid sprites use a native1x1 base when SizeSpec::Px is0; zoomto
    // carries the requested rendered dimensions in Actor.scale.
    let color = [1.0, 0.0, 0.25, 0.5];
    let mut p = Preferences::default();
    p.set_show(true);
    p.set_color([2.0, -1.0, 0.25, 0.5]).unwrap();
    for (order, z) in [(Order::AfterBackground, 8000), (Order::AfterWaveForm, 8001)] {
        p.set_order(order);
        s.configure_dark_preferences(p);
        let mut actors = Vec::new();
        s.actors(&mut actors);
        let overlay=actors.iter().position(|a| matches!(a,Actor::Sprite{source:SpriteSource::Solid,tint,z:az,..} if *tint==color && *az==z)).expect("overlay");
        assert!(
            matches!(&actors[overlay],Actor::Sprite{offset,size,scale,..} if *offset==[427.0,240.0] && matches!(size,[SizeSpec::Px(w),SizeSpec::Px(h)] if (if *w==0.0 {1.0}else{*w})*scale[0]==854.0 && (if *h==0.0 {1.0}else{*h})*scale[1]==480.0))
        );
        if order == Order::AfterWaveForm {
            // Grid lines share waveform z but are deliberately appended after
            // the overlay, keeping chart guides visible above the dark layer.
            let grid:Vec<_>=actors.iter().enumerate().filter_map(|(i,a)|
                matches!(a,Actor::Sprite{z:8001,tint,..} if *tint==[0.12,0.16,0.22,1.0]).then_some(i)).collect();
            assert!(!grid.is_empty());
            assert!(grid.iter().all(|i|*i>overlay));
        }
    }
    p.set_size(Size::Charts);
    s.configure_dark_preferences(p);
    let mut a = Vec::new();
    s.actors(&mut a);
    assert!(a.iter().any(|a|matches!(a,Actor::Sprite{source:SpriteSource::Solid,tint,size,scale,..} if *tint==color && matches!(size,[SizeSpec::Px(w),_] if (if *w==0.0 {1.0}else{*w})*scale[0]>0.0 && (if *w==0.0 {1.0}else{*w})*scale[0]<854.0))));
    assert_eq!(serialized(&s), SOURCE);
    assert!(!s.workspace().file().session.is_dirty());
}
