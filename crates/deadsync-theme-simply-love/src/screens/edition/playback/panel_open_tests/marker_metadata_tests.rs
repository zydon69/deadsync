//! Actual PatternAdded inspector B, physical Apply/Delete/Cancel and global history.
use super::*;
use deadsync_editor::{generation::marker_edit::Draft,quick_add::{Plan,Kind}};
fn open(mode:usize)->(tempfile::TempDir,PathBuf,State){
 let(root,path,mut s)=fixture(false,false);let plan=Plan::prepare(&s.workspace.file().session,0,Kind::Pattern,96.).unwrap();s.workspace.apply_quick_add(&plan).unwrap();s.refresh();assert_eq!(s.marker.as_ref().unwrap().start,96,"actual PatternAdded target B");
 s.navigate_exact_position(-0.25,true);
 assert_eq!(s.focus_position.as_ref().unwrap().row(),-0.25,"{}",s.status);
 if mode!=0{start(&mut s,mode==3);if mode!=3{let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=s.editor_asset_owner();snapshot.editor.output=s.audio_output_snapshot();snapshot.editor.logical_music=Some(deadsync_gameplay::EditorLogicalMusicClock{generation:17,asset_epoch:29,music_nanos:627_000_000});snapshot.stream_clock.valid_at=Instant::now();s.update_audio(snapshot);}if mode==2{s.pause_audio();}}
 s.take_audio_commands();s.panel=Some(Panel::Marker(0));(root,path,s)
}
fn press_row(s:&mut State,target:usize){let Some(Panel::Marker(mut row))=s.panel else{panic!("marker expected: {}",s.status)};while row!=target{press(s,KeyCode::ArrowDown);row=(row+1)%16;}}
fn focus(s:&State)->(u64,u64,u64,u64){let presentation=s.capture_song_property_presentation().unwrap();let p=&presentation.focus;assert!(p.is_current(&s.workspace));(s.workspace.row(),p.row().to_bits(),p.music_time().to_bits(),p.chart_time().to_bits())}
fn audio(s:&State)->String{
 if s.is_playing(){witness(s).audio}else if let Some(p)=s.paused_position(){format!("{:?}",(s.editor_asset_owner(),p.row.to_bits(),p.music_time.to_bits(),p.chart_time.to_bits(),s.paused_playback.as_ref().unwrap().intent_origin))}else{"Idle".into()}
}
fn peer(s:&State)->Vec<u8>{let d=s.workspace.file().session.document();d.value(d.charts()[1].notes.clone()).to_vec()}
fn retained(s:&mut State,expected:&str,position:(u64,u64,u64,u64),step:&str){assert_eq!(audio(s),expected,"audio {step}");assert_eq!(focus(s),position,"focus {step}");assert!(s.take_audio_commands().is_empty(),"metadata must not emit audio commands: {step}");}
#[test]fn physical_marker_b_apply_and_delete_retain_a_in_main_pause_preview_and_signed_idle_history(){
 for mode in 0..4{for operation in 0..3{
  let delete=operation==2;
  let(_root,path,mut s)=open(mode);let old=focus(&s);let sound=audio(&s);let before=s.workspace.file().session.serialize().unwrap();let expected_peer=peer(&s);let disk=fs::read(&path).unwrap();
  if delete{press_row(&mut s,7);press(&mut s,KeyCode::Enter);assert!(matches!(s.panel,Some(Panel::ConfirmMarkerDelete)));press(&mut s,KeyCode::Enter);}else{press_row(&mut s,if operation==1{0}else{5});press(&mut s,KeyCode::ArrowRight);press_row(&mut s,6);press(&mut s,KeyCode::Enter);}
  if operation==1{assert!(matches!(s.panel,Some(Panel::Marker(0))),"move callback reopens inspector: {}",s.status);assert_eq!(s.marker.as_ref().unwrap().start,96+deadsync_editor::workspace::snap_edit_rows(s.workspace.snap()));}else{assert!(s.panel.is_none(),"{}",s.status);}retained(&mut s,&sound,old,"Apply/Delete");let after=s.workspace.file().session.serialize().unwrap();assert_ne!(after,before);assert_eq!(peer(&s),expected_peer);assert_eq!(fs::read(path).unwrap(),disk);assert!(s.workspace.file().session.history_preserves_marker_metadata(false));
  for (action,bytes)in[(Action::Undo,&before),(Action::Redo,&after)]{
   // Source global Undo is deliberately not a Marker-context shortcut. Close
   // the reopened inspector with a real key before invoking global history.
   if matches!(s.panel,Some(Panel::Marker(_))){press(&mut s,KeyCode::Escape);assert!(s.panel.is_none());retained(&mut s,&sound,old,"close before global history");}
   bind(&mut s,action);press(&mut s,KeyCode::F12);assert_eq!(&s.workspace.file().session.serialize().unwrap(),bytes);assert_eq!(peer(&s),expected_peer);retained(&mut s,&sound,old,"UndoRedo");}
 }}
}
#[test]fn physical_noop_cancel_and_stale_marker_keep_documents_history_and_current_transport(){
 for mode in 0..4{
  let(_root,_path,mut s)=open(mode);let before=s.workspace.file().session.serialize().unwrap();let revision=s.workspace.file().session.revision();let history=(s.workspace.file().session.undo_label().map(str::to_owned),s.workspace.file().session.redo_label().map(str::to_owned));let old=focus(&s);let sound=audio(&s);
  press_row(&mut s,6);press(&mut s,KeyCode::Enter);assert!(s.panel.is_none());assert_eq!(s.workspace.file().session.revision(),revision);retained(&mut s,&sound,old,"no-op");
  s.marker=Some(Draft::at_cursor(&s.workspace.file().session,0,96).unwrap());s.panel=Some(Panel::Marker(0));press_row(&mut s,5);press(&mut s,KeyCode::ArrowRight);press_row(&mut s,8);press(&mut s,KeyCode::Enter);assert_eq!(s.workspace.file().session.serialize().unwrap(),before);assert_eq!((s.workspace.file().session.undo_label().map(str::to_owned),s.workspace.file().session.redo_label().map(str::to_owned)),history);retained(&mut s,&sound,old,"Cancel");
  // Keep an older captured intent, then change the document through the real
  // native New Seed control, whose receipt rebinds a paused transport as well.
  // Direct Workspace::Place bypasses that controller and invalidates its pause;
  // it is not a valid setup for asserting transport preservation on refusal.
  let mut stale=Draft::at_cursor(&s.workspace.file().session,0,96).unwrap();
  stale.definition.ignore_preceding_distribution=!stale.definition.ignore_preceding_distribution;
  s.marker=Some(Draft::at_cursor(&s.workspace.file().session,0,96).unwrap());s.panel=Some(Panel::Marker(0));
  let seed_revision=s.workspace.file().session.revision();press_row(&mut s,14);press(&mut s,KeyCode::Enter);
  assert_ne!(s.workspace.file().session.revision(),seed_revision,"real New Seed must publish history even if random seed repeats");
  retained(&mut s,&sound,old,"native seed before stale submission");
  let changed=s.workspace.file().session.serialize().unwrap();let revision=s.workspace.file().session.revision();let changed_history=(s.workspace.file().session.undo_label().map(str::to_owned),s.workspace.file().session.redo_label().map(str::to_owned));
  // Submit the previously captured request. No result or presentation receipt is
  // injected: actual Apply must perform its usual revision check and refuse.
  s.marker=Some(stale);s.panel=Some(Panel::Marker(0));press_row(&mut s,6);press(&mut s,KeyCode::Enter);
  assert!(matches!(s.panel,Some(Panel::Marker(6))));assert!(s.marker.is_some());assert_eq!(s.workspace.file().session.serialize().unwrap(),changed);assert_eq!(s.workspace.file().session.revision(),revision);assert_eq!((s.workspace.file().session.undo_label().map(str::to_owned),s.workspace.file().session.redo_label().map(str::to_owned)),changed_history);retained(&mut s,&sound,old,"stale refusal");
 }
}
