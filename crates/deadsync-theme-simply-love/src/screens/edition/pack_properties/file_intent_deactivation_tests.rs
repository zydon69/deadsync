use super::*;
use std::fs;
fn fixture()->(tempfile::TempDir,State,PathBuf){
 i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let song=root.path().join("Song");fs::create_dir(&song).unwrap();let path=song.join("song.ssc");
 fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let pack=root.path().join("Pack.ini");fs::write(&pack,Data::new("Original","",2026,None,0.009).canonical().unwrap()).unwrap();
 let mut s=State::open(&path,0).unwrap();s.open_pack_properties();s.pack_properties.as_mut().unwrap().set_text(Field::Title,"Cached before departure").unwrap();s.pack_properties.as_mut().unwrap().index=8;s.panel=None;s.text_buffer=None;(root,s,pack)
}
#[test]
fn close_and_exit_capture_the_deactivated_field_but_keep_explicit_save_cancel_discard(){
 for exit in [false,true]{for choice in [0,1,2]{
  let(_root,mut s,pack)=fixture();let disk=fs::read(&pack).unwrap();let before=deadsync_editor::history::file_intent::IntentGuard::capture(&s.workspace.file().session);
  if exit {assert!(matches!(s.request_application_exit(),super::super::ApplicationExit::Pending));} else {assert!(!s.request_close());}
  assert!(!before.is_current(&s.workspace.file().session),"actual field deactivation created an ordinary application action");
  assert_eq!(fs::read(&pack).unwrap(),disk,"deactivation never saves implicitly");
  assert!(matches!(s.panel,Some(Panel::FileUnsaved(0))));
  let ready=s.file_menu_key(choice,KeyCode::Enter);
  match choice{0=>{assert!(!ready);assert_eq!(fs::read(&pack).unwrap(),disk);assert!(s.pack_has_unsaved_changes());},1=>{if exit{assert!(matches!(s.take_application_exit(),Some(super::super::ApplicationExit::Ready)));}else{assert!(ready);}assert_ne!(fs::read(&pack).unwrap(),disk);},_=>{assert!(!ready);assert!(matches!(s.panel,Some(Panel::FileDiscard)));s.file_menu_key(0,KeyCode::Enter);assert_eq!(fs::read(&pack).unwrap(),disk);}}
 }}
}
#[test]
fn invalid_or_stale_deactivation_refuses_before_capturing_departure_or_publishing_history(){
 for external in [false,true]{let(_root,mut s,pack)=fixture();if external{fs::write(&pack,b"external bytes").unwrap();}else{assert!(s.pack_properties.as_mut().unwrap().set_text(Field::Year,"not a year").is_err());}
 let before=s.workspace.file().session.serialize().unwrap();let guard=deadsync_editor::history::file_intent::IntentGuard::capture(&s.workspace.file().session);let disk=fs::read(&pack).unwrap();assert!(!s.request_close());assert!(!matches!(s.panel,Some(Panel::FileUnsaved(_))));assert!(guard.is_current(&s.workspace.file().session));assert_eq!(s.workspace.file().session.serialize().unwrap(),before);assert_eq!(fs::read(&pack).unwrap(),disk);assert!(s.pack_has_unsaved_changes());}
}
