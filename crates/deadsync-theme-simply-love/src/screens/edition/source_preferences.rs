//! Explicit GrooveAuthor preferences import. This module installs typed owners
//! incrementally; it never reloads the song or rebuilds the audio output lease.
use super::*;
use deadsync_editor::{
    Error,
    audio_preferences::{
        File,
        source_import::{self, Draft},
    },
};
#[derive(Default)]
pub(super) struct ImportUi {
    /// Runtime baseline and source archive when no persistent preferences path exists.
    pub memory: Option<File>,
    pub path: String,
    pub draft: Option<Box<Draft>>,
    pub accepted: bool,
    pub row: usize,
}
impl State {
    /// Runtime readers may consume an explicit memory import without opting
    /// file writers into a fabricated persistence path.
    pub(super) fn application_preferences_owner(&self)->Option<&File>{
        self.audio_preferences_file.as_ref().or(self.source_preferences_import.memory.as_ref())
    }
    pub(super) fn application_opening_preferences(&self,
    )->Result<deadsync_editor::open_preferences::Preferences,Error>{
        let mut file=self.application_preferences_owner().cloned().map(Ok).unwrap_or_else(File::memory)?;
        file.set_general_options(self.general_options.clone())?;file.opening()
    }
    pub(super) fn application_preferences_owner_mut(&mut self)->Option<&mut File>{
        self.audio_preferences_file.as_mut().or(self.source_preferences_import.memory.as_mut())
    }
    /// Same explicit departure barrier as disk settings, without a disk path.
    pub(super) fn flush_application_memory_preferences(&mut self)->Result<(),Error>{
        let Some(memory)=self.source_preferences_import.memory.as_ref() else{return Ok(());};
        let mut owner=Some(memory.clone());
        deadsync_editor::audio_preferences::publication::publish(&mut owner,None,(),|current|Ok(((),current.clone())),
        )?;
        self.source_preferences_import.memory=owner;
        Ok(())
    }
    pub(super) fn publish_application_preferences_owner(&mut self, file: File) {
        if file.path().as_os_str().is_empty() {
            self.audio_preferences_file = None;
            self.audio_preferences_path = None;
            self.source_preferences_import.memory = Some(file);
        } else {
            self.audio_preferences_path = Some(file.path().to_owned());
            self.audio_preferences_file = Some(file);
            self.source_preferences_import.memory = None;
        }
    }
    /// Global source Player follows the active chart. A chartless import keeps
    /// its raw signed value until the first actual chart supplies a limit.
    pub(super) fn source_player_preference(&self) -> Option<i32> {
        let file = self
            .audio_preferences_file
            .as_ref()
            .or(self.source_preferences_import.memory.as_ref())?;
        file.source_archive()?;
        Some(if self.workspace.has_chart() {
            self.workspace.player() as i32
        } else {
            file.preferred_player()
        })
    }
    pub(super) fn publish_source_player_preference(&mut self) {
        if !self.workspace.has_chart() {
            return;
        }
        let player = self.workspace.player() as i32;
        if let Some(file) = self
            .audio_preferences_file
            .as_mut()
            .or(self.source_preferences_import.memory.as_mut())
        {
            if file.source_archive().is_some() {
                file.set_preferred_player(player);
            }
        }
    }
    pub(super) fn install_source_player_preference(&mut self, raw: i32) -> Result<(), Error> {
        self.workspace.source_preference_player(raw)?;
        let effective = if self.workspace.has_chart() {
            self.workspace.player() as i32
        } else {
            raw
        };
        if let Some(file) = self
            .audio_preferences_file
            .as_mut()
            .or(self.source_preferences_import.memory.as_mut())
        {
            file.set_preferred_player(effective);
        }
        Ok(())
    }
    pub(super) fn capture_application_preferences_file(&self) -> Result<File, Error> {
        let mut file = self
            .audio_preferences_file
            .clone()
            .or_else(|| self.source_preferences_import.memory.clone())
            .map(Ok)
            .unwrap_or_else(File::memory)?;
        file.set_recent_songs(self.file_menu.recent_preferences().clone())?;
        file.set_log_preferences(self.log_preferences());
        file.set_performance(self.performance_preferences())?;
        file.set_ftue(self.ftue_preferences());
        file.set_audio(self.audio_preferences);
        file.set_waveform(self.waveform_preferences);
        file.set_zoom(self.zoom_preferences);
        file.set_scroll(self.scroll_preferences);
        file.set_spacing_preference(self.spacing_preference);
        file.set_display_options(self.display_options);
        file.set_general_options(self.general_options.clone())?;
        file.set_receptors(self.receptor_preferences());
        file.set_dark(self.dark_preferences())?;
        file.set_streams(self.stream_preferences())?;
        file.set_density(self.density_preferences())?;
        file.set_selection(self.selection_preferences)?;
        // The palette cache includes the live inspector preview. A canonical
        // publication baseline must contain only committed colors: Save merges
        // the staged colors once, and Cancel must never leak them to another owner.
        file.set_note_colors(self.note_color_draft.as_ref()
            .map(|draft| draft.baseline())
            .unwrap_or_else(|| Arc::clone(&self.note_colors)),
        )?;
        file.set_key_bindings(Arc::clone(&self.command_bindings.preferences))?;
        file.set_minimap(self.minimap.preferences);
        file.set_minimap_position(self.minimap.position);
        file.set_minimap_content(self.minimap.content.options)?;
        file.set_autogen(self.autogen_preferences.clone());
        let mut snap = file.snap();
        snap.level = deadsync_editor::snap::Level::native(self.workspace.snap())?;
        file.set_snap(snap);
        file.set_note_entry_mode(if self.workspace.auto_advance() {
            deadsync_editor::lane_entry::Mode::AdvanceBySnap
        } else {
            deadsync_editor::lane_entry::Mode::Normal
        });
        if self.workspace.has_chart() {
            file.set_preferred_player(
                i32::try_from(self.workspace.player())
                    .map_err(|_| Error::InvalidEdit("Invalid active player".into()))?,
            );
        }
        Ok(file)
    }
    pub(super) fn validate_source_preferences_effects(&self, file: &File) -> Result<(), Error> {
        let mut zoom = self.zoom.clone();
        file.zoom().apply(&mut zoom)?;
        for (i, mode) in [
            deadsync_editor::zoom::SpacingMode::ConstantTime,
            deadsync_editor::zoom::SpacingMode::ConstantRow,
            deadsync_editor::zoom::SpacingMode::Variable,
        ]
        .into_iter()
        .enumerate()
        {
            zoom.set_spacing(
                mode,
                file.scroll()
                    .speed(i)
                    .ok_or_else(|| Error::InvalidEdit("Invalid spacing preference".into()))?,
            )?;
        }
        file.performance().validate()?;
        file.general_options().validate()?;
        file.streams().validate()?;
        file.density().validate()?;
        file.selection().validate()?;
        file.dark().validate()?;
        let colors = file.note_colors_arc();
        colors.validate()?;
        deadsync_editor::minimap_palette::Palette::new(&colors.colors)?;
        deadsync_editor::note_color_palette::Palette::capture(colors)?;
        file.key_bindings().validate()?;
        file.minimap_content()?;
        Ok(())
    }
    fn install_source_preferences_file(&mut self, file: File) -> Result<(), Error> {
        // All fallible input validation ran before the core transaction. These are
        // the same canonical installers used by the existing inspectors/history.
        let previous_audio = self.audio_preferences;
        if self.application_preferences_owner()
            .is_some_and(|old| old.advanced_save() != file.advanced_save())
        {
            self.invalidate_source_output_request();
        }
        self.configure_display_options(file.display_options());
        self.configure_general_options(file.general_options().clone())?;
        self.configure_autogen_preferences(file.autogen());
        self.configure_receptor_preferences(file.receptors());
        self.configure_dark_preferences(file.dark());
        self.configure_stream_preferences(file.streams())?;
        self.configure_density_preferences(file.density())?;
        self.selection_preferences = file.selection();
        self.configure_note_color_arc(file.note_colors_arc())?;
        self.configure_minimap_preferences(file.minimap());
        self.configure_minimap_position_preferences(file.minimap_position());
        self.configure_minimap_content_preferences(file.minimap_content()?);
        file.zoom().apply(&mut self.zoom)?;
        self.zoom_preferences = file.zoom();
        self.install_scroll_preferences(file.scroll())?;
        self.configure_spacing_preference(file.spacing_preference());
        self.workspace.set_auto_advance(
            file.note_entry_mode() == deadsync_editor::lane_entry::Mode::AdvanceBySnap,
        );
        self.workspace.install_source_snap(file.snap().level);
        self.workspace
            .source_preference_player(file.preferred_player())?;
        self.configure_key_binding_preferences(file.key_bindings());
        self.file_menu.install_recent(file.recent_songs().clone());
        self.configure_log_preferences(file.log_preferences());
        self.configure_performance_preferences(file.performance())?;
        self.ftue = file.ftue().clone();
        self.audio_preferences = file.audio();
        self.waveform_preferences = file.waveform();
        if !file.path().as_os_str().is_empty() {
            self.audio_preferences_path = Some(file.path().to_owned());
        }
        if file.path().as_os_str().is_empty() {
            self.audio_preferences_path = None;
            self.audio_preferences_file = None;
            self.source_preferences_import.memory = Some(file);
        } else {
            self.source_preferences_import.memory = None;
            self.audio_preferences_file = Some(file);
        }
        self.sync_editor_audio_settings(previous_audio);
        self.apply_waveform_preferences();
        // DPI capture and startup-only graph/session choices deliberately remain
        // unchanged until restart; their persisted typed values are installed above.
        Ok(())
    }
    pub(super) fn sync_source_preferences_history(&mut self) -> Result<(), Error> {
        if let Some(update) = self.workspace.take_source_preferences_update() {
            self.install_source_preferences_file(update.snapshot().file().clone())?;
        }
        Ok(())
    }
}
pub(super) const IMPORT_KEYS: [&str; 3] = [
    "SourcePreferencesPath",
    "SourcePreferencesReview",
    "KeepEditing",
];
pub(super) const REVIEW_KEYS: [&str; 4] = [
    "SourcePreferencesReportRow",
    "SourcePreferencesAccept",
    "SourcePreferencesApply",
    "SourcePreferencesCancel",
];
fn read_source_file(path: &Path) -> Result<Vec<u8>, Error> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((source_import::MAX_SOURCE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > source_import::MAX_SOURCE_BYTES {
        return Err(Error::InvalidFormat(
            "Source preferences exceed the 8 MiB input limit".into(),
        ));
    }
    Ok(bytes)
}
impl State {
    pub(super) fn open_source_preferences_import(&mut self) {
        self.source_preferences_import.draft = None;
        self.source_preferences_import.accepted = false;
        self.source_preferences_import.row = 0;
        self.panel = Some(Panel::SourcePreferencesImport(0));
        self.text_buffer = None;
        self.label_preedit.clear();
        self.focus_text();
        self.status = i18n::tr("Edition", "SourcePreferencesHelp");
    }
    fn prepare_source_preferences_import(&mut self) -> Result<(), Error> {
        self.flush_wheel_settings()?;
        if !self.sync_text() || !self.label_preedit.is_empty() {
            return Err(Error::InvalidEdit(
                "Uncommitted preference path text".into(),
            ));
        }
        let raw = read_source_file(Path::new(&self.source_preferences_import.path))?;
        let current = self.capture_application_preferences_file()?;
        let draft = self.workspace.prepare_source_preferences(
            &current,
            &raw,
            deadsync_editor::key_binding_preferences::source_profile::Platform::current(),
        )?;
        self.validate_source_preferences_effects(draft.target())?;
        self.source_preferences_import.draft = Some(Box::new(draft));
        self.source_preferences_import.accepted = false;
        self.source_preferences_import.row = 0;
        self.panel = Some(Panel::SourcePreferencesReview(0));
        self.text_buffer = None;
        self.status = i18n::tr("Edition", "SourcePreferencesReviewHelp");
        Ok(())
    }
    fn apply_source_preferences_import(&mut self) -> Result<(), Error> {
        self.flush_wheel_settings()?;
        let draft = self
            .source_preferences_import
            .draft
            .as_ref()
            .ok_or_else(|| Error::InvalidEdit("No source preference review".into()))?;
        let raw = read_source_file(Path::new(&self.source_preferences_import.path))?;
        if raw != draft.raw_source() {
            return Err(Error::ExternalChange);
        }
        let current = self.capture_application_preferences_file()?;
        self.validate_source_preferences_effects(draft.target())?;
        if !self.source_preferences_import.accepted {
            return Err(Error::InvalidEdit(
                "Explicit acceptance of the source import report is required".into(),
            ));
        }
        let draft = *self.source_preferences_import.draft.take().unwrap();
        self.workspace.import_source_preferences(
            draft,
            &current,
            self.source_preferences_import.accepted,
        )?;
        self.sync_source_preferences_history()?;
        self.panel = None;
        self.refresh();
        self.status = i18n::tr("Edition", "SourcePreferencesImported");
        Ok(())
    }
    pub(super) fn source_preferences_key(&mut self, index: usize, code: KeyCode) {
        let review = matches!(self.panel, Some(Panel::SourcePreferencesReview(_)));
        if code == KeyCode::Escape {
            self.source_preferences_import.draft = None;
            self.panel = Some(Panel::GeneralOptions(4));
            self.text_buffer = None;
            self.label_preedit.clear();
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            if !self.sync_text() {
                return;
            }
            let count = if review { 4 } else { 3 };
            let backwards = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            let next = (index + if backwards { count - 1 } else { 1 }) % count;
            self.panel = Some(if review {
                Panel::SourcePreferencesReview(next)
            } else {
                Panel::SourcePreferencesImport(next)
            });
            self.focus_text();
            return;
        }
        if review
            && index == 0
            && matches!(
                code,
                KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::PageUp | KeyCode::PageDown
            )
        {
            let count = self.source_preferences_report_count().max(1);
            let step = if matches!(code, KeyCode::PageUp | KeyCode::PageDown) {
                8
            } else {
                1
            };
            let backwards = matches!(code, KeyCode::ArrowLeft | KeyCode::PageUp);
            self.source_preferences_import.row = (self.source_preferences_import.row
                + if backwards {
                    count - (step % count)
                } else {
                    step
                })
                % count;
            return;
        }
        if !matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if !review && index == 0 {
                self.text_field_key(0, code);
            }
            return;
        }
        let result = match (review, index) {
            (false, 0 | 1) => self.prepare_source_preferences_import(),
            (true, 1) => {
                self.source_preferences_import.accepted = !self.source_preferences_import.accepted;
                Ok(())
            }
            (true, 2) => self.apply_source_preferences_import(),
            (false, 2) | (true, 3) => {
                self.source_preferences_import.draft = None;
                self.panel = Some(Panel::GeneralOptions(4));
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.status = i18n::tr_fmt(
                "Edition",
                "SourcePreferencesFailed",
                &[("error", &i18n::editor_error(&error))],
            );
        }
    }
    fn source_preferences_report_count(&self) -> usize {
        let Some(draft) = self.source_preferences_import.draft.as_ref() else {
            return 0;
        };
        let report = draft.report();
        report.items.len()
            + report.key_bindings.as_ref().map_or(0, |k| {
                k.applied.len()
                    + k.applied_modifiers.len()
                    + k.rejected.len()
                    + k.unavailable_commands.len()
                    + k.retained_modifiers.len()
                    + k.retained_unknown.len()
                    + k.neutralized_native_extensions.len()
                    + k.rescue_commands.len()
            })
            + 1
    }
    pub(super) fn source_preferences_label(&self, index: usize, key: &str) -> Arc<str> {
        if matches!(self.panel, Some(Panel::SourcePreferencesImport(_))) {
            return if index == 0 {
                self.text_field_label(0)
            } else {
                i18n::tr("Edition", key)
            };
        }
        if index == 1 {
            return i18n::tr_fmt(
                "Edition",
                "SourcePreferencesAcceptance",
                &[(
                    "value",
                    &i18n::tr(
                        "Edition",
                        if self.source_preferences_import.accepted {
                            "WaveformYes"
                        } else {
                            "WaveformNo"
                        },
                    ),
                )],
            );
        }
        if index != 0 {
            return i18n::tr("Edition", key);
        }
        let Some(draft) = self.source_preferences_import.draft.as_ref() else {
            return i18n::tr("Edition", "SourcePreferencesNoReview");
        };
        let report = draft.report();
        let row = self.source_preferences_import.row;
        if let Some(item) = report.items.get(row) {
            let classification = match item.disposition {
                source_import::Disposition::Applied => "SourcePreferencesApplied",
                source_import::Disposition::Defaulted => "SourcePreferencesDefaulted",
                source_import::Disposition::Retained => "SourcePreferencesRetained",
                source_import::Disposition::Normalized => "SourcePreferencesNormalized",
                source_import::Disposition::EnumFallback => "SourcePreferencesFallback",
                source_import::Disposition::Adapted => {
                    if item.reason.starts_with("Unknown accumulation uses Row") {
                        "SourcePreferencesAccumulationAdapted"
                    } else if item.reason.starts_with("Source window visibility retained") {
                        "SourcePreferencesWindowAdapted"
                    } else {
                        "SourcePreferencesAdapted"
                    }
                }
            };
            let compact = if let Some(raw) = item.source_token.as_deref() {
                raw.chars()
                    .filter(|c| !c.is_control())
                    .take(100)
                    .collect::<String>()
            } else {
                item.raw
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "-".into())
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(100)
                    .collect::<String>()
            };
            let effective = item.effective.as_ref().map(|value| {
                value
                    .to_string()
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(100)
                    .collect::<String>()
            });
            let detail = if let Some(effective) = effective {
                i18n::tr_fmt(
                    "Edition",
                    "SourcePreferencesValueChange",
                    &[("source", &compact), ("effective", &effective)],
                )
                .to_string()
            } else {
                compact
            };
            return format!(
                "{}/{}  {} - {} : {}",
                row + 1,
                self.source_preferences_report_count(),
                item.path,
                i18n::tr("Edition", classification),
                detail
            )
            .into();
        }
        let mut offset = row.saturating_sub(report.items.len());
        if let Some(keys) = &report.key_bindings {
            for values in [&keys.applied, &keys.applied_modifiers] {
                if let Some(id) = values.get(offset) {
                    return format!(
                        "{} : {}",
                        i18n::tr("Edition", "SourcePreferencesApplied"),
                        id
                    )
                    .into();
                }
                offset = offset.saturating_sub(values.len());
            }
            if let Some(rejected) = keys.rejected.get(offset) {
                return format!(
                    "{} : {}",
                    i18n::tr("Edition", "SourcePreferencesKeyRejected"),
                    rejected.id
                )
                .into();
            }
            offset = offset.saturating_sub(keys.rejected.len());
            for (kind, values) in [
                (
                    "SourcePreferencesKeyUnavailable",
                    &keys.unavailable_commands,
                ),
                ("SourcePreferencesRetained", &keys.retained_modifiers),
                ("SourcePreferencesRetained", &keys.retained_unknown),
                (
                    "SourcePreferencesKeyNeutralized",
                    &keys.neutralized_native_extensions,
                ),
                ("SourcePreferencesKeyRescue", &keys.rescue_commands),
            ] {
                if let Some(id) = values.get(offset) {
                    return format!("{} : {}", i18n::tr("Edition", kind), id).into();
                }
                offset = offset.saturating_sub(values.len());
            }
        }
        i18n::tr_fmt(
            "Edition",
            "SourcePreferencesSummary",
            &[
                ("rows", &report.items.len().to_string()),
                ("omitted", &report.omitted_items.to_string()),
            ],
        )
    }
}

