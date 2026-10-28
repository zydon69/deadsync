use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadsync_editor::notes::{Kind, Position};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent { code, pressed, repeat: false, timestamp: Instant::now(), host_nanos: 0 });
}
fn press(s: &mut State, code: KeyCode) { key(s,code,true);key(s,code,false); }
fn ctrl(s: &mut State, code: KeyCode) {key(s,KeyCode::SuperLeft,true);press(s,code);key(s,KeyCode::SuperLeft,false);}
fn init() {
    if super::FRENCH { i18n::init(deadsync_assets::language::load_for_tests("fr")); } else {i18n::init_for_tests();}
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(854.0,480.0));
}
fn text(s: &State, key: &str) -> bool {
    let mut actors=Vec::new();s.actors(&mut actors);
    let expected=i18n::tr("Edition",key);
    actors.iter().any(|a| matches!(a,deadlib_present::actors::Actor::Text{content,..} if content.as_ref()==expected.as_ref()))
}
fn serialized(s: &State)->Vec<u8>{s.workspace().file().session.serialize().unwrap()}
fn open_swap(s:&mut State,pair:usize){
    press(s,KeyCode::F1);for _ in 0..4{press(s,KeyCode::ArrowDown);}press(s,KeyCode::Enter);
    for _ in 0..9{press(s,KeyCode::ArrowDown);}press(s,KeyCode::Enter);
    for _ in 0..5{press(s,KeyCode::ArrowDown);}for _ in 0..pair{press(s,KeyCode::ArrowRight);}
}
#[test]
fn simultaneous_note_swap_undo_redo_and_save_reload(){
    init();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("swap.ssc");
    let source=b"#VERSION:0.83;#TITLE:Swap;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1M00;";
    fs::write(&path,source).unwrap();let mut s=State::open(&path,0).unwrap();
    open_swap(&mut s,0);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);
    let p=|lane|Position{row:0,lane,player:0};
    assert_eq!(s.workspace().grid().get(p(0)).unwrap().kind,Kind::Mine);
    assert_eq!(s.workspace().grid().get(p(1)).unwrap().kind,Kind::Tap);
    let after=serialized(&s);ctrl(&mut s,KeyCode::KeyZ);assert_eq!(serialized(&s),source);
    ctrl(&mut s,KeyCode::KeyY);assert_eq!(serialized(&s),after);ctrl(&mut s,KeyCode::KeyS);
    let reopened=State::open(&path,0).unwrap();assert_eq!(serialized(&reopened),after);
}
#[test]
fn unrepresentable_note_swap_is_atomic_and_escape_preserves_redo(){
    init();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("swap.ssc");
    let source=b"#VERSION:0.83;#TITLE:Swap;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1200\n0300;";
    fs::write(&path,source).unwrap();let mut s=State::open(&path,0).unwrap();
    open_swap(&mut s,6);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);
    assert_eq!(serialized(&s),source);assert!(text(&s,"NoteSwapFailed"));
    assert!(!s.workspace().file().session.is_dirty());assert!(s.workspace().file().session.undo_label().is_none());
    press(&mut s,KeyCode::Escape);press(&mut s,KeyCode::Escape);assert_eq!(fs::read(path).unwrap(),source);
}
fn open_timing(s:&mut State){press(s,KeyCode::F10);press(s,KeyCode::F7);press(s,KeyCode::ArrowDown);press(s,KeyCode::ArrowRight);press(s,KeyCode::ArrowDown);press(s,KeyCode::Enter);}
#[test]
fn timing_conversion_review_cancel_apply_undo_and_save(){
    init();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("timing.ssc");
    let source=b"#VERSION:0.83;#TITLE:Timing;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=60;#WARPS:4=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path,source).unwrap();let mut s=State::open(&path,0).unwrap();
    open_timing(&mut s);assert_eq!(serialized(&s),source);press(&mut s,KeyCode::Escape);press(&mut s,KeyCode::Escape);assert_eq!(serialized(&s),source);
    open_timing(&mut s);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);
    assert_eq!(s.workspace().file().session.document().text(None,"STOPS").as_deref(),Some("4=-1"));
    assert!(text(&s,"TimingConversionApplied"));let after=serialized(&s);ctrl(&mut s,KeyCode::KeyZ);assert_eq!(serialized(&s),source);
    ctrl(&mut s,KeyCode::KeyY);assert_eq!(serialized(&s),after);ctrl(&mut s,KeyCode::KeyS);assert_eq!(fs::read(path).unwrap(),after);
}
#[test]
fn timing_collision_requires_individual_acknowledgement_before_deletion(){
    init();let dir=tempfile::tempdir().unwrap();let path=dir.path().join("timing.ssc");
    let source=b"#VERSION:0.83;#TITLE:Timing;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=60;#WARPS:4=1,8=1;#STOPS:4=2,8=3;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    fs::write(&path,source).unwrap();let mut s=State::open(&path,0).unwrap();open_timing(&mut s);
    press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);assert_eq!(serialized(&s),source);
    press(&mut s,KeyCode::ArrowUp);press(&mut s,KeyCode::Enter);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);assert_eq!(serialized(&s),source);
    press(&mut s,KeyCode::ArrowUp);press(&mut s,KeyCode::Enter);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);
    assert_eq!(s.workspace().file().session.document().text(None,"WARPS").as_deref(),Some(""));
    assert_eq!(s.workspace().file().session.document().text(None,"STOPS").as_deref(),Some("4=2,8=3"));
    ctrl(&mut s,KeyCode::KeyZ);assert_eq!(serialized(&s),source);
}
