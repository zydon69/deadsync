//! Six source onboarding steps. Settings use their existing General owner.
use super::*;
impl State {
    pub(super) fn ftue_preferences(&self) -> deadsync_editor::ftue::Preferences {
        self.application_preferences_owner()
            .map(|f| f.ftue().clone())
            .unwrap_or_else(|| self.ftue.clone())
    }
    pub(super) fn ftue_complete(&self) -> bool {
        self.ftue_preferences().completed()
    }
    fn save_ftue(&mut self, value: deadsync_editor::ftue::Preferences) -> bool {
        let result=(||->Result<_,deadsync_editor::Error>{
            let current=self.capture_application_preferences_file()?;let before=self.ftue_preferences();
            let mut owner=Some(current.clone());
            let merged=deadsync_editor::audio_preferences::publication::publish(&mut owner,Some(&current),value.clone(),|current|{
                let mut file=current.clone();let merged=file.ftue().merge_changes(&before,&value)?;
                file.set_ftue(merged.clone());Ok((merged,file))
            })?;Ok((owner,merged))
        })();
        match result{Ok((owner,merged))=>{if let Some(file)=owner{self.publish_application_preferences_owner(file);}self.ftue=merged;true},Err(error)=>{self.status=i18n::tr_fmt("Edition","Error",&[("message",&crate::i18n::editor_error(&error))]);false}}
    }