// Insert in source_preferences.rs's cfg(test) module (never inside impl State).
#[cfg(test)]
mod tests {
    use super::*;
    fn state(root: &Path) -> State {
        let path = root.join("song.ssc");
        std::fs::write(
            &path,
            "#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        State::open(&path, 0).unwrap()
    }
    #[test]
    fn source_import_keyboard_review_cancel_and_consumers_preserve_fractional_focus() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut state = state(root.path());
        state.navigate_exact_position(60.5, true);
        state.audio_preferences_file = None;
        state.audio_preferences_path = None;
        let path = root.path().join("source.json");
        let raw=br#"{// exact archive
 "PreferencesDark":{"ShowDarkBg":true},"PreferencesScroll":{"SpacingMode":"ConstantRow","RowBasedWaveFormScrollMode":"CurrentTempo"},"PreferencesSelection":{"RegionSelectionMode":"Any"},"Player":7,}"#;
        std::fs::write(&path, raw).unwrap();
        let chart = state.workspace.chart();
        let row = state.workspace.row();
        let position = state.focus_position.clone();
        let audio_owner = state.audio_session.scope.owner;
        state.open_source_preferences_import();
        state.source_preferences_import.path = path.to_string_lossy().into_owned();
        state.text_buffer = None;
        state.source_preferences_key(1, KeyCode::Enter);
        assert!(matches!(
            state.panel,
            Some(Panel::SourcePreferencesReview(0))
        ));
        assert!(!state.source_preferences_import.accepted);
        assert!(
            state
                .source_preferences_import
                .draft
                .as_ref()
                .unwrap()
                .report()
                .requires_acknowledgement()
        );
        state.source_preferences_key(0, KeyCode::Escape);
        assert!(state.audio_preferences_file.is_none());
        assert!(state.source_preferences_import.draft.is_none());
        state.open_source_preferences_import();
        state.text_buffer = None;
        state.source_preferences_key(1, KeyCode::Enter);
        state.source_preferences_key(1, KeyCode::Enter);
        assert!(state.source_preferences_import.accepted);
        state.source_preferences_key(2, KeyCode::Enter);
        assert!(state.panel.is_none(), "{}", state.status);
        assert_eq!(state.workspace.chart(), chart);
        assert_eq!(state.workspace.row(), row);
        assert_eq!(
            state.focus_position.as_ref().unwrap().row(),
            position.as_ref().unwrap().row()
        );
        assert_eq!(state.audio_session.scope.owner, audio_owner);
        assert_eq!(
            state.spacing_mode,
            deadsync_editor::zoom::SpacingMode::ConstantRow
        );
        assert_eq!(state.workspace.player(), 0);
        assert_eq!(
            state
                .source_preferences_import
                .memory
                .as_ref()
                .unwrap()
                .source_archive()
                .unwrap()["RawUtf8"]
                .as_str()
                .unwrap()
                .as_bytes(),
            raw
        );
        state.command(Command::Undo);
        assert!(
            state
                .source_preferences_import
                .memory
                .as_ref()
                .unwrap()
                .source_archive()
                .is_none()
        );
        assert_eq!(state.focus_position.as_ref().unwrap().row(), 60.5);
        state.command(Command::Redo);
        assert!(
            state
                .source_preferences_import
                .memory
                .as_ref()
                .unwrap()
                .source_archive()
                .is_some()
        );
    }
    #[test]
    fn source_import_rejects_changed_source_after_review_without_publication() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut state = state(root.path());
        let path = root.path().join("source.json");
        std::fs::write(&path, b"{}").unwrap();
        state.open_source_preferences_import();
        state.source_preferences_import.path = path.to_string_lossy().into_owned();
        state.text_buffer = None;
        state.source_preferences_key(1, KeyCode::Enter);
        state.source_preferences_key(1, KeyCode::Enter);
        std::fs::write(&path, b"{\"Player\":1}").unwrap();
        state.source_preferences_key(2, KeyCode::Enter);
        assert!(state.audio_preferences_file.is_none());
        assert!(matches!(
            state.panel,
            Some(Panel::SourcePreferencesReview(_))
        ));
        assert!(
            state.take_audio_commands().iter().all(|command| !matches!(
                command,
                deadsync_gameplay::GameplayAudioCommand::StopMusic
            ))
        );
    }
    #[test]
    fn source_import_no_song_keyboard_performance_ftue_and_dpi_deferred_en_fr() {
        for language in ["en", "fr"] {
            crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let mut state = State::without_song(root.path()).unwrap();
            state.audio_preferences_file = None;
            state.audio_preferences_path = None;
            let path = root.path().join("source.json");
            std::fs::write(&path,br#"{"PreferencesPerformance":{"ShowPerformanceWindow":true,"MaxFramesToDraw":64},"LastCompletedFtueVersion":"0.1.0","FtueIndex":5}"#).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            let dpi = state.ui_dpi;
            state.panel = Some(Panel::GeneralOptions(4));
            state.general_options_key(4, KeyCode::Enter);
            assert!(matches!(
                state.panel,
                Some(Panel::SourcePreferencesImport(0))
            ));
            state.source_preferences_import.path = path.to_string_lossy().into_owned();
            state.text_buffer = None;
            state.source_preferences_key(1, KeyCode::Enter);
            assert!(matches!(
                state.panel,
                Some(Panel::SourcePreferencesReview(0))
            ));
            let label = state.source_preferences_label(1, "SourcePreferencesAccept");
            assert!(!label.is_empty());
            state.source_preferences_key(1, KeyCode::Enter);
            state.source_preferences_key(2, KeyCode::Enter);
            assert!(state.panel.is_none(), "{}", state.status);
            assert!(!state.has_song());
            assert_eq!(state.performance_preferences().max_frames, 64);
            assert!(state.performance_preferences().visible);
            assert!(state.ftue_preferences().completed());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert_eq!(state.ui_dpi, dpi);
            state.command(Command::Undo);
            assert_eq!(
                state.performance_preferences(),
                deadsync_editor::performance::preferences::Preferences::default()
            );
            assert!(!state.ftue_preferences().completed());
            assert!(!state.has_song());
            state.command(Command::Redo);
            assert!(state.ftue_preferences().completed());
            assert_eq!(state.performance_preferences().max_frames, 64);
            assert!(state.audio_preferences_file.is_none());
            assert!(state.audio_preferences_path.is_none());
            state.begin_ftue();
            assert_eq!(
                state.ftue_preferences().page(),
                Some(0),
                "pathless import must not activate filesystem preference writes"
            );
        }
    }
}

