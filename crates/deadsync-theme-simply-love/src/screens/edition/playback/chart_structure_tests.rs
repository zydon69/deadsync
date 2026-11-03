use super::*;
use super::super::{Panel,RawKeyboardEvent};
use deadlib_platform::input::KeyCode;
use deadsync_editor::{key_binding_preferences::{Action,Preferences},chart_order::{Orderer,Snapshot}};
fn press(state:&mut State,code:KeyCode){for pressed in [true,false]{state.raw_key(&RawKeyboardEvent{code,pressed,repeat:false,timestamp:Instant::now(),host_nanos:0});}}
struct FixtureOrder;
impl Orderer for FixtureOrder {fn order(&mut self,snapshot:&Snapshot)->Result<Vec<usize>,deadsync_editor::Error>{Ok(snapshot.source_indices().to_vec())}}
fn fixture(language:&str,override_music:bool)->(tempfile::TempDir,State,u64){
 crate::i18n::init(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();
 for name in ["main.wav","override.wav"]{std::fs::write(root.path().join(name),b"existing asset descriptor fixture").unwrap();}
 let chart_music=if override_music{"#MUSIC:override.wav;"}else{""};
 let source=format!("#VERSION:0.83;#MUSIC:main.wav;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;{chart_music}#OFFSET:0.125;#NOTES:1000;");
 let path=root.path().join("song.ssc");std::fs::write(&path,source).unwrap();let mut state=State::open(&path,0).unwrap();
 // New/Delete are native authoring extensions. Source preset import
 // deliberately neutralizes them; use the full native profile to exercise
 // these actual native keyboard paths, preserving every command-label owner.
 let mut preferences=Preferences::default();
 for (action,key) in [(Action::NewChart,"F13"),(Action::DeleteChart,"F14"),(Action::Undo,"F15"),(Action::Redo,"F16")] {
     preferences.commands.insert(action.id().to_owned(),vec![vec![key.to_owned()]]);
 }
 preferences.validate().unwrap();
 state.configure_key_binding_preferences(Arc::new(preferences));
 assert_eq!(state.command_binding_label(Action::NewChart),"F13");
 assert_eq!(state.command_binding_label(Action::DeleteChart),"F14");
 let owner=state.take_audio_commands().iter().find_map(|c|if let GameplayAudioCommand::EditorBeginAudio{owner,..}=c{Some(*owner)}else{None}).unwrap();
 let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=owner;snapshot.editor.output.available=true;snapshot.editor.output.leased=true;snapshot.editor.output.period_control=deadsync_gameplay::EditorBufferControl::Adjustable;snapshot.editor.output.requested_period=Some(512);snapshot.editor.output.negotiated_period=Some(512);state.update_audio(snapshot);state.take_audio_commands();
 state.toggle_audio();assert!(state.is_playing());state.take_audio_commands();state.bind_decoder_generation(Some(71));state.bind_decoder_asset_epoch(Some(owner),Some(83));
 advance(&mut state,owner,1_250_000_000);state.take_audio_commands();(root,state,owner)
}
fn advance(state:&mut State,owner:u64,music_nanos:i64){let mut snapshot=GameplayAudioSnapshot::default();snapshot.editor.owner=owner;snapshot.editor.output=state.audio_output_snapshot();snapshot.stream_clock.valid_at=Instant::now();snapshot.stream_clock.stream_seconds=12.;snapshot.stream_clock.has_music_mapping=true;snapshot.stream_clock.music_nanos=music_nanos;snapshot.stream_clock.music_seconds_per_second=1.;snapshot.editor.logical_music=Some(deadsync_gameplay::EditorLogicalMusicClock{generation:71,asset_epoch:83,music_nanos});state.update_audio(snapshot);}
fn stamp(s:&State)->(u64,Option<u64>,Option<u64>,u64,bool){(s.audio_session.scope.owner,s.chart_audio_generation(),s.playback_asset_epoch(),s.chart_focus_time().unwrap().to_bits(),s.audio_output_snapshot().leased)}
fn no_reset(commands:&[GameplayAudioCommand]){assert!(commands.iter().all(|c|!matches!(c,GameplayAudioCommand::EditorStopMusic{..}|GameplayAudioCommand::EditorStartMusic{..}|GameplayAudioCommand::EditorPlayMusic{..}|GameplayAudioCommand::EditorSeekMusic{..}|GameplayAudioCommand::EditorEndAudio{..})),"unexpected transport reset");}
fn open_delete(s:&mut State){press(s,KeyCode::F14);assert!(matches!(s.panel,Some(Panel::DeleteChart(_))),"{}",s.status);assert!(s.process_song_timing_order(&mut FixtureOrder));}
#[test]
fn real_keyboard_create_cancel_commit_and_structural_undo_redo_keep_same_asset_transport_en_fr(){
 for language in ["en","fr"]{let(_root,mut s,owner)=fixture(language,false);let original=s.workspace.file().session.serialize().unwrap();let original_ids=s.workspace.file().session.chart_ids().to_vec();let before=stamp(&s);
 press(&mut s,KeyCode::F13);assert!(matches!(s.panel,Some(Panel::NewChart(_))),"{}",s.status);assert_eq!(stamp(&s),before);no_reset(&s.take_audio_commands());
 press(&mut s,KeyCode::Escape);assert_eq!(s.workspace.file().session.serialize().unwrap(),original);assert_eq!(stamp(&s),before);
 press(&mut s,KeyCode::F13);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);assert!(s.panel.is_none());assert_eq!(s.workspace.file().session.document().charts().len(),2);assert_eq!(s.workspace.chart(),1);assert_eq!(stamp(&s),before);let commands=s.take_audio_commands();no_reset(&commands);assert!(!commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSwapMusicAsset{..}|GameplayAudioCommand::EditorClearMusicAsset{..})));
 let created=s.workspace.file().session.serialize().unwrap();press(&mut s,KeyCode::F15);assert_eq!(s.workspace.file().session.serialize().unwrap(),original);assert_eq!(s.workspace.file().session.chart_ids(),original_ids);assert_eq!(stamp(&s),before);no_reset(&s.take_audio_commands());
 press(&mut s,KeyCode::F16);assert_eq!(s.workspace.file().session.serialize().unwrap(),created);assert_eq!(stamp(&s),before);no_reset(&s.take_audio_commands());advance(&mut s,owner,1_500_000_000);assert!(s.chart_focus_time().unwrap()>f64::from_bits(before.3));
 }
}
#[test]
fn creation_with_different_inherited_music_swaps_typed_asset_without_resetting_main(){
 for language in ["en","fr"]{let(_root,mut s,_owner)=fixture(language,true);let before=stamp(&s);press(&mut s,KeyCode::F13);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);assert_eq!(stamp(&s),before);let commands=s.take_audio_commands();no_reset(&commands);assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSwapMusicAsset{path,..}if path.ends_with("main.wav"))));
 press(&mut s,KeyCode::F15);assert_eq!(stamp(&s),before);let commands=s.take_audio_commands();no_reset(&commands);assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSwapMusicAsset{path,..}if path.ends_with("override.wav"))));}
}
#[test]
fn delete_last_chart_cancel_commit_undo_redo_retains_override_and_chartless_clock(){
 for language in ["en","fr"]{let(_root,mut s,owner)=fixture(language,true);let original=s.workspace.file().session.serialize().unwrap();let before=stamp(&s);open_delete(&mut s);assert_eq!(stamp(&s),before);no_reset(&s.take_audio_commands());press(&mut s,KeyCode::Escape);assert_eq!(s.workspace.file().session.serialize().unwrap(),original);assert_eq!(stamp(&s),before);
 open_delete(&mut s);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);assert!(!s.workspace.has_chart());assert_eq!(s.workspace.file().session.document().charts().len(),0);assert_eq!(stamp(&s),before);assert!(s.unfocused_music.as_ref().unwrap().ends_with("override.wav"));let commands=s.take_audio_commands();no_reset(&commands);assert!(!commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorSwapMusicAsset{..}|GameplayAudioCommand::EditorClearMusicAsset{..})));
 press(&mut s,KeyCode::F15);assert_eq!(s.workspace.file().session.serialize().unwrap(),original);assert!(s.workspace.has_chart());assert_eq!(stamp(&s),before);no_reset(&s.take_audio_commands());press(&mut s,KeyCode::F16);assert!(!s.workspace.has_chart());assert_eq!(stamp(&s),before);no_reset(&s.take_audio_commands());advance(&mut s,owner,1_500_000_000);assert!(s.chart_focus_time().unwrap()>f64::from_bits(before.3));
 }
}
#[test]
fn stale_creation_and_deletion_refuse_without_transport_mutation(){
 for language in ["en","fr"]{let(_root,mut s,_owner)=fixture(language,false);
 press(&mut s,KeyCode::F13);compete(&mut s,"Competing edit");let bytes=s.workspace.file().session.serialize().unwrap();let before=stamp(&s);s.take_audio_commands();press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);assert!(matches!(s.panel,Some(Panel::NewChart(_))),"{}",s.status);assert_eq!(s.workspace.file().session.serialize().unwrap(),bytes);assert_eq!(stamp(&s),before);no_reset(&s.take_audio_commands());press(&mut s,KeyCode::Escape);
 open_delete(&mut s);compete(&mut s,"Another edit");let bytes=s.workspace.file().session.serialize().unwrap();let before=stamp(&s);s.take_audio_commands();press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);assert!(matches!(s.panel,Some(Panel::DeleteChart(_))),"{}",s.status);assert_eq!(s.workspace.file().session.serialize().unwrap(),bytes);assert_eq!(stamp(&s),before);no_reset(&s.take_audio_commands());
 }
}