    pub(super) fn begin_ftue(&mut self) {
        let mut value = self.ftue_preferences();
        value.restart();
        if self.save_ftue(value) {
            self.show_ftue_page();
        }
    }
    fn show_ftue_page(&mut self) {
        let value = self.ftue_preferences();
        self.general_field = match value.page() {
            Some(1) => 4,
            Some(2) => 1,
            Some(3) => 2,
            Some(4) => 6,
            _ => 0,
        };
        self.lane_entry_release_input();
        self.panel = Some(Panel::Ftue(0));
    }
    pub fn update_host_ftue(&mut self) {
        if self.ftue_complete()
            || self.panel.is_some()
            || self.help
            || self.region_selection.is_some()
            || self.preview.is_some()
            || self.text_buffer.is_some()
            || self.is_generating()
            || self.file_modal()
            || self.audio_output_busy()
            || self.host_close_pending()
        {
            return;
        }
        let mut value = self.ftue_preferences();
        value.finish_invalid();
        if value.completed() {
            self.save_ftue(value);
        } else {
            self.show_ftue_page();
        }
    }
    pub(super) fn ftue_key(&mut self, index: usize, code: KeyCode) {
        let Some(page) = self.ftue_preferences().page() else {
            return;
        };
        if matches!(page, 2 | 3) && code == KeyCode::KeyH {
            self.editor_external_open = Some(editor_help::ExternalOpen::Url(
                "https://github.com/PerryAsleep/GrooveAuthor/blob/48e18f71655d0f0d86144c63239331833501fda2/StepManiaEditor/docs/SongSync.md",
            ));
            return;
        }
        let count = if page == 0 {
            1
        } else if page == 5 {
            4
        } else {
            3
        };
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::Ftue(
                (index + if back { count - 1 } else { 1 }) % count,
            ));
            return;
        }
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if (1..=4).contains(&page) && index == 0 {
            self.general_options_key(1, code);
            self.panel = Some(Panel::Ftue(0));
            return;
        }
        if !enter {
            return;
        }
        if page == 5 && index < 2 {
            self.editor_external_open = Some(editor_help::ExternalOpen::Url(if index == 0 {
                "https://github.com/PerryAsleep/GrooveAuthor/blob/48e18f71655d0f0d86144c63239331833501fda2/StepManiaEditor/docs/TableOfContents.md"
            } else {
                "https://github.com/PerryAsleep/GrooveAuthor"
            }));
            return;
        }
        let mut value = self.ftue_preferences();
        let back = page > 0 && index == if page == 5 { 2 } else { 1 };
        if back {
            value.back();
        } else {
            value.next();
        }
        if self.save_ftue(value) {
            if self.ftue_complete() {
                self.panel = None;
            } else {
                self.show_ftue_page();
            }
        }
    }
    pub(super) fn ftue_actors(&self, actors: &mut Vec<Actor>, index: usize) {
        let Some(page) = self.ftue_preferences().page() else {
            return;
        };
        let mut rows: Vec<String> = Vec::new();
        if (1..=4).contains(&page) {
            rows.push(self.general_options_label(1, "GeneralValue").to_string());
        }
        if page == 5 {
            rows.push(i18n::tr("Edition", "EditorHelpDocs").to_string());
            rows.push(i18n::tr("Edition", "EditorHelpGitHub").to_string());
        }
        if page > 0 {
            rows.push(i18n::tr("Edition", "FtueBack").to_string());
        }
        rows.push(i18n::tr("Edition", if page == 5 { "FtueDone" } else { "FtueNext" }).to_string());
        // Original UIFTUE: width622, small125/medium220/large395.
        // Native keyboard rows are vertical, so reserve their actual space
        // instead of forcing every page into the old generic 800x480 modal.
        const FONT: f32 = 15. / 24.; // Miso's retained LineSpacing is24px.
        let w = (screen_width() - 30.).min(622.).max(1.);
        let controls = rows.len() as f32 * 26.;
        let preferred: f32 = match page {
            4 => 395.,
            5 => 220.,
            _ => 125.,
        };
        let h = preferred
            .max(128. + controls)
            .min((screen_height() - 30.).max(1.));
        let x = screen_width() * 0.5;
        let y = (screen_height() - h) * 0.5;
        actors.push(act!(quad:xy(x,y+h*0.5):setsize(w,h):diffuse(0.04,0.06,0.1,0.98):z(8070)));
        actors.push(act!(text:font("miso"):settext(format!("{} {}/6",i18n::tr("Edition","FtueWelcome"),page+1)):xy(x,y+20.):zoom(FONT):maxwidth(w-30.):z(8071)));
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition",["FtueIntro","FtueChart","FtueNewSync","FtueOpenSync","FtueGraphs","FtueFinal"][page])):align(0.,0.):xy(x-w*0.5+15.,y+44.):zoom(FONT):wrapwidthpixels((w-30.)/FONT):maxheight((h-controls-62.).max(1.)):z(8071)));
        for (i, row) in rows.iter().enumerate() {
            let start = actors.len();
            actors.push(act!(text:font("miso"):settext(format!("{} {}",if i==index{">"}else{" "},row)):align(0.,0.):xy(x-w*0.5+15.,y+h-controls-12.+i as f32*26.):zoom(FONT):maxwidth(w-30.):z(8071)));
            self.ui_row(actors, start, i == index, 26.);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&deadlib_platform::input::RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    #[test]
    fn six_pages_use_existing_options_and_preserve_document_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("preferences.json");
            let mut state = State::without_song(directory.path()).unwrap();
            state.configure_audio_preferences(&path).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            state.panel = None;
            state.update_host_ftue();
            assert!(matches!(state.panel, Some(Panel::Ftue(0))));
            for page in 0..6 {
                assert_eq!(state.ftue_preferences().page(), Some(page));
                let mut actors = Vec::new();
                state.ftue_actors(&mut actors, 0);
                assert!(!actors.is_empty());
                if (1..=4).contains(&page) {
                    let original = state.general_options.clone();
                    press(&mut state, KeyCode::ArrowRight);
                    assert_ne!(state.general_options, original);
                    assert!(matches!(state.panel, Some(Panel::Ftue(0))));
                }
                let next = if page == 0 {
                    0
                } else if page == 5 {
                    3
                } else {
                    2
                };
                for _ in 0..next {
                    press(&mut state, KeyCode::ArrowDown);
                }
                press(&mut state, KeyCode::Enter);
            }
            assert!(state.ftue_complete());
            assert!(
                state.workspace.file().session.undo_label().is_none(),
                "source wizard controls are not undoable"
            );
            assert!(state.panel.is_none());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert!(
                deadsync_editor::audio_preferences::File::open(&path)
                    .unwrap()
                    .ftue()
                    .completed()
            );
            state.begin_ftue();
            assert_eq!(state.ftue_preferences().page(), Some(0));
            assert!(!state.ftue_complete());
        }
    }
    #[test]
    fn changing_onboarding_sync_defaults_preserves_loaded_song_clock_and_history() {
        i18n::init_for_tests();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("song.ssc");
        std::fs::write(&path, r#"#VERSION:0.83;#OFFSET:0.125;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{"SyncOffset":0.009};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;
0100;
"#).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.command(Command::Move(1));
        let before = state.workspace.file().session.serialize().unwrap();
        let row = state.workspace.row();
        let time = state.chart_focus_time().unwrap();
        let revision = state.workspace.file().session.revision();
        state.panel = None;
        state.update_host_ftue();
        for page in 0..6 {
            if matches!(page, 2 | 3) {
                press(&mut state, KeyCode::ArrowRight);
            }
            let next = if page == 0 {
                0
            } else if page == 5 {
                3
            } else {
                2
            };
            for _ in 0..next {
                press(&mut state, KeyCode::ArrowDown);
            }
            press(&mut state, KeyCode::Enter);
        }
        assert!(state.ftue_complete());
        assert_eq!(state.workspace.row(), row);
        assert_eq!(state.chart_focus_time().unwrap().to_bits(), time.to_bits());
        assert_eq!(state.workspace.file().session.revision(), revision);
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert!(state.workspace.file().session.undo_label().is_none());
    }
    #[test]
    fn onboarding_waits_for_modals_and_does_not_consume_update_once_latch() {
        i18n::init_for_tests();
        let directory = tempfile::tempdir().unwrap();
        let mut state = State::without_song(directory.path()).unwrap();
        state.panel = Some(Panel::EditorHelp(0));
        state.update_host_ftue();
        assert!(matches!(state.panel, Some(Panel::EditorHelp(0))));
        state.panel = None;
        state.offer_available_update("v999", "https://example.com/release");
        assert!(state.panel.is_none());
        state.update_host_ftue();
        assert!(matches!(state.panel, Some(Panel::Ftue(0))));
    }
}
#[cfg(test)]
mod memory_ftue_publication {
 use super::*;
 #[test]
 fn imported_ftue_progress_is_saved_without_path_or_history(){
  i18n::init_for_tests();let dir=tempfile::tempdir().unwrap();let mut state=State::without_song(dir.path()).unwrap();let file=state.capture_application_preferences_file().unwrap();let draft=state.workspace.prepare_source_preferences(&file,br#"{"FtueIndex":2,"VendorPreserved":7}"#,deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();state.workspace.import_source_preferences(draft,&file,true).unwrap();state.sync_source_preferences_history().unwrap();
  let label=state.workspace.file().session.undo_label().map(str::to_owned);let mut value=state.ftue_preferences();value.next();assert!(state.save_ftue(value.clone()));assert_eq!(state.ftue_preferences(),value);assert_eq!(state.application_preferences_owner().unwrap().ftue(),&value);assert!(!state.application_preferences_owner().unwrap().has_changes());assert!(state.audio_preferences_file.is_none());assert!(state.audio_preferences_path.is_none());assert_eq!(state.workspace.file().session.undo_label(),label.as_deref());
 }
}
