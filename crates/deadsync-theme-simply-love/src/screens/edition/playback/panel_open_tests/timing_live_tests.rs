use super::*;
fn entry(s:&mut State,index:usize){
    let Some(Panel::Timing(mut current))=s.panel else{panic!("timing panel required: {}",s.status)};
    while current!=index{press(s,KeyCode::ArrowDown);current=(current+1)%11;}
}
fn position(s:&State)->(f64,f64){let p=s.focus_position.as_ref().expect("precise focus");(p.row(),p.music_time())}
fn lease(s:&State)->(u64,Option<u64>,Option<u64>,Option<u64>){let p=s.playback.as_ref().unwrap();(s.editor_asset_owner(),p.clock_generation,p.issued_generation,p.asset_epoch)}
fn no_transport_restart(s:&mut State){for command in s.take_audio_commands(){assert!(!matches!(command,GameplayAudioCommand::EditorStartMusic{..}|GameplayAudioCommand::EditorStopMusic{..}|GameplayAudioCommand::EditorPauseMusic{..}|GameplayAudioCommand::EditorResumeMusic{..}|GameplayAudioCommand::EditorPlayPreview{..}|GameplayAudioCommand::EditorSuspendPreview{..}|GameplayAudioCommand::EditorResumePreview{..}|GameplayAudioCommand::SeekMusic(_)|GameplayAudioCommand::EditorSeekMusic{..}),"unexpected {command:?}");}}
#[test]
fn physical_tempo_commit_cancel_undo_redo_preserve_main_time_pause_row_and_preview_callback(){
 deadlib_present::space::with_metrics(deadlib_present::space::Metrics::centered(1100.,760.),||{
  for mode in 0..3 {
   let(_root,path,mut s)=fixture(false,false);start(&mut s,mode==2);
   if mode==1{s.pause_audio();}
   if mode!=0{let focus=deadsync_editor::chart_focus::Position::at_fractional_row(&s.workspace.file().session,0,60.25).unwrap();s.workspace.seek_preserving_selection(focus.edit_row()).unwrap();if mode==1{let paused=s.paused_playback.as_mut().unwrap();paused.edit_row=focus.edit_row();paused.position=Position{row:focus.row(),chart_time:focus.chart_time(),music_time:focus.music_time()};}s.focus_position=Some(focus);}
   let before=s.workspace.file().session.serialize().unwrap();let disk=fs::read(&path).unwrap();
   let anchor=s.capture_timing_presentation().unwrap();let oldrow=anchor.row();let oldtime=anchor.music_time();
   let retained=if mode!=1{Some(lease(&s))}else{None};let sample=if mode==2{Some(witness(&s).audio)}else{None};let pause_origin=s.paused_playback.as_ref().and_then(|p|p.intent_origin);
   s.take_audio_commands();bind(&mut s,Action::OpenTiming);press(&mut s,KeyCode::F12);assert!(matches!(s.panel,Some(Panel::Timing(0))));
   entry(&mut s,2);press(&mut s,KeyCode::ArrowRight);entry(&mut s,9);press(&mut s,KeyCode::Enter);assert_eq!(s.workspace.file().session.serialize().unwrap(),before,"Cancel");no_transport_restart(&mut s);
   press(&mut s,KeyCode::F12);entry(&mut s,1);press(&mut s,KeyCode::Digit0);press(&mut s,KeyCode::Enter);entry(&mut s,2);press(&mut s,KeyCode::ArrowRight);entry(&mut s,8);press(&mut s,KeyCode::Enter);assert!(s.panel.is_none(),"{}",s.status);
   let after=s.workspace.file().session.serialize().unwrap();assert_ne!(after,before);assert_eq!(fs::read(&path).unwrap(),disk);
   for (step,redo,bytes) in [("Apply",None,&after),("Undo",Some(false),&before),("Redo",Some(true),&after)] {
    if let Some(redo)=redo{bind(&mut s,if redo{Action::Redo}else{Action::Undo});press(&mut s,KeyCode::F12);}
    assert_eq!(&s.workspace.file().session.serialize().unwrap(),bytes,"{step}");let(row,time)=position(&s);
    if mode==0{assert_eq!(time.to_bits(),oldtime.to_bits(),"{step}");assert!(s.chart_transport_is_playing());let p=s.playback.as_ref().unwrap();assert_eq!(p.preview.as_ref().unwrap().position(time).unwrap(),row);}
    else{assert_eq!(row.to_bits(),oldrow.to_bits(),"{step}");}
    if mode==1{assert!(s.paused_position().is_some(),"{step}");assert_eq!(s.paused_playback.as_ref().unwrap().intent_origin,pause_origin);}
    if let Some(expected)=retained{assert_eq!(lease(&s),expected,"{step}");}
    if let Some(expected)=&sample{assert_eq!(&witness(&s).audio,expected,"audition {step}");}
    no_transport_restart(&mut s);
   }
  }
 });
}#[test]
fn physical_neighbor_navigation_uses_scroll_policy_and_retargets_main_instead_of_stale_cursor(){
 for stop in [false,true] {
  let(_root,path,state)=fixture(false,false);drop(state);let text=fs::read_to_string(&path).unwrap().replace("#BPMS:0=120;","#BPMS:0=120,4=180;");fs::write(&path,text).unwrap();let mut s=State::open(&path,0).unwrap();start(&mut s,false);s.scroll_preferences.set_stop_playback(stop);s.take_audio_commands();
  bind(&mut s,Action::OpenTiming);press(&mut s,KeyCode::F12);entry(&mut s,7);press(&mut s,KeyCode::Enter);assert_eq!(s.workspace.row(),192);assert_eq!(s.focus_position.as_ref().unwrap().row(),192.);
  assert_eq!(s.chart_transport_is_playing(),!stop);
  let owner=s.editor_asset_owner();let commands=s.take_audio_commands();
  let seeks:Vec<_>=commands.iter().filter_map(|command|match command{GameplayAudioCommand::EditorSeekMusic{owner,seconds}=>Some((*owner,*seconds)),_=>None}).collect();
  if stop {
   assert!(seeks.is_empty(),"{commands:?}");
   assert!(commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorPauseMusic{owner:actual,..} if *actual==owner)),"{commands:?}");
  }else{
   assert_eq!(seeks,vec![(owner,2.0+s.audio_preferences.effective().offset)],"{commands:?}");
   assert!(!commands.iter().any(|command|matches!(command,GameplayAudioCommand::EditorStartMusic{..}|GameplayAudioCommand::EditorStopMusic{..}|GameplayAudioCommand::EditorPauseMusic{..})),"{commands:?}");
  }
  assert!(!commands.iter().any(|command|matches!(command,GameplayAudioCommand::SeekMusic(_))),"published seek must carry its owner");
 }
}
#[test]
fn stale_timing_apply_keeps_review_and_transport_without_publishing_another_history_action(){
 let(_root,_path,mut s)=fixture(false,false);start(&mut s,false);bind(&mut s,Action::OpenTiming);press(&mut s,KeyCode::F12);entry(&mut s,2);press(&mut s,KeyCode::ArrowRight);
 s.workspace.execute(Command::Place(2)).unwrap();let bytes=s.workspace.file().session.serialize().unwrap();let revision=s.workspace.file().session.revision();let retained=lease(&s);s.take_audio_commands();entry(&mut s,8);press(&mut s,KeyCode::Enter);assert!(matches!(s.panel,Some(Panel::Timing(8))));assert_eq!(s.workspace.file().session.serialize().unwrap(),bytes);assert_eq!(s.workspace.file().session.revision(),revision);assert_eq!(lease(&s),retained);no_transport_restart(&mut s);
}