#[cfg(test)]
mod source_player_tests {
    use super::*;
    fn routine(root: &Path) -> PathBuf {
        let path = root.join("routine.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#TITLE:Routine;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:10000000\n&\n00001000;").unwrap();
        path
    }
    fn import(state: &mut State, raw: &[u8]) {
        let file = state.capture_application_preferences_file().unwrap();
        let draft = state
            .workspace
            .prepare_source_preferences(
                &file,
                raw,
                deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            )
            .unwrap();
        state
            .workspace
            .import_source_preferences(draft, &file, true)
            .unwrap();
        state.sync_source_preferences_history().unwrap();
    }
    #[test]
    fn chartless_source_player_survives_open_clamps_then_follows_live_player_across_snapshot() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = routine(root.path());
        let mut source = State::without_song(root.path()).unwrap();
        source.audio_preferences_file = None;
        source.audio_preferences_path = None;
        import(&mut source, br#"{"Player":7}"#);
        assert_eq!(source.source_player_preference(), Some(7));
        assert!(source.audio_preferences_file.is_none());
        let mut target = State::open(&path, 0).unwrap();
        target.audio_preferences_file = None;
        target.audio_preferences_path = None;
        pack_song_list::application_preferences::Snapshot::capture(&source)
            .install(&mut target)
            .unwrap();
        assert_eq!(target.workspace.grid().players(), 2);
        assert_eq!(target.workspace.player(), 1);
        assert_eq!(
            target
                .source_preferences_import
                .memory
                .as_ref()
                .unwrap()
                .preferred_player(),
            1
        );
        // Native SelectPlayer is authoritative after import, even if the archived
        // file baseline still carries the prior selected value.
        target.command(Command::SelectPlayer(0));
        assert_eq!(
            target
                .source_preferences_import
                .memory
                .as_ref()
                .unwrap()
                .preferred_player(),
            0
        );
        let mut next = State::open(&path, 0).unwrap();
        pack_song_list::application_preferences::Snapshot::capture(&target)
            .install(&mut next)
            .unwrap();
        assert_eq!(next.workspace.player(), 0);
        assert_eq!(next.source_player_preference(), Some(0));
        let mut closed = State::without_song(root.path()).unwrap();
        pack_song_list::application_preferences::Snapshot::capture(&target)
            .install(&mut closed)
            .unwrap();
        assert_eq!(closed.source_player_preference(), Some(0));
        assert!(!closed.has_song());
    }
    #[test]
    fn persisted_source_player_load_clamps_without_writing_preferences_and_chartless_keeps_signed_value()
     {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = routine(root.path());
        let prefs = root.path().join("preferences.json");
        for (raw, expected) in [(7, 1), (-5, 0)] {
            std::fs::write(&prefs,format!(r#"{{"Version":1,"Player":{raw},"GrooveAuthorSourceArchive":{{"RawUtf8":"{{}}"}}}}"#)).unwrap();
            let bytes = std::fs::read(&prefs).unwrap();
            let state = State::open_with_preferences(&path, 0, &prefs).unwrap();
            assert_eq!(state.workspace.player(), expected);
            assert_eq!(
                state
                    .audio_preferences_file
                    .as_ref()
                    .unwrap()
                    .preferred_player(),
                expected as i32
            );
            assert_eq!(std::fs::read(&prefs).unwrap(), bytes);
            let mut no_song = State::without_song(root.path()).unwrap();
            no_song.configure_audio_preferences(&prefs).unwrap();
            assert_eq!(no_song.source_player_preference(), Some(raw));
            assert!(!no_song.has_song());
        }
    }
    #[test]
    fn a_reviewed_source_player_cannot_be_applied_after_chart_focus_changes() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("two.ssc");
        std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0100;").unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.audio_preferences_file = None;
        state.audio_preferences_path = None;
        let file = state.capture_application_preferences_file().unwrap();
        let draft = state
            .workspace
            .prepare_source_preferences(
                &file,
                br#"{"Player":1}"#,
                deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            )
            .unwrap();
        state.workspace.execute(Command::SelectChart(1)).unwrap();
        let current = state.capture_application_preferences_file().unwrap();
        assert!(
            state
                .workspace
                .import_source_preferences(draft, &current, true)
                .is_err()
        );
        assert!(state.workspace.take_source_preferences_update().is_none());
        assert_eq!(state.workspace.chart(), 1);
        assert_eq!(state.workspace.player(), 0);
    }
}

#[cfg(test)]
mod source_recent_memory_tests {
    use super::*;
    #[test]
    fn live_pathless_recent_changes_are_authenticated_and_survive_document_replacement() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let mut source = State::without_song(root.path()).unwrap();
        source.audio_preferences_file = None;
        source.audio_preferences_path = None;
        let old = deadsync_editor::recent_songs::Song {
            file: root.path().join("old.ssc"),
            title: Some("Old".into()),
            pack: None,
            spacing_zoom: 1.0,
            chart_position: 0.0,
            active: vec![],
            focused: None,
        };
        let mut recent = source.file_menu.recent_preferences().clone();
        recent.record(&old).unwrap();
        source.file_menu.install_recent(recent);
        let before = source.capture_application_preferences_file().unwrap();
        let draft = source
            .workspace
            .prepare_source_preferences(
                &before,
                br#"{"RecentFiles":[]}"#,
                deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            )
            .unwrap();
        source
            .workspace
            .import_source_preferences(draft, &before, true)
            .unwrap();
        source.sync_source_preferences_history().unwrap();
        let song = deadsync_editor::recent_songs::Song {
            file: root.path().join("new.ssc"),
            title: Some("New".into()),
            pack: None,
            spacing_zoom: 1.0,
            chart_position: 0.0,
            active: vec![],
            focused: None,
        };
        let mut recent = source.file_menu.recent_preferences().clone();
        recent.record(&song).unwrap();
        source.file_menu.install_recent(recent);
        let current = source.capture_application_preferences_file().unwrap();
        assert_eq!(current.recent_songs().entries.len(), 1);
        source
            .workspace
            .capture_source_preferences_context(&current);
        assert!(source.workspace.execute(Command::Undo).is_err());
        assert!(source.workspace.take_source_preferences_update().is_none());
        let mut next = State::without_song(root.path()).unwrap();
        pack_song_list::application_preferences::Snapshot::capture(&source)
            .install(&mut next)
            .unwrap();
        assert_eq!(
            next.file_menu.recent_preferences(),
            source.file_menu.recent_preferences()
        );
        assert_eq!(
            next.capture_application_preferences_file()
                .unwrap()
                .recent_songs()
                .entries
                .len(),
            1
        );
        assert!(next.audio_preferences_file.is_none());
        assert!(
            next.source_preferences_import
                .memory
                .as_ref()
                .unwrap()
                .source_archive()
                .is_some()
        );
    }
}

