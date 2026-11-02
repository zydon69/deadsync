use super::*;
fn press(s:&mut State,code:KeyCode){for pressed in[true,false]{s.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:std::time::Instant::now(),host_nanos:0});}}
#[test]fn source_center_and_axis_locks_share_render_key_and_precise_entry_gates_en_fr(){
 for language in["en","fr"]{i18n::init_for_tests();i18n::set_locale(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let song=root.path().join("song.ssc");std::fs::write(&song,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();let mut s=State::open(&song,0).unwrap();let source=s.workspace.file().session.serialize().unwrap();let revision=s.workspace.file().session.revision();
 for center in[false,true]{for lock_x in[false,true]{for lock_y in[false,true]{for allow_header in[false,true]{let mut p=Preferences::default();p.placement.center=center;p.placement.lock_x=lock_x;p.placement.lock_y=lock_y;p.placement.allow_header=allow_header;
  for (index,active)in[(11,!lock_x),(12,!lock_x&&!center),(13,!lock_y),(17,!center&&(!lock_x||allow_header))]{
   s.configure_receptor_preferences(p);s.open_receptor_settings();s.panel=Some(Panel::ReceptorSettings(index));assert_eq!(s.receptor_control_enabled(index),active);
   let value=match index{11=>i18n::tr("Edition",if center{"WaveformYes"}else{"WaveformNo"}).to_string(),12=>p.placement.x.to_string(),13=>p.placement.y.to_string(),_=>String::new()};let base=if index==17{i18n::tr("Edition",KEYS[index])}else{i18n::tr_fmt("Edition",KEYS[index],&[("value",&value)])};let expected=if active{base}else{i18n::tr_fmt("Edition","ReceptorControlInactive",&[("value",&base)])};let mut actors=Vec::new();s.receptor_settings_actors(index,&mut actors);assert!(actors.iter().any(|actor|matches!(actor,Actor::Text{content,color,..}if content.as_str()==expected.as_ref()&&(active||*color==[0.5,0.5,0.5,1.]))));
   if !active{for code in[KeyCode::ArrowLeft,KeyCode::ArrowRight,KeyCode::Home,KeyCode::Enter]{press(&mut s,code);assert_eq!(s.receptor_draft,Some(p));assert!(s.preference_numeric.is_none());}assert_eq!(s.status,i18n::tr("Edition","ReceptorControlUnavailable"));}
   else if matches!(index,12|13){press(&mut s,KeyCode::Enter);assert!(s.preference_numeric.is_some(),"active source coordinate must permit precise input");press(&mut s,KeyCode::Escape);assert!(s.preference_numeric.is_none());}
   assert_eq!(s.workspace.file().session.serialize().unwrap(),source);assert_eq!(s.workspace.file().session.revision(),revision);s.receptor_draft=None;s.panel=None;
  }
 }}}}
 }}