fn compete(state:&mut State,title:&str){let mut draft=state.workspace.song_properties().unwrap();draft.set_text(deadsync_editor::song_text::Field::Title,title).unwrap();state.workspace.apply_song_properties(&draft).unwrap();}
#[test]
fn paused_structural_commit_rebinds_resume_stamp_without_seeking_or_losing_retained_music(){
 for language in ["en","fr"]{let(_root,mut s,owner)=fixture(language,false);let time=s.chart_focus_time().unwrap();s.toggle_audio();assert!(s.paused_playback.is_some());s.take_audio_commands();
 press(&mut s,KeyCode::F13);press(&mut s,KeyCode::ArrowDown);press(&mut s,KeyCode::Enter);assert!(s.paused_playback.is_some());assert_eq!(s.chart_focus_time().unwrap().to_bits(),time.to_bits());no_reset(&s.take_audio_commands());
 press(&mut s,KeyCode::F15);assert!(s.paused_playback.is_some());assert_eq!(s.chart_focus_time().unwrap().to_bits(),time.to_bits());no_reset(&s.take_audio_commands());
 s.toggle_audio();assert!(s.is_playing());let commands=s.take_audio_commands();assert!(commands.iter().any(|c|matches!(c,GameplayAudioCommand::EditorResumeMusic{owner:actual,seconds,..}if *actual==owner&&(*seconds-time-s.audio_preferences.effective().offset).abs()<1e-12)));no_reset(&commands);
 }
}
