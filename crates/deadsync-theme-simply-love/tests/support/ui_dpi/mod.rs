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
fn fixture(scale:f64)->(tempfile::TempDir,std::path::PathBuf,std::path::PathBuf){let root=tempfile::tempdir().unwrap();let song=root.path().join("song.ssc");std::fs::write(&song,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();let prefs=root.path().join("prefs.json");std::fs::write(&prefs,serde_json::to_vec(&serde_json::json!({"Version":1,"Options":{"UseCustomDpiScale":true,"DpiScale":scale,"Future":17},"KeyBinds":{"Undo":[["F14"]],"Redo":[["F15"]]}})).unwrap()).unwrap();(root,song,prefs)}
#[test]
fn candidates_are_inactive_real_host_captures_once_and_dpi_edits_wait_for_restart_with_undo(){
 i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.,480.));
 let(_unused,path,prefs)=fixture(4.);let candidate=State::open_with_preferences(&path,0,&prefs).unwrap();assert_eq!(candidate.active_ui_dpi(),1.);drop(candidate);
 let(_root,path,prefs)=fixture(2.);let mut state=State::open_with_preferences(&path,0,&prefs).unwrap();let bytes=state.workspace().file().session.serialize().unwrap();let mode=state.spacing_mode();let reversed=state.display_reversed();state.bind_ui_dpi(1.).unwrap();assert_eq!(state.active_ui_dpi(),2.);open(&mut state);for _ in 0..8{press(&mut state,KeyCode::ArrowRight);}press(&mut state,KeyCode::Tab);press(&mut state,KeyCode::ArrowRight);assert!((state.general_options().dpi_scale-2.01).abs()<1e-9);assert_eq!(state.active_ui_dpi(),2.);assert!(state.ui_dpi_pending_restart());let mut actors=Vec::new();state.actors(&mut actors);assert!(texts(&actors).iter().any(|s|s.contains(&*i18n::tr("Edition","UiDpiRestartRequired"))));assert_eq!((deadlib_present::space::screen_width(),deadlib_present::space::screen_height()),(854.,480.));assert!(actors.iter().any(|a|matches!(a,Actor::SharedTransform{..})));// Retina projection already supplies two physical pixels per canvas unit.
 state.set_ui_dpi_viewport(1708,960);let mut retina=Vec::new();state.actors(&mut retina);assert!(!retina.iter().any(|a|matches!(a,Actor::SharedTransform{source_view_proj,..}if *source_view_proj==glam::Mat4::IDENTITY)));assert_eq!(state.active_ui_dpi(),2.);
 // A different canvas and a nonuniform physical projection use independent axes.
 deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(960.,540.));state.set_ui_dpi_viewport(1920,1080);let mut resized=Vec::new();state.actors(&mut resized);assert!(!resized.iter().any(|a|matches!(a,Actor::SharedTransform{source_view_proj,..}if *source_view_proj==glam::Mat4::IDENTITY)));
 state.set_ui_dpi_viewport(1440,1080);let mut stretched=Vec::new();state.actors(&mut stretched);assert!(stretched.iter().any(|a|matches!(a,Actor::SharedTransform{transform,source_view_proj,..}if *source_view_proj==glam::Mat4::IDENTITY&&(transform.x_axis.x-4./3.).abs()<1e-6&&(transform.y_axis.y-1.).abs()<1e-6)));
 deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.,480.));state.set_ui_dpi_viewport(854,480);
 press(&mut state,KeyCode::Escape);press(&mut state,KeyCode::F14);assert_eq!(state.general_options().dpi_scale,2.);assert!(!state.ui_dpi_pending_restart());press(&mut state,KeyCode::F15);assert!(state.ui_dpi_pending_restart());let mut reopened=State::open_with_preferences(&path,0,&prefs).unwrap();reopened.bind_ui_dpi(3.).unwrap();assert_eq!(reopened.active_ui_dpi(),2.);assert!(reopened.ui_dpi_pending_restart());assert_eq!(state.spacing_mode(),mode);assert_eq!(state.display_reversed(),reversed);assert_eq!(state.workspace().file().session.serialize().unwrap(),bytes);assert!(!state.workspace().file().session.is_dirty());let json:serde_json::Value=serde_json::from_slice(&std::fs::read(prefs).unwrap()).unwrap();assert_eq!(json["Options"]["Future"],17);
}
