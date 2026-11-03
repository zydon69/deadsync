use super::*;
use super::super::{Panel,RawKeyboardEvent};
use deadlib_platform::input::KeyCode;
use deadsync_editor::{key_binding_preferences::{Action,Preferences},storage::FileSession,workspace::Command};
fn press(state:&mut State,code:KeyCode){for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:Instant::now(),host_nanos:0});}}
fn fixture(language:&str)->(tempfile::TempDir,State,u64){
 crate::i18n::init(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();let old=root.path().join("old");std::fs::create_dir(&old).unwrap();std::fs::create_dir(root.path().join("new")).unwrap();
 std::fs::write(old.join("music.wav"),b"existing asset descriptor").unwrap();std::fs::write(old.join("override.wav"),b"existing override descriptor").unwrap();
 let source=old.join("source.ssc");std::fs::write(&source,b"#VERSION:0.83;#MUSIC:music.wav;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#OFFSET:0.125;#WARPS:1=2;#MUSIC:override.wav;#NOTES:1000;").unwrap();
 let mut s=State::open(&source,0).unwrap();let mut preferences=Preferences::default();preferences.commands.insert(Action::SaveAs.id().to_owned(),vec![vec!["F13".into()]]);preferences.validate().unwrap();s.configure_key_binding_preferences(Arc::new(preferences));assert_eq!(s.command_binding_label(Action::SaveAs),"F13");
 let owner=s.take_audio_commands().iter().find_map(|c|if let GameplayAudioCommand::EditorBeginAudio{owner,..}=c{Some(*owner)}else{None}).unwrap();let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=owner;snapshot.editor.output.available=true;snapshot.editor.output.leased=true;snapshot.editor.output.period_control=deadsync_gameplay::EditorBufferControl::Adjustable;snapshot.editor.output.requested_period=Some(512);snapshot.editor.output.negotiated_period=Some(512);s.update_audio(snapshot);s.take_audio_commands();s.toggle_audio();assert!(s.is_playing());s.take_audio_commands();s.bind_decoder_generation(Some(71));s.bind_decoder_asset_epoch(Some(owner),Some(83));advance(&mut s,owner,1_250_000_000);s.take_audio_commands();(root,s,owner)
}
fn advance(s:&mut State,owner:u64,music_nanos:i64){let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=owner;snapshot.editor.output=s.audio_output_snapshot();snapshot.stream_clock.valid_at=Instant::now();snapshot.stream_clock.has_music_mapping=true;snapshot.stream_clock.music_nanos=music_nanos;snapshot.stream_clock.music_seconds_per_second=1.;snapshot.stream_clock.stream_seconds=12.;snapshot.editor.logical_music=Some(deadsync_gameplay::EditorLogicalMusicClock{generation:71,asset_epoch:83,music_nanos});s.update_audio(snapshot);}
fn stamp(s:&State)->(u64,Option<u64>,Option<u64>,u64,bool,bool){(s.audio_session.scope.owner,s.chart_audio_generation(),s.playback_asset_epoch(),s.chart_focus_time().unwrap().to_bits(),s.audio_output_snapshot().leased,s.paused_playback.is_some())}
fn no_reset(commands:&[GameplayAudioCommand]){assert!(commands.iter().all(|c|!matches!(c,GameplayAudioCommand::EditorStopMusic{..}|GameplayAudioCommand::EditorPauseMusic{..}|GameplayAudioCommand::EditorResumeMusic{..}|GameplayAudioCommand::EditorStartMusic{..}|GameplayAudioCommand::EditorPlayMusic{..}|GameplayAudioCommand::EditorSeekMusic{..}|GameplayAudioCommand::EditorEndAudio{..})),"Save As reset transport");}
fn open(s:&mut State){press(s,KeyCode::F13);assert!(matches!(s.panel,Some(Panel::SaveAsDestination(0))),"{}",s.status);}
fn destination(s:&mut State,path:&std::path::Path){assert!(s.text_input(&path.to_string_lossy()));assert_eq!(s.save_as.destination,path.to_string_lossy());}
fn review(s:&mut State,path:&std::path::Path){open(s);destination(s,path);press(s,KeyCode::Enter);assert!(matches!(s.panel,Some(Panel::SaveAsReview(_))),"{}",s.status);}
fn publish(s:&mut State){let Some(Panel::SaveAsReview(index))=s.panel else{panic!("{}",s.status);};for _ in 0..index{press(s,KeyCode::ArrowUp);}press(s,KeyCode::Enter);}
#[test]
fn keyboard_open_text_invalid_destination_cancel_preserve_main_and_paused_exactly_en_fr(){
 for language in ["en","fr"]{for paused in [false,true]{let(root,mut s,_owner)=fixture(language);if paused{s.toggle_audio();s.take_audio_commands();}let before=stamp(&s);let bytes=s.workspace.file().session.serialize().unwrap();let focus=s.focus_position.as_ref().map(|p|(p.row().to_bits(),p.music_time().to_bits()));
 open(&mut s);assert_eq!(stamp(&s),before);assert!(s.text_input("relative/path.ssc"));press(&mut s,KeyCode::Enter);assert!(matches!(s.panel,Some(Panel::SaveAsDestination(_))));assert_eq!(stamp(&s),before);assert_eq!(s.workspace.file().session.serialize().unwrap(),bytes);assert_eq!(s.focus_position.as_ref().map(|p|(p.row().to_bits(),p.music_time().to_bits())),focus);no_reset(&s.take_audio_commands());press(&mut s,KeyCode::Escape);assert!(s.panel.is_none());assert_eq!(stamp(&s),before);assert!(!root.path().join("relative/path.ssc").exists());no_reset(&s.take_audio_commands());
 }}
}
#[test]
fn dsp_progress_during_review_publishes_current_position_and_keeps_same_asset_phase(){
 for language in ["en","fr"]{let(root,mut s,owner)=fixture(language);let destination_path=root.path().join("old/copy.ssc");let original_path=s.workspace.file().require_existing_path().unwrap().to_owned();let original=std::fs::read(&original_path).unwrap();let old_ids=s.workspace.file().session.chart_ids().to_vec();
 review(&mut s,&destination_path);advance(&mut s,owner,1_750_000_000);let before=stamp(&s);s.take_audio_commands();publish(&mut s);assert!(destination_path.exists(),"{}",s.status);assert_eq!(s.workspace.file().require_existing_path().unwrap(),destination_path.canonicalize().unwrap());assert_eq!(stamp(&s),before);assert_eq!(s.workspace.file().session.chart_ids(),old_ids);assert!(s.focus_position.as_ref().unwrap().is_current(&s.workspace));assert_eq!(s.focus_position.as_ref().unwrap().music_time().to_bits(),before.3);assert_eq!(std::fs::read(original_path).unwrap(),original);let commands=s.take_audio_commands();no_reset(&commands);assert!(!commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSwapMusicAsset{..}|GameplayAudioCommand::EditorClearMusicAsset{..})));advance(&mut s,owner,2_000_000_000);assert!(s.chart_focus_time().unwrap()>f64::from_bits(before.3));
 }
}
#[test]
fn relocated_save_as_uses_published_receipt_for_focus_paused_stamp_history_and_recovery(){
 for language in ["en","fr"]{for paused in [false,true]{let(root,mut s,_owner)=fixture(language);let old_path=s.workspace.file().require_existing_path().unwrap().to_owned();let old_bytes=std::fs::read(&old_path).unwrap();s.command(Command::Place(1));assert!(s.active_views.as_ref().unwrap().belongs_to(&s.workspace.file().session));let pending_grid=s.workspace.grid().clone();s.workspace.execute(Command::SaveRecovery).unwrap();let old_recovery=s.workspace.file().recovery_path().unwrap();assert!(old_recovery.exists());let old_recovery_bytes=std::fs::read(&old_recovery).unwrap();
 if paused{s.toggle_audio();s.take_audio_commands();}let before=stamp(&s);let path=root.path().join("new/published.ssc");review(&mut s,&path);s.take_audio_commands();publish(&mut s);assert!(path.exists(),"{}",s.status);assert_eq!(stamp(&s),before);assert_eq!(std::fs::read(&old_path).unwrap(),old_bytes);assert_eq!(std::fs::read(&old_recovery).unwrap(),old_recovery_bytes);
 let commands=s.take_audio_commands();no_reset(&commands);assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSwapMusicAsset{path,..}if path.ends_with("override.wav"))));let absolute=s.workspace.file().resource_base().join(s.workspace.file().session.document().text(Some(0),"MUSIC").unwrap());assert_eq!(absolute.canonicalize().unwrap(),root.path().join("old/override.wav").canonicalize().unwrap());
 assert_eq!(s.workspace.grid(),&pending_grid);s.command(Command::Undo);assert_ne!(s.workspace.grid(),&pending_grid);s.command(Command::Redo);assert_eq!(s.workspace.grid(),&pending_grid);s.workspace.execute(Command::SaveRecovery).unwrap();let recovered_bytes=s.workspace.file().session.serialize().unwrap();let mut recovered=FileSession::open(&path).unwrap();recovered.recover().unwrap();assert_eq!(recovered.session.serialize().unwrap(),recovered_bytes);assert_eq!(recovered.session.grid(0).unwrap(),&pending_grid);
 if paused{assert!(s.paused_playback.is_some());assert_eq!(s.paused_position().unwrap().music_time.to_bits(),before.3);}
 }}
}
#[test]
fn conflicting_destination_and_manual_focus_refuse_publication_without_stopping_music(){
 for language in ["en","fr"]{let(root,mut s,_owner)=fixture(language);let path=root.path().join("new/copy.ssc");std::fs::write(&path,b"review baseline").unwrap();review(&mut s,&path);let before=stamp(&s);let source=s.workspace.file().require_existing_path().unwrap().to_owned();let resident=s.workspace.file().session.serialize().unwrap();std::fs::write(&path,b"external replacement").unwrap();s.take_audio_commands();publish(&mut s);assert_eq!(stamp(&s),before);assert_eq!(s.workspace.file().require_existing_path().unwrap(),source);assert_eq!(s.workspace.file().session.serialize().unwrap(),resident);assert_eq!(std::fs::read(&path).unwrap(),b"external replacement");no_reset(&s.take_audio_commands());press(&mut s,KeyCode::Escape);
 let other=root.path().join("new/other.ssc");review(&mut s,&other);s.navigate_exact_position(201.25,true);let changed=stamp(&s);s.take_audio_commands();publish(&mut s);assert!(!other.exists());assert_eq!(stamp(&s),changed);no_reset(&s.take_audio_commands());
 }
}

