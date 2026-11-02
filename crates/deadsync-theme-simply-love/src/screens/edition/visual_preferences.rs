//! Canonical native publication for the non-file Waveform/MiniMap history action.
use super::*;
use deadsync_editor::{Error, visual_preferences::{MiniMap, Snapshot}};
impl State {
    pub(super) fn retry_preferences_durability(&mut self)->bool {
        if self.auto_recovery.is_pending(){self.status=i18n::tr("Edition","FileBusy");return false;}
        if let Err(error)=self.workspace.file().retry_recovery_durability(){self.status=i18n::tr_fmt("Edition","RecoveryDurabilityPending",&[("error",&i18n::editor_error(&error))]);return false;}
        match deadsync_editor::audio_preferences::durability::retry_all(){
            Ok(())=>true,
            Err(error)=>{self.status=i18n::tr_fmt("Edition","PreferencesSyncRetryFailed",&[("error",&i18n::editor_error(&error))]);false}
        }
    }
    pub(super) fn preferences_durability_actors(&self,actors:&mut Vec<Actor>){
        if let Some(error)=self.workspace.file().recovery_durability_warning(){actors.push(act!(text:font("miso"):settext(i18n::tr_fmt("Edition","RecoveryDurabilityPending",&[("error",&error)])):xy(screen_width()*0.5,screen_height()-48.):zoom(0.5):maxwidth((screen_width()-32.).max(1.)):diffuse(1.,0.72,0.25,1.):z(8500)));}
        if !deadsync_editor::audio_preferences::durability::has_pending_publications(){return;}
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","PreferencesSyncPending")):xy(screen_width() * 0.5,screen_height()-24.0):zoom(0.55):maxwidth((screen_width()-32.0).max(1.0)):diffuse(1.0,0.72,0.25,1.0):z(8500)));
    }
    pub(super) fn visual_minimap(&self) -> MiniMap {
        MiniMap {
            viewport: self.minimap.preferences,
            position: self.minimap.position,
            content: self.minimap.content.options,
        }
    }
    pub(super) fn apply_visual_preferences(
        &mut self,
        before: Snapshot,
        after: Snapshot,
    ) -> Result<(), Error> {
        let file = self.capture_application_preferences_file()?;
        self.workspace
            .set_visual_preferences(before, after, Some(&file))?;
        self.sync_visual_preferences_history();
        Ok(())
    }
    pub(super) fn sync_visual_preferences_history(&mut self) {
        let Some(update) = self.workspace.take_visual_preferences_update() else {
            return;
        };
        match update.preferences {
            Snapshot::Waveform(p) => {
                self.waveform_preferences = p;
                self.apply_waveform_preferences();
            }
            Snapshot::MiniMap(p) => {
                self.configure_minimap_preferences(p.viewport);
                self.configure_minimap_position_preferences(p.position);
                self.configure_minimap_content_preferences(p.content);
            }
        }
        if let Some(file) = update.file {
            if file.path().as_os_str().is_empty() {
                self.audio_preferences_path = None;
                self.audio_preferences_file = None;
                self.source_preferences_import.memory = Some(file);
            } else {
                self.audio_preferences_path = Some(file.path().to_owned());
                self.source_preferences_import.memory = None;
                self.audio_preferences_file = Some(file);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_minimap_restore_buttons_cover_whole_group_and_waveform_save_undo_preserves_song_en_fr()
    {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let song = root.path().join("song.ssc");
            std::fs::write(
                &song,
                "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
            )
            .unwrap();
            let mut state = State::open(&song, 0).unwrap();
            let bytes = state.workspace.file().session.serialize().unwrap();
            let before = state.visual_minimap();
            let mut changed = before;
            changed
                .viewport
                .set_visible_range(deadsync_editor::minimap_viewport::Axis::Time, 100.)
                .unwrap();
            changed.position.set_offset(changed.position.mount, 40);
            changed.content.width = 128;
            changed.content.max_notes = 7000;
            state
                .apply_visual_preferences(Snapshot::MiniMap(before), Snapshot::MiniMap(changed))
                .unwrap();
            for options in [false, true] {
                if options {
                    state.open_minimap_options();
                    state.minimap_options_key(13, KeyCode::Enter);
                    state.minimap_options_key(14, KeyCode::Enter);
                } else {
                    state.open_minimap_settings();
                    state.minimap_settings_key(4, KeyCode::Enter);
                    state.minimap_settings_key(5, KeyCode::Enter);
                }
                assert_eq!(state.visual_minimap(), changed.restored());
                state.command(Command::Undo);
                assert_eq!(state.visual_minimap(), changed);
                state.command(Command::Redo);
                assert_eq!(state.visual_minimap(), changed.restored());
                state.command(Command::Undo);
            }
            let waveform = state.waveform_preferences;
            state.open_waveform_settings();
            state.waveform_settings_key(5, KeyCode::Enter);
            state.waveform_settings_key(14, KeyCode::Enter);
            assert_ne!(state.waveform_preferences, waveform);
            state.command(Command::Undo);
            assert_eq!(state.waveform_preferences, waveform);
            state.command(Command::Redo);
            assert_ne!(state.waveform_preferences, waveform);
            assert!(state.audio_preferences_file.is_none());
            assert!(state.audio_preferences_path.is_none());
            assert!(state.source_preferences_import.memory.is_some());
            assert!(!state.workspace.file().session.is_dirty());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), bytes);
        }
    }
}