#[cfg(test)]
mod source_memory_consumer_tests{
 use super::*;
 fn import(state:&mut State,raw:&[u8]){let file=state.capture_application_preferences_file().unwrap();let draft=state.workspace.prepare_source_preferences(&file,raw,deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            ).unwrap();state.workspace.import_source_preferences(draft,&file,true).unwrap();state.sync_source_preferences_history().unwrap();}
 #[test]fn pathless_snap_lock_opening_and_suppression_have_live_consumers_and_restore_history(){
  crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        ).unwrap();let mut state=State::open(&path,0).unwrap();state.audio_preferences_file=None;state.audio_preferences_path=None;
  import(&mut state,br#"{"SnapIndex":1,"SnapLockIndex":1,"PreferencesOptions":{"DefaultStepsType":"dance_double","SuppressExternalSongModificationNotification":true}}"#);
  assert!(state.audio_preferences_file.is_none());assert!(state.options_snapshot().suppress_external_song);assert_eq!(state.application_opening_preferences().unwrap().steps_type(),Some("dance-double"));
  state.cycle_snap(true);assert_eq!(state.workspace.snap(),deadsync_editor::snap::LEGACY_OFF_INDEX);
  let mut general=state.general_options.clone();general.default_steps_type=0;state.configure_general_options(general).unwrap();assert_eq!(state.application_opening_preferences().unwrap().steps_type(),Some("dance-single"));
  state.restore_all_options();assert!(!state.options_snapshot().suppress_external_song);state.command(Command::Undo);assert!(state.options_snapshot().suppress_external_song);state.command(Command::Redo);assert!(!state.options_snapshot().suppress_external_song);assert!(state.audio_preferences_file.is_none());
 }
 #[test]fn pathless_source_new_song_offset_is_used_by_real_new_song_action(){
  crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let mut state=State::without_song(root.path()).unwrap();state.audio_preferences_file=None;state.audio_preferences_path=None;
  import(&mut state,br#"{"PreferencesOptions":{"NewSongSyncOffset":0.015}}"#,
        );state.command_file_new();assert!(state.has_song());assert!((state.workspace.song_properties().unwrap().sync_offset()-0.015).abs()<1e-12);assert!(state.audio_preferences_file.is_none());
 }
}

