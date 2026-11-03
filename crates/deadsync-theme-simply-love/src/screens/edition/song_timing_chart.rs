//! Common host ordering for the song timing picker and deletion fallback.
use super::*;
use deadsync_editor::{chart_order::Orderer,history::file_intent::IntentGuard};
pub(super) enum Purpose{Deletion,Cycle(bool)}
pub(super) struct Request{guard:IntentGuard,culture:u64,purpose:Purpose}
impl State {
 pub(super) fn queue_song_timing_order(&mut self,purpose:Purpose){self.song_timing_request=Some(Request{guard:IntentGuard::capture(&self.workspace.file().session),culture:i18n::revision(),purpose});}
 pub(super) fn process_song_timing_order(&mut self,orderer:&mut impl Orderer)->bool{
  let Some(request)=self.song_timing_request.take()else{return false;};
  let active=match &request.purpose{Purpose::Deletion=>self.deletion.is_some()&&matches!(self.panel,Some(Panel::DeleteChart(_))),Purpose::Cycle(_)=>self.song_properties.is_some()&&matches!(self.panel,Some(Panel::SongProperties(_)))};if !active{return true;}
  let current=|state:&State|request.culture==i18n::revision()&&request.guard.is_current(&state.workspace.file().session);
  if !current(self){self.status=i18n::tr("Edition","SongTimingStale");return true;}
  let result=self.chart_list_cache.refresh(&mut self.workspace,request.culture,orderer).map(|rows|rows.iter().map(|row|row.source_index).collect::<Vec<_>>());
  if !current(self){self.status=i18n::tr("Edition","SongTimingStale");return true;}
  let result=result.and_then(|order|{
   match request.purpose{
    Purpose::Deletion=>{
     let preferences=self.application_opening_preferences()?;
     let draft=self.deletion.as_mut().ok_or_else(||deadsync_editor::Error::InvalidEdit("Deletion is no longer open".into()))?;
     draft.prepare_song_timing(&self.workspace.file().session,&preferences,&order)
    },
    Purpose::Cycle(forward)=>{
     let draft=self.song_properties.as_mut().ok_or_else(||deadsync_editor::Error::InvalidEdit("Song properties are no longer open".into()))?;
     let old=draft.timing_chart(&self.workspace.file().session)?.and_then(|source|order.iter().position(|index|*index==source)).map(|i|i+1).unwrap_or(0);
     let next=(old+if forward{1}else{order.len()})%(order.len()+1);
     draft.set_timing_chart(&self.workspace.file().session,next.checked_sub(1).map(|index|order[index]))?;Ok(())
    }
   }
  });
  self.status=match result{Ok(())=>i18n::tr("Edition","SongTimingHelp"),Err(error)=>i18n::tr_fmt("Edition","Error",&[("message",&crate::i18n::editor_error(&error))])};true
 }
 pub(super) fn song_timing_label(&self,key:&str)->Arc<str>{
  let result=self.song_properties.as_ref().map(|draft|draft.timing_chart(&self.workspace.file().session)).unwrap_or_else(||self.workspace.file().session.song_timing_chart());
  let value=match result{
   Ok(None)=>i18n::tr("Edition","SongTimingNone").to_string(),
   Ok(Some(index))=>{let document=self.workspace.file().session.document();let chart=&document.charts()[index];let description=deadsync_editor::chart_text::Text::read(document,index).map(|text|text.get(deadsync_editor::chart_text::Field::Description).to_owned()).unwrap_or_default();let difficulty=deadsync_editor::chart_identity::Identity::read(document,index).map(|value|value.raw_difficulty().to_owned()).unwrap_or_default();format!("{} / {} / {}",chart.steps_type,difficulty,description)},
   Err(_)=>i18n::tr("Edition","ProfileStale").to_string(),
  };i18n::tr_fmt("Edition",key,&[("value",&value)])
 }
}
impl State {
 pub(super) fn apply_song_timing_history(&mut self,redo:bool){
  let result=(||->Result<(),deadsync_editor::Error>{
   let before=if self.workspace.has_chart(){Some(self.capture_song_property_presentation()?)}else{None};
   let application=self.workspace.apply_song_timing_history(redo)?;
   if let(Some(before),Some(receipt))=(before,application.presentation_receipt()){self.rebind_song_property_presentation(before,receipt)?;}
   Ok(())
  })();
  self.status=match result{Ok(())=>i18n::tr("Edition","Ready"),Err(error)=>i18n::tr_fmt("Edition","Error",&[("message",&crate::i18n::editor_error(&error))])};self.refresh();
 }
}
#[cfg(test)]
mod tests {
 use super::*;
 struct Reverse;
 impl deadsync_editor::chart_order::Orderer for Reverse{fn order(&mut self,snapshot:&deadsync_editor::chart_order::Snapshot)->Result<Vec<usize>,deadsync_editor::Error>{Ok(snapshot.source_indices().iter().copied().rev().collect())}}
 fn state(language:&str)->(tempfile::TempDir,State){crate::i18n::init_for_tests();crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));let root=tempfile::tempdir().unwrap();std::fs::write(root.path().join("prefs.json"),br#"{"Version":1}"#).unwrap();std::fs::write(root.path().join("song.ssc"),"#VERSION:0.83;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#WARPS:1=2;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:First;#DIFFICULTY:Hard;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Second;#DIFFICULTY:Hard;#NOTES:0100;").unwrap();let mut state=State::open_with_preferences(&root.path().join("song.ssc"),0,&root.path().join("prefs.json")).unwrap();state.workspace.execute(Command::Seek(60)).unwrap();state.focus_position=Some(deadsync_editor::chart_focus::Position::at_fractional_row(&state.workspace.file().session,0,60.5).unwrap());state.take_audio_commands();(root,state)}
 #[test] fn keyboard_song_picker_uses_common_order_pending_barrier_and_receipt_en_fr(){for language in ["en","fr"]{
  let(_root,mut state)=state(language);let focus=state.focus_position.as_ref().unwrap().row();state.open_song_properties();assert!(state.song_properties.is_some());
  // Reverse order is [second,first], so left from first selects second.
  state.song_properties_key(6,KeyCode::ArrowLeft);state.song_properties_key(1,KeyCode::Enter);assert!(state.song_properties.is_some());assert_eq!(state.status,i18n::tr("Edition","SongTimingLoading"));
  assert!(state.process_chart_order(&mut Reverse));state.song_properties_key(1,KeyCode::Enter);assert_eq!(state.workspace.file().session.song_timing_chart().unwrap(),Some(1));assert_eq!(state.workspace.chart(),0);assert_eq!(state.focus_position.as_ref().unwrap().row().to_bits(),focus.to_bits());
  state.command(Command::Undo);assert_eq!(state.workspace.file().session.song_timing_chart().unwrap(),Some(0));assert_eq!(state.focus_position.as_ref().unwrap().row().to_bits(),focus.to_bits());state.command(Command::Redo);assert_eq!(state.workspace.file().session.song_timing_chart().unwrap(),Some(1));
  assert!(!state.take_audio_commands().iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorStopMusic{..}|deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic{..})));
 }}
 #[test] fn picker_cancel_stale_and_no_song_are_non_mutating_en_fr(){for language in ["en","fr"]{
  let(root,mut state)=state(language);let before=state.workspace.file().session.serialize().unwrap();state.open_song_properties();state.song_properties_key(6,KeyCode::ArrowRight);state.song_properties_key(2,KeyCode::Enter);assert!(state.process_chart_order(&mut Reverse));assert_eq!(state.workspace.file().session.serialize().unwrap(),before);assert!(state.song_properties.is_none());
  state.open_song_properties();state.song_properties_key(6,KeyCode::ArrowRight);state.workspace.execute(Command::Place(1)).unwrap();let changed=state.workspace.file().session.serialize().unwrap();state.process_chart_order(&mut Reverse);assert_eq!(state.status,i18n::tr("Edition","SongTimingStale"));assert_eq!(state.workspace.file().session.serialize().unwrap(),changed);
  let mut no_song=State::without_song(root.path()).unwrap();no_song.open_song_properties();assert!(no_song.song_properties.is_none());assert_eq!(no_song.status,i18n::tr("Edition","FileNoSongHelp"));
 }}
 #[test] fn output_default_follows_song_but_arrow_explicitly_selects_override(){let(_root,mut state)=state("en");state.open_output_settings();state.output_key(0,KeyCode::Enter);assert!(matches!(state.output_candidate_policy().unwrap(),deadsync_editor::output_policy::OutputPolicy::AdvancedSongTiming{..}));state.output_key(2,KeyCode::ArrowRight);assert!(matches!(state.output_candidate_policy().unwrap(),deadsync_editor::output_policy::OutputPolicy::Advanced{..}));state.output_key(2,KeyCode::ArrowLeft);assert!(matches!(state.output_candidate_policy().unwrap(),deadsync_editor::output_policy::OutputPolicy::AdvancedSongTiming{..}));}
}
