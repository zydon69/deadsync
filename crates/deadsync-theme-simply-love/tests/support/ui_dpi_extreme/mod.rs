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
fn texts(actors:&[Actor])->Vec<String>{let mut result=Vec::new();for a in actors{match a{Actor::Text{content,..}=>result.push(content.as_str().to_owned()),Actor::Frame{children,..}=>result.extend(texts(children)),Actor::SharedFrame{children,..}|Actor::SharedTransform{children,..}=>result.extend(texts(children)),_=>{}}}result}
fn ui_offset(actors:&[Actor])->[f32;2]{actors.iter().find_map(|a|if let Actor::SharedTransform{children,..}=a{children.iter().find_map(|a|if let Actor::Frame{offset,..}=a{Some(*offset)}else{None})}else{None}).expect("scaled UI Frame")}
#[test]
fn dpi_eight_pages_every_control_and_horizontal_navigation_works_without_changing_song(){
 i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.,480.));let root=tempfile::tempdir().unwrap();let prefs=root.path().join("prefs.json");std::fs::write(&prefs,r#"{"Version":1,"Options":{"UseCustomDpiScale":true,"DpiScale":8}}"#).unwrap();let mut state=State::without_song(root.path()).unwrap();state.configure_audio_preferences(&prefs).unwrap();state.bind_ui_dpi(1.).unwrap();assert_eq!(state.active_ui_dpi(),8.);let bytes=state.workspace().file().session.serialize().unwrap();press(&mut state,KeyCode::F1);press(&mut state,KeyCode::F9);let mut actors=Vec::new();state.actors(&mut actors);let first=texts(&actors);let full=i18n::tr("Edition","GeneralAllDefaults");assert!(!first.iter().any(|v|v==&*full));press(&mut state,KeyCode::ArrowUp);actors.clear();state.actors(&mut actors);assert!(texts(&actors).iter().any(|v|v==&*full));let before=ui_offset(&actors);
 state.raw_key(&RawKeyboardEvent{code:KeyCode::AltLeft,pressed:true,repeat:false,timestamp:Instant::now(),host_nanos:0});press(&mut state,KeyCode::ArrowRight);state.raw_key(&RawKeyboardEvent{code:KeyCode::AltLeft,pressed:false,repeat:false,timestamp:Instant::now(),host_nanos:0});actors.clear();state.actors(&mut actors);assert!(ui_offset(&actors)[0]<before[0]);assert_eq!((deadlib_present::space::screen_width(),deadlib_present::space::screen_height()),(854.,480.));assert_eq!(state.workspace().file().session.serialize().unwrap(),bytes);assert!(state.workspace().file().existing_path().is_none());
 // A shortcut recording remains authoritative, and its review prompt becomes
 // the focused visible row instead of leaving it below the DPI viewport.
 press(&mut state,KeyCode::Escape);press(&mut state,KeyCode::F1);for _ in 0..3{press(&mut state,KeyCode::ArrowDown);}press(&mut state,KeyCode::Enter);for _ in 0..2{press(&mut state,KeyCode::ArrowDown);}press(&mut state,KeyCode::Enter);actors.clear();state.actors(&mut actors);let capture_offset=ui_offset(&actors);state.raw_key(&RawKeyboardEvent{code:KeyCode::AltLeft,pressed:true,repeat:false,timestamp:Instant::now(),host_nanos:0});press(&mut state,KeyCode::ArrowRight);state.raw_key(&RawKeyboardEvent{code:KeyCode::AltLeft,pressed:false,repeat:false,timestamp:Instant::now(),host_nanos:0});actors.clear();state.actors(&mut actors);assert_eq!(ui_offset(&actors)[0],capture_offset[0]);let review=i18n::tr("Edition","KeyBindingReview");let prefix=review.split("{keys}").next().unwrap();assert!(texts(&actors).iter().any(|v|v.starts_with(prefix)));press(&mut state,KeyCode::Enter);press(&mut state,KeyCode::ArrowUp);actors.clear();state.actors(&mut actors);assert!(texts(&actors).iter().any(|v|v.contains("Alt")));
}