#[test]
fn stopped_warp_anchor_and_final_closed_view_relocate_without_replacing_their_position(){
 for language in ["en","fr"]{for mode in 0..3 {let chartless=mode!=0;let(root,mut s,_owner)=fixture(language);
 if chartless{s.chart_views_apply(super::super::chart_views::ViewAction::Close);assert!(!s.workspace.has_chart());if mode==2{s.toggle_audio();assert!(s.paused_playback.is_some());}}else{s.stop_audio();s.navigate_exact_position(80.25,true);assert_eq!(s.focus_position.as_ref().unwrap().row().to_bits(),80.25f64.to_bits());}
 let before=stamp(&s);let exact=s.focus_position.as_ref().map(|p|(p.row().to_bits(),p.music_time().to_bits()));s.take_audio_commands();open(&mut s);assert_eq!(stamp(&s),before);press(&mut s,KeyCode::Escape);assert_eq!(s.focus_position.as_ref().map(|p|(p.row().to_bits(),p.music_time().to_bits())),exact);
 let path=root.path().join("new/anchor.ssc");review(&mut s,&path);s.take_audio_commands();publish(&mut s);assert!(path.exists(),"{}",s.status);assert_eq!(stamp(&s),before);assert_eq!(s.workspace.has_chart(),!chartless);assert_eq!(s.focus_position.as_ref().map(|p|(p.row().to_bits(),p.music_time().to_bits())),exact);no_reset(&s.take_audio_commands());if chartless{assert!(s.unfocused_music.as_ref().unwrap().ends_with("override.wav"));}
 }}
}