#[cfg(test)]
mod chart_player_focus_memory_tests {
 use super::*;
 #[test]
 fn native_focus_round_trip_restores_owner_and_explicit_choice_invalidates_peer_cache(){
  crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("players.ssc");
  std::fs::write(&path,b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;").unwrap();
  let mut state=State::open(&path,0).unwrap();state.command(Command::SelectPlayer(1));
  let instant=state.chart_focus_time().unwrap();assert!(state.focus_chart_at_time(1,instant));assert_eq!(state.workspace.player(),0);
  assert!(state.focus_chart_at_time(0,instant));assert_eq!(state.workspace.player(),1);
  state.command(Command::Place(0));assert!(state.workspace.grid().get(deadsync_editor::notes::Position{player:1,lane:0,row:0}).is_some());
  state.command(Command::Undo);assert!(state.focus_chart_at_time(1,instant));state.command(Command::SelectPlayer(0));
  assert!(state.focus_chart_at_time(0,instant));assert_eq!(state.workspace.player(),0);
 }
}

#[cfg(test)]
mod canonical_preference_owner_workflows {
    use super::*;
    fn imported() -> (tempfile::TempDir, State) {
        i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::without_song(dir.path()).unwrap();
        let file = state.capture_application_preferences_file().unwrap();
        let draft = state.workspace.prepare_source_preferences(&file,
            br#"{"VendorPreserved":{"opaque":true},"PreferencesScroll":{"Reverse":true},"PreferencesOptions":{"OpenLastOpenedFileOnLaunch":false}}"#,
            deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();
        state.workspace.import_source_preferences(draft,&file,true).unwrap();
        state.sync_source_preferences_history().unwrap();
        (dir,state)
    }
    fn memory(state:&State) {
        assert!(state.audio_preferences_file.is_none());
        assert!(state.audio_preferences_path.is_none());
        let owner=state.application_preferences_owner().unwrap();
        assert!(owner.path().as_os_str().is_empty());
        assert!(!owner.has_changes());
        assert!(owner.source_archive().is_some());
    }
    #[test]
    fn receptor_and_stream_panels_publish_memory_and_undo_redo_real_consumers() {
        let (_dir,mut state)=imported();
        let before=state.receptor_preferences();
        state.open_receptor_settings();
        state.receptor_settings_key(0,KeyCode::Enter);
        state.receptor_settings_key(9,KeyCode::Enter);
        assert_eq!(state.receptor_preferences().hide_arrows,!before.hide_arrows);memory(&state);
        state.command(Command::Undo);assert_eq!(state.receptor_preferences(),before);memory(&state);
        state.command(Command::Redo);assert_eq!(state.receptor_preferences().hide_arrows,!before.hide_arrows);memory(&state);
        let before=state.stream_preferences();
        state.open_stream_settings();
        let mut after=before;after.show_break_lengths=!before.show_break_lengths;
        state.stream_draft=Some(after);state.stream_settings_key(8,KeyCode::Enter);
        assert_eq!(state.stream_preferences(),after);memory(&state);
        state.command(Command::Undo);assert_eq!(state.stream_preferences(),before);memory(&state);
        state.command(Command::Redo);assert_eq!(state.stream_preferences(),after);memory(&state);
    }
    #[test]
    fn scroll_restore_general_display_and_direct_zoom_keep_canonical_memory() {
        let (_dir,mut state)=imported();
        assert!(state.scroll_preferences.reverse());
        state.restore_source_scroll();assert!(!state.scroll_preferences.reverse());memory(&state);
        state.command(Command::Undo);assert!(state.scroll_preferences.reverse());memory(&state);
        state.command(Command::Redo);assert!(!state.scroll_preferences.reverse());memory(&state);
        let mut general=state.general_options.clone();general.open_last=!general.open_last;
        state.commit_general_options(general.clone()).unwrap();assert_eq!(state.general_options,general);memory(&state);
        let mut display=state.display_options;display.render.notes=!display.render.notes;
        state.commit_display_options(display).unwrap();assert_eq!(state.render_policy().notes,display.render.notes);memory(&state);
        state.panel=None;state.open_zoom_settings();
        state.zoom_settings_key(0,KeyCode::ArrowRight);state.zoom_settings_key(5,KeyCode::Enter);memory(&state);
        assert_eq!(state.application_preferences_owner().unwrap().zoom(),state.zoom_preferences);
        let undo=state.workspace.file().session.undo_label().map(str::to_owned);
        state.set_source_scroll_speed(0,450.).unwrap();memory(&state);
        assert_eq!(state.application_preferences_owner().unwrap().scroll().speed(0),Some(450.));
        assert_eq!(state.zoom.spacing(deadsync_editor::zoom::SpacingMode::ConstantTime),450.);
        assert_eq!(state.workspace.file().session.undo_label(),undo.as_deref());
    }
}
#[cfg(test)]
mod direct_memory_owner_workflows {
 use super::*;
 fn imported()->(tempfile::TempDir,State){
  i18n::init_for_tests();let dir=tempfile::tempdir().unwrap();let mut state=State::without_song(dir.path()).unwrap();let file=state.capture_application_preferences_file().unwrap();
  let draft=state.workspace.prepare_source_preferences(&file,br#"{"VendorPreserved":{"opaque":[3,7]},"PreferencesOptions":{"OpenLastOpenedFileOnLaunch":false}}"#,deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();state.workspace.import_source_preferences(draft,&file,true).unwrap();state.sync_source_preferences_history().unwrap();(dir,state)
 }
 #[test]
 fn snap_note_entry_spacing_autogen_and_log_keep_pathless_owner_and_no_undo(){
  let (_dir,mut state)=imported();let undo=state.workspace.file().session.undo_label().map(str::to_owned);
  state.cycle_snap(true);assert_eq!(state.application_preferences_owner().unwrap().snap().level.native_index(),state.workspace.snap());assert!(!state.application_preferences_owner().unwrap().has_changes());
  state.workspace.set_auto_advance(true);state.publish_note_entry_mode();assert_eq!(state.application_preferences_owner().unwrap().note_entry_mode(),deadsync_editor::lane_entry::Mode::AdvanceBySnap);
  state.cycle_source_spacing_mode();assert_eq!(state.application_preferences_owner().unwrap().spacing_preference(),state.spacing_preference());
  state.generation.profiles.performed=None;state.generation.preset=1;state.publish_autogen_profile_preferences();assert_eq!(state.application_preferences_owner().unwrap().autogen(),state.autogen_preferences());
  state.flush_wheel_settings().unwrap();assert!(!state.application_preferences_owner().unwrap().has_changes());assert!(state.application_preferences_owner().unwrap().source_archive().is_some());
  state.panel=None;state.open_log();assert!(state.log_preferences().show);assert_eq!(state.application_preferences_owner().unwrap().log_preferences(),state.log_preferences());state.log_key(KeyCode::Escape);assert!(!state.log_preferences().show);assert!(!state.application_preferences_owner().unwrap().has_changes());
  assert!(state.audio_preferences_file.is_none());assert!(state.audio_preferences_path.is_none());assert_eq!(state.workspace.file().session.undo_label(),undo.as_deref());
 }
}
