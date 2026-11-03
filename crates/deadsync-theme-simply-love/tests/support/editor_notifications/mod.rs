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
fn open(s: &mut State) {
    press(s, KeyCode::F1);
    press(s, KeyCode::ArrowUp);
    press(s, KeyCode::Enter);
}
fn texts(actors:&[Actor])->Vec<String>{let mut result=Vec::new();for a in actors{match a{Actor::Text{content,..}=>result.push(content.as_str().to_owned()),Actor::Frame{children,..}=>result.extend(texts(children)),Actor::SharedFrame{children,..}|Actor::SharedTransform{children,..}=>result.extend(texts(children)),_=>{}}}result}
fn fixture(scale:f64)->(tempfile::TempDir,std::path::PathBuf,std::path::PathBuf){let root=tempfile::tempdir().unwrap();let song=root.path().join("song.ssc");std::fs::write(&song,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();let prefs=root.path().join("prefs.json");std::fs::write(&prefs,serde_json::to_vec(&serde_json::json!({"Version":1,"LastCompletedFtueVersion":"0.1.0","FtueIndex":6,"Options":{"UseCustomDpiScale":true,"DpiScale":scale,"Future":17},"KeyBinds":{"Undo":[["F14"]],"Redo":[["F15"]]}})).unwrap()).unwrap();(root,song,prefs)}
#[test]
fn update_notice_is_once_per_app_and_suppression_is_persisted_undoable_without_touching_song(){
 i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.,480.));
 let(_root,path,prefs)=fixture(1.);let mut state=State::open_with_preferences(&path,0,&prefs).unwrap();let before=state.workspace().file().session.serialize().unwrap();
 state.offer_available_update("v999.0.0","");let mut actors=Vec::new();state.actors(&mut actors);assert!(!texts(&actors).iter().any(|s|s.contains("v999.0.0")));
 state.offer_available_update("v999.0.0","https://github.com/owner/repo/releases/tag/v999.0.0");actors.clear();state.actors(&mut actors);assert!(texts(&actors).iter().any(|s|s.contains("v999.0.0")));assert!(state.take_notification_url().is_none());
 press(&mut state,KeyCode::ArrowDown);press(&mut state,KeyCode::Enter);assert_eq!(state.take_notification_url().as_deref(),Some("https://github.com/owner/repo/releases/tag/v999.0.0"));assert!(state.take_notification_url().is_none());
 press(&mut state,KeyCode::ArrowDown);press(&mut state,KeyCode::Enter);assert!(state.general_options().suppress_update);let stored:serde_json::Value=serde_json::from_slice(&std::fs::read(&prefs).unwrap()).unwrap();assert_eq!(stored["Options"]["SuppressUpdateNotification"],true);assert_eq!(stored["Options"]["Future"],17);
 press(&mut state,KeyCode::Escape);press(&mut state,KeyCode::F14);assert!(!state.general_options().suppress_update);press(&mut state,KeyCode::F15);assert!(state.general_options().suppress_update);press(&mut state,KeyCode::F14);
 let mut next=State::open_with_preferences(&path,0,&prefs).unwrap();next.offer_available_update("v1000.0.0","https://github.com/owner/repo/releases");actors.clear();next.actors(&mut actors);assert!(!texts(&actors).iter().any(|s|s.contains("v1000.0.0")));
 assert_eq!(state.workspace().file().session.serialize().unwrap(),before);assert!(!state.workspace().file().session.is_dirty());
}