fn acknowledged_phase(s:&mut State,owner:u64,generation:u64,epoch:u64,music_nanos:i64){
 let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=owner;snapshot.editor.output=s.audio_output_snapshot();snapshot.stream_clock.valid_at=Instant::now();snapshot.stream_clock.has_music_mapping=true;snapshot.stream_clock.music_nanos=music_nanos;snapshot.stream_clock.music_seconds_per_second=1.;snapshot.stream_clock.stream_seconds=13.;snapshot.editor.logical_music=Some(deadsync_gameplay::EditorLogicalMusicClock{generation,asset_epoch:epoch,music_nanos});s.update_audio(snapshot);
}
#[test]
fn initial_decoder_acknowledgement_during_review_keeps_the_same_play_intent_and_can_publish(){
 for language in ["en","fr"]{let(root,mut s,owner)=fixture(language);s.stop_audio();s.toggle_audio();assert!(s.chart_transport_is_playing());assert_eq!(s.chart_audio_generation(),None);assert_eq!(s.playback_asset_epoch(),None);s.take_audio_commands();
 let path=root.path().join("old/initial-ack.ssc");review(&mut s,&path);let intent=s.save_as_transport_stamp();
 s.bind_decoder_generation(Some(91));s.bind_decoder_asset_epoch(Some(owner),Some(93));acknowledged_phase(&mut s,owner,91,93,1_500_000_000);
 assert_eq!(s.save_as_transport_stamp(),intent,"ordinary host bind/update is not a new Play");let current=stamp(&s);s.take_audio_commands();publish(&mut s);assert!(path.exists(),"{}",s.status);assert_eq!(stamp(&s),current);no_reset(&s.take_audio_commands());
 }
}
#[test]
fn watched_same_source_epoch_acknowledgement_during_review_does_not_invalidate_intent(){
 for language in ["en","fr"]{let(root,mut s,owner)=fixture(language);let path=root.path().join("old/reloaded.ssc");review(&mut s,&path);let intent=s.save_as_transport_stamp();
 // A real file change is paired with the exact native host publication seam.
 // OS notify/PCM loading is covered by the audio service tests, not fabricated
 // here: this test proves acceptance of that host's same-owner new asset epoch.
 std::fs::write(root.path().join("old/override.wav"),b"replacement source bytes").unwrap();s.bind_decoder_asset_epoch(Some(owner),Some(84));acknowledged_phase(&mut s,owner,71,84,1_750_000_000);
 assert_eq!(s.save_as_transport_stamp(),intent);assert_eq!(s.playback_asset_epoch(),Some(84));let current=stamp(&s);s.take_audio_commands();publish(&mut s);assert!(path.exists(),"{}",s.status);assert_eq!(stamp(&s),current);no_reset(&s.take_audio_commands());
 }
}
#[test]
fn same_cursor_restart_resume_pause_seek_and_rate_intents_refuse_review_without_publication(){
 for language in ["en","fr"]{for action in 0..4 {let(root,mut s,owner)=fixture(language);let time=s.chart_focus_time().unwrap();s.navigate_exact_time(time,true).unwrap();if action==1{s.toggle_audio();assert!(s.paused_playback.is_some());}s.take_audio_commands();
 let path=root.path().join("old/revoked.ssc");review(&mut s,&path);let intent=s.save_as_transport_stamp();let focus=s.focus_position.as_ref().map(|position|(position.row().to_bits(),position.music_time().to_bits()));let source=s.workspace.file().require_existing_path().unwrap().to_owned();let resident=s.workspace.file().session.serialize().unwrap();
 match action {
  0=>{s.stop_audio();s.toggle_audio();s.bind_decoder_generation(Some(72));s.bind_decoder_asset_epoch(Some(owner),Some(85));},
  1=>{s.toggle_audio();s.toggle_audio();assert!(s.paused_playback.is_some());},
  2=>{s.navigate_exact_time(time,true).unwrap();},
  _=>{let before=s.audio_preferences.effective().rate;s.step_music_rate(true);assert!(s.audio_preferences.effective().rate>before);},
 }
 let changed=s.save_as_transport_stamp();assert_eq!(changed.0,intent.0);assert_eq!(changed.2,intent.2);assert_eq!(changed.3,intent.3);assert_ne!(changed.1,intent.1,"{action} must reanchor the existing source clock");
 assert_eq!(s.focus_position.as_ref().map(|position|(position.row().to_bits(),position.music_time().to_bits())),focus);
 let current=stamp(&s);s.take_audio_commands();publish(&mut s);assert!(!path.exists(),"stale Play/seek/rate intent was published");assert_eq!(stamp(&s),current);assert_eq!(s.workspace.file().require_existing_path().unwrap(),source);assert_eq!(s.workspace.file().session.serialize().unwrap(),resident);no_reset(&s.take_audio_commands());
 }}
}
