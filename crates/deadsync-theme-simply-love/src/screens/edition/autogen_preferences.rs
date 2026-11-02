//! Application autogen choices survive song replacement. Temporary expression
//! overrides remain separate from the source chart's persisted expression GUID.
use super::*;
use deadsync_editor::autogen_preferences::{self as settings,Preferences};
impl State {
    pub fn autogen_preferences(&self)->Preferences {self.autogen_preferences.clone()}
    pub fn configure_autogen_preferences(&mut self,preferences:Preferences){
        self.generation.target=preferences.target.layout().ok().and_then(|layout|TARGET_LAYOUTS.iter().position(|candidate|*candidate==layout)).unwrap_or(0);
        let presets=[settings::BALANCED,settings::STAMINA_SINGLE,settings::STAMINA_DOUBLE];
        if let Some(preset)=presets.iter().position(|id|*id==preferences.performed.to_string()) {
            self.generation.preset=preset;self.generation.profiles.performed=None;self.generation.performed_name=None;
        } else {
            self.generation.preset=0;
            self.generation.profiles.performed=Some(preferences.performed.clone());
            // Until the catalogue resolves its source label, show the actual
            // identity rather than claiming this is the Balanced preset.
            self.generation.performed_name=Some(Arc::from(preferences.performed.to_string()));
        }
        self.autogen_preferences=preferences;
    }
    pub(super) fn publish_autogen_profile_preferences(&mut self){
        self.autogen_preferences.performed=self.generation.profiles.performed.clone().unwrap_or_else(||[
            settings::BALANCED,settings::STAMINA_SINGLE,settings::STAMINA_DOUBLE,
        ][self.generation.preset].parse().expect("constant GUID"));
        let value=self.autogen_preferences.clone();if let Some(file)=self.application_preferences_owner_mut(){file.set_autogen(value);}
    }
}
#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn imported_autogen_target_and_profile_survive_application_snapshot_new_and_no_song_en_fr(){
  crate::i18n::init_for_tests();
  for language in ["en","fr"]{
   crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
   let root=tempfile::tempdir().unwrap();
   let prefs=root.path().join("prefs.json");
   let custom="11111111-2222-3333-4444-555555555555";
   std::fs::write(&prefs,serde_json::to_vec(&serde_json::json!({"Version":1,"LastSelectedAutogenChartType":"pump_double","LastSelectedAutogenPerformedChartConfig":custom,"Vendor":{"x":7}})).unwrap()).unwrap();
   let mut state=State::without_song(root.path()).unwrap();state.configure_audio_preferences(&prefs).unwrap();
   let before=state.autogen_preferences();assert_eq!(before.target.layout().unwrap(),"pump-double");assert_eq!(before.performed.to_string(),custom);
   let mut candidate=State::without_song(root.path()).unwrap();
   pack_song_list::application_preferences::Snapshot::capture(&state).install(&mut candidate).unwrap();
   assert_eq!(candidate.autogen_preferences(),before);
   assert!(!candidate.has_song());
   candidate.command_file_new();assert!(candidate.has_song());assert_eq!(candidate.autogen_preferences(),before);
   candidate.audio_preferences_file.as_mut().unwrap().save().unwrap();
   assert_eq!(deadsync_editor::audio_preferences::File::open(&prefs).unwrap().autogen(),before);
   let value:serde_json::Value=serde_json::from_slice(&std::fs::read(prefs).unwrap()).unwrap();assert_eq!(value["Vendor"],serde_json::json!({"x":7}));
  }
 }
 #[test]
 fn unknown_source_target_is_not_used_as_a_fake_supported_layout(){
  crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();
  let value=serde_json::json!({"Version":1,"LastSelectedAutogenChartType":"dance_routine"}).as_object().unwrap().clone();
  state.configure_autogen_preferences(Preferences::read(&value).unwrap());assert!(state.generation_options().is_err());
  state.adjust_generation_setting(0,true);assert!(state.generation_options().is_ok());
 }
}

#[cfg(test)]
mod deletion_undo_tests {
 use super::*;
 #[test]
 fn deleting_selected_profile_repairs_persistent_choice_and_undo_redo_restore_it_en_fr(){
  use deadsync_editor::generation::{profiles::{Profile,Kind},profile_catalog::Task};
  use std::sync::atomic::AtomicBool;
  crate::i18n::init_for_tests();
  for language in ["en","fr"] {
   crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
   let root=tempfile::tempdir().unwrap();let library=root.path().join("configs");std::fs::create_dir(&library).unwrap();
   let profile=Profile::parse(Kind::Performed,&serde_json::to_vec(&serde_json::json!({"Guid":"11111111-2222-3333-4444-555555555555","Name":"Custom","Config":PerformedConfig::balanced()})).unwrap()).unwrap();profile.export_new(&library).unwrap();
   let source=root.path().join("song.ssc");std::fs::write(&source,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
   let prefs=root.path().join("prefs.json");std::fs::write(&prefs,serde_json::to_vec(&serde_json::json!({"Version":1,"LastSelectedAutogenChartType":"pump_double","LastSelectedAutogenPerformedChartConfig":profile.guid()})).unwrap()).unwrap();
   let mut state=State::open(&source,0).unwrap();state.configure_audio_preferences(&prefs).unwrap();state.profile_directory=Some(library.clone());
   let catalogue=Task::capture(&state.workspace.file().session,state.workspace.chart(),Some(&library)).unwrap().run(&AtomicBool::new(false)).unwrap();
   let selected=catalogue.entries(Kind::Performed).iter().position(|entry|&entry.guid==profile.guid()).unwrap()+1;
   state.profile_selection=Some(ProfileSettings{catalog:catalogue,pattern:None,performed:Some(selected),expressed:Some(0)});
   state.open_profile_edit(Kind::Performed,Panel::ConversionProfiles(0));state.profile_edit_key(0,KeyCode::F9);assert!(matches!(state.panel,Some(Panel::ProfileDelete(_))));
   state.profile_delete_key(1,KeyCode::Enter);
   assert_eq!(state.autogen_preferences.performed.to_string(),settings::BALANCED);
   assert_eq!(state.audio_preferences_file.as_ref().unwrap().autogen().performed.to_string(),settings::BALANCED);
   assert_eq!(state.autogen_preferences.target.layout().unwrap(),"pump-double");
   state.command(Command::Undo);assert_eq!(&state.autogen_preferences.performed,profile.guid());assert!(library.join(profile.filename()).exists());
   state.command(Command::Redo);assert_eq!(state.autogen_preferences.performed.to_string(),settings::BALANCED);assert!(!library.join(profile.filename()).exists());
   state.audio_preferences_file.as_mut().unwrap().save().unwrap();assert_eq!(deadsync_editor::audio_preferences::File::open(&prefs).unwrap().autogen(),state.autogen_preferences);
  }
 }
}
