//! Shared keyboard text editing for chart and song property drafts.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{
    chart_text::Field as ChartField,
    note_attributes::Field as NoteField,
    song_text::Field as SongField,
    text_buffer::{Buffer, Movement},
};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Audio(deadsync_editor::audio_preferences::Field),
    PreferenceNumeric,
    SaveAsDestination,
    FileOpenPath,
    SourcePreferencesPath,
    Chart(ChartField),
    Song(SongField),
    Note(NoteField),
    NoteRaw(NoteField),
    NoteReserved,
    Attack(usize),
    Pack(super::pack_properties::Field),
}
impl Target {
    pub(super) fn file_kind(self) -> Option<deadsync_editor::music_file::Kind> {
        match self {
            Self::Chart(ChartField::Music) => Some(deadsync_editor::music_file::Kind::Audio),
            Self::Song(field) => field.file_kind(),
            Self::Pack(super::pack_properties::Field::Banner) => {
                Some(deadsync_editor::music_file::Kind::Image)
            }
            _ => None,
        }
    }
    pub(super) fn is_file(self) -> bool {
        self.file_kind().is_some()
    }
    pub(super) fn is_music(self) -> bool {
        matches!(
            self,
            Self::Chart(ChartField::Music) | Self::Song(SongField::Music | SongField::PreviewMusic)
        )
    }
    pub(super) fn is_preview(self) -> bool {
        matches!(self, Self::Song(SongField::PreviewMusic))
    }
    pub(super) fn is_song(self) -> bool {
        matches!(self, Self::Song(_))
    }
    fn key(self) -> &'static str {
        match self {
            Self::Audio(_) => "AudioValue",
            Self::PreferenceNumeric => "PreferenceNumericValue",
            Self::SourcePreferencesPath => "SourcePreferencesPath",
            Self::FileOpenPath => "FileOpenPath",
            Self::SaveAsDestination => "SaveAsDestinationValue",
            Self::Attack(index) => super::attacks::FIELD_KEYS[index],
            Self::Pack(field) => field.key(),
            Self::Note(NoteField::Head) => "NoteAttributesHead",
            Self::Note(NoteField::Tail) => "NoteAttributesTail",
            Self::NoteReserved => "NoteAttributesReserved",
            Self::NoteRaw(_) => "NoteAttributesRaw",
            Self::Chart(field) => match field {
                ChartField::Name => "ChartName",
                ChartField::Description => "ChartDescription",
                ChartField::Credit => "ChartCredit",
                ChartField::Style => "ChartStyle",
                ChartField::Music => "ChartMusic",
            },
            Self::Song(field) => match field {
                SongField::Title => "SongTitle",
                SongField::Subtitle => "SongSubtitle",
                SongField::Artist => "SongArtist",
                SongField::Credit => "SongCredit",
                SongField::Genre => "SongGenre",
                SongField::Origin => "SongOrigin",
                SongField::TitleTransliteration => "SongTitleTranslit",
                SongField::SubtitleTransliteration => "SongSubtitleTranslit",
                SongField::ArtistTransliteration => "SongArtistTranslit",
                SongField::Music => "SongMusic",
                SongField::PreviewMusic => "SongPreviewMusic",
                SongField::Banner => "SongBanner",
                SongField::Background => "SongBackground",
                SongField::CdTitle => "SongCdTitle",
                SongField::Jacket => "SongJacket",
                SongField::CdImage => "SongCdImage",
                SongField::DiscImage => "SongDiscImage",
                SongField::PreviewVideo => "SongPreviewVideo",
                SongField::Lyrics => "SongLyrics",
            },
        }
    }
}
impl State {
    fn field_at(&self, index: usize) -> Option<Target> {
        match self.panel {
            Some(Panel::AudioSettings(1)) if index == 1 && self.audio_numeric_text.is_some() => {
                Some(Target::Audio(
                    deadsync_editor::audio_preferences::Field::ALL[self.audio_field],
                ))
            }
            Some(Panel::SourcePreferencesImport(_)) if index == 0 => {
                Some(Target::SourcePreferencesPath)
            }
            Some(Panel::FileOpen(_)) if index == 0 => Some(Target::FileOpenPath),
            Some(Panel::SaveAsDestination(_)) if index == 0 => Some(Target::SaveAsDestination),
            Some(Panel::Attacks(_)) if (3..8).contains(&index) => Some(Target::Attack(index - 3)),
            Some(Panel::NoteAttributes(_)) if self.note_attribute_typed => match index {
                4 => Some(Target::NoteReserved),
                5 => Some(Target::NoteRaw(self.note_attribute_inspector.endpoint())),
                _ => None,
            },
            Some(Panel::NoteAttributes(_)) => match index {
                1 => Some(Target::Note(NoteField::Head)),
                2 => Some(Target::Note(NoteField::Tail)),
                _ => None,
            },
            Some(Panel::PackProperties) => super::pack_properties::Field::ALL
                .get(index)
                .copied()
                .map(Target::Pack),
            Some(Panel::ChartText(_)) => ChartField::ALL.get(index).copied().map(Target::Chart),
            Some(Panel::SongText(_)) => SongField::ALL.get(index).copied().map(Target::Song),
            _ => None,
        }
    }
    fn field_value(&self, target: Target) -> Option<std::borrow::Cow<'_, str>> {
        match target {
            Target::Audio(_) => self.audio_numeric_text.as_deref().map(Into::into),
            Target::PreferenceNumeric => self
                .preference_numeric
                .as_ref()
                .map(|edit| edit.text.as_str().into()),
            Target::SourcePreferencesPath => {
                Some(self.source_preferences_import.path.as_str().into())
            }
            Target::FileOpenPath => Some(self.file_menu.destination.as_str().into()),
            Target::SaveAsDestination => Some(self.save_as.destination.as_str().into()),
            Target::Pack(field) => self
                .pack_properties
                .as_ref()
                .map(|settings| settings.text(field)),
            Target::NoteReserved => self
                .note_attributes
                .as_ref()
                .and_then(|draft| self.note_attribute_inspector.attributes(draft).ok())
                .map(|attributes| std::borrow::Cow::Owned(attributes.reserved.to_string())),
            Target::Attack(index) => self
                .attacks
                .as_ref()
                .map(|d| d.fields[index].as_str().into()),
            Target::Note(f) | Target::NoteRaw(f) => {
                self.note_attributes.as_ref().map(|d| d.text(f).into())
            }
            Target::Chart(f) => self.chart_properties.as_ref().map(|d| d.text(f).into()),
            Target::Song(f) => self.song_properties.as_ref().map(|d| d.text(f).into()),
        }
    }
    fn field_supported(&self, target: Target) -> bool {
        match target {
            Target::PreferenceNumeric => self.preference_numeric_is_current(),
            Target::Audio(field) => {
                self.audio_numeric_text.is_some()
                    && field == deadsync_editor::audio_preferences::Field::ALL[self.audio_field]
                    && matches!(self.panel, Some(Panel::AudioSettings(1)))
            }
            Target::SourcePreferencesPath => {
                matches!(self.panel, Some(Panel::SourcePreferencesImport(_)))
            }
            Target::FileOpenPath => matches!(self.panel, Some(Panel::FileOpen(_))),
            Target::SaveAsDestination => matches!(self.panel, Some(Panel::SaveAsDestination(_))),
            Target::Pack(_) => self.pack_text_current(),
            Target::NoteReserved => self
                .note_attributes
                .as_ref()
                .is_some_and(|draft| self.note_attribute_inspector.attributes(draft).is_ok()),
            Target::Attack(index) => self
                .attacks
                .as_ref()
                .is_some_and(|d| d.can_edit_field(index)),
            Target::Note(f) | Target::NoteRaw(f) => self
                .note_attributes
                .as_ref()
                .is_some_and(|d| f == NoteField::Head || d.note().end.is_some()),
            Target::Chart(f) => self
                .chart_properties
                .as_ref()
                .is_some_and(|d| d.supports_text(f)),
            Target::Song(f) => self
                .song_properties
                .as_ref()
                .is_some_and(|d| d.supports_text(f)),
        }
    }
    fn set_field_value(
        &mut self,
        target: Target,
        value: &str,
    ) -> Result<(), deadsync_editor::Error> {
        let missing = || deadsync_editor::Error::InvalidEdit("No property draft is active".into());
        match target {
            Target::PreferenceNumeric => {
                if value.len() > 256 || value.chars().any(char::is_control) {
                    return Err(deadsync_editor::Error::InvalidEdit(
                        "Invalid numeric text".into(),
                    ));
                }
                if !self.preference_numeric_is_current() {
                    return Err(deadsync_editor::Error::ExternalChange);
                }
                self.preference_numeric.as_mut().ok_or_else(missing)?.text = value.to_owned();
                Ok(())
            }
            Target::Audio(_) => {
                if value.len() > 256 || value.chars().any(char::is_control) {
                    return Err(deadsync_editor::Error::InvalidEdit(
                        "Invalid numeric text".into(),
                    ));
                }
                self.audio_numeric_text = Some(value.to_owned());
                Ok(())
            }
            Target::SourcePreferencesPath => {
                if value.len() > 4096 || value.chars().any(char::is_control) {
                    return Err(deadsync_editor::Error::InvalidEdit(
                        "Invalid preference import path text".into(),
                    ));
                }
                self.source_preferences_import.path = value.to_owned();
                self.source_preferences_import.draft = None;
                Ok(())
            }
            Target::FileOpenPath => {
                if value.len() > 4096 || value.chars().any(char::is_control) {
                    return Err(deadsync_editor::Error::InvalidEdit(
                        "Invalid open path text".into(),
                    ));
                }
                self.file_menu.destination = value.to_owned();
                Ok(())
            }
            Target::SaveAsDestination => {
                if value.len() > 4096 || value.chars().any(char::is_control) {
                    return Err(deadsync_editor::Error::InvalidEdit(
                        "Invalid destination text".into(),
                    ));
                }
                self.save_as.destination = value.to_owned();
                Ok(())
            }
            Target::Pack(field) => {
                if !self.pack_text_current() {
                    return Err(missing());
                }
                self.pack_properties
                    .as_mut()
                    .ok_or_else(missing)?
                    .set_text(field, value)
            }
            Target::NoteReserved => self
                .note_attribute_inspector
                .set_reserved(self.note_attributes.as_mut().ok_or_else(missing)?, value),
            Target::Attack(index) => {
                if value.chars().any(char::is_control) {
                    return Err(deadsync_editor::Error::InvalidEdit(
                        "Attack fields must occupy one line".into(),
                    ));
                }
                self.attacks.as_mut().ok_or_else(missing)?.fields[index] = value.into();
                Ok(())
            }
            Target::Note(f) | Target::NoteRaw(f) => self
                .note_attributes
                .as_mut()
                .ok_or_else(missing)?
                .set_text(f, value),
            Target::Chart(f) => self
                .chart_properties
                .as_mut()
                .ok_or_else(missing)?
                .set_text(f, value),
            Target::Song(f) => {
                self.song_properties
                    .as_mut()
                    .ok_or_else(missing)?
                    .set_text(f, value)?;
                if f == SongField::PreviewMusic {
                    self.refresh_sample_region();
                } else {
                    self.refresh_misc_draft_widgets();
                }
                Ok(())
            }
        }
    }
    pub(super) fn text_target(&self) -> Option<Target> {
        if self.preference_numeric_is_current() {
            return Some(Target::PreferenceNumeric);
        }
        if matches!(self.panel, Some(Panel::PackProperties)) {
            return if self.pack_text_current() {
                self.pack_properties.as_ref()?.field().map(Target::Pack)
            } else {
                None
            };
        }
        let index = match self.panel {
            Some(
                Panel::AudioSettings(i)
                | Panel::SourcePreferencesImport(i)
                | Panel::FileOpen(i)
                | Panel::SaveAsDestination(i)
                | Panel::ChartText(i)
                | Panel::SongText(i)
                | Panel::NoteAttributes(i)
                | Panel::Attacks(i),
            ) => i,
            _ => return None,
        };
        self.field_at(index)
            .filter(|target| self.field_supported(*target))
    }
    pub(super) fn open_chart_text(&mut self) {
        let first = if self
            .chart_properties
            .as_ref()
            .is_some_and(|d| d.supports_text(ChartField::Name))
        {
            0
        } else {
            1
        };
        self.panel = Some(Panel::ChartText(first));
        self.focus_text();
        self.status = i18n::tr(
            "Edition",
            if first == 0 {
                "ChartTextHelpSsc"
            } else {
                "ChartTextHelpSm"
            },
        );
    }
    pub(super) fn open_song_text(&mut self) {
        self.panel = Some(Panel::SongText(0));
        self.focus_text();
        self.status = i18n::tr("Edition", "SongTextHelp");
    }
    pub(super) fn focus_text(&mut self) {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        if matches!(self.text_target(), Some(Target::Pack(_))) {
            self.begin_pack_field();
        }
        self.text_buffer = self.text_target().and_then(|target| {
            self.field_value(target)
                .map(|value| Buffer::new(value.as_ref(), true))
        });
        self.label_preedit.clear();
    }
    pub(super) fn sync_text(&mut self) -> bool {
        let Some(target) = self.text_target() else {
            return true;
        };
        let Some(buffer) = self.text_buffer.as_ref() else {
            return true;
        };
        if self.field_value(target).as_deref() == Some(buffer.value()) {
            return true;
        }
        let value = buffer.value().to_owned();
        if let Err(error) = self.set_field_value(target, &value) {
            if matches!(target, Target::Pack(_))
                && !matches!(&error,deadsync_editor::Error::InvalidEdit(message) if message=="Invalid pack property value")
            {
                self.status = i18n::editor_error(&error).into();
                return false;
            }
            self.status = i18n::tr(
                "Edition",
                if matches!(target, Target::NoteReserved) {
                    "NoteAttributesReservedInvalid"
                } else if matches!(target, Target::Pack(super::pack_properties::Field::Year)) {
                    "PackInvalidYear"
                } else if matches!(target, Target::Pack(_)) {
                    "PackInvalidText"
                } else {
                    "ChartTextSingleLine"
                },
            );
            return false;
        }
        true
    }
    pub(super) fn edit_text_input(&mut self, text: &str) -> bool {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        let Some(buffer) = self.text_buffer.as_mut() else {
            return false;
        };
        buffer.insert(text);
        self.sync_text();
        self.label_preedit.clear();
        true
    }
    pub(super) fn text_field_key(&mut self, index: usize, code: KeyCode) {
        self.text_edit_serial = self.text_edit_serial.wrapping_add(1);
        let note = matches!(self.panel, Some(Panel::NoteAttributes(_)));
        let song = matches!(self.panel, Some(Panel::SongText(_)));
        let count = if song {
            SongField::ALL.len()
        } else {
            ChartField::ALL.len()
        };
        if self.text_target().is_some_and(Target::is_file) {
            if code == KeyCode::F2 {
                if self.sync_text() {
                    self.queue_music_picker();
                }
                return;
            }
            if code == KeyCode::F3 {
                self.clear_music();
                return;
            }
            if code == KeyCode::F4 && self.find_song_media() {
                return;
            }
        }
        if self.control && matches!(code, KeyCode::KeyC | KeyCode::KeyX | KeyCode::KeyV) {
            self.queue_text_clipboard(code);
            return;
        }
        if !note
            && (code == KeyCode::Escape
                || (index == count && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter)))
        {
            let accepted = self.sync_text();
            self.label_preedit.clear();
            self.text_buffer = None;
            self.panel = Some(if song {
                Panel::SongProperties(3)
            } else {
                Panel::ChartProperties(9)
            });
            self.status = i18n::tr(
                "Edition",
                match (song, accepted) {
                    (true, true) => "SongTextStaged",
                    (true, false) => "SongTextInvalidDiscarded",
                    (false, true) => "ChartTextStaged",
                    (false, false) => "ChartTextInvalidDiscarded",
                },
            );
            return;
        }
        if !note
            && matches!(
                code,
                KeyCode::ArrowUp
                    | KeyCode::ArrowDown
                    | KeyCode::Tab
                    | KeyCode::PageUp
                    | KeyCode::PageDown
            )
        {
            if !self.sync_text() {
                return;
            }
            let total = count + 1;
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            let delta = if backward { total - 1 } else { 1 };
            let mut next = match code {
                KeyCode::PageUp => index.saturating_sub(8) / 8 * 8,
                KeyCode::PageDown => (index / 8 * 8 + 8).min(count),
                _ => (index + delta) % total,
            };
            while next < count
                && !self
                    .field_at(next)
                    .is_some_and(|target| self.field_supported(target))
            {
                next = (next + delta) % total;
            }
            self.panel = Some(if song {
                Panel::SongText(next)
            } else {
                Panel::ChartText(next)
            });
            self.focus_text();
            return;
        }
        let Some(buffer) = self.text_buffer.as_mut() else {
            return;
        };
        match code {
            KeyCode::ArrowLeft => buffer.move_cursor(
                if self.control {
                    Movement::WordLeft
                } else {
                    Movement::Left
                },
                self.shift,
            ),
            KeyCode::ArrowRight => buffer.move_cursor(
                if self.control {
                    Movement::WordRight
                } else {
                    Movement::Right
                },
                self.shift,
            ),
            KeyCode::Home => buffer.move_cursor(Movement::Start, self.shift),
            KeyCode::End => buffer.move_cursor(Movement::End, self.shift),
            KeyCode::KeyA if self.control => buffer.select_all(),
            KeyCode::Backspace => {
                buffer.backspace(self.control);
            }
            KeyCode::Delete => {
                buffer.delete(self.control);
            }
            KeyCode::KeyZ if self.control => {
                if self.shift {
                    buffer.redo();
                } else {
                    buffer.undo();
                }
            }
            KeyCode::KeyY if self.control => {
                buffer.redo();
            }
            _ => return,
        }
        self.sync_text();
    }
    pub(super) fn text_field_label(&self, index: usize) -> std::sync::Arc<str> {
        let Some(target) = self.field_at(index) else {
            return std::sync::Arc::from("");
        };
        if !self.field_supported(target) {
            let unsupported = i18n::tr("Edition", "ChartTextUnsupportedSm");
            let existing = self.field_value(target).unwrap_or_default();
            let value = if existing.is_empty() {
                unsupported.to_string()
            } else {
                let display = Buffer::new(existing.as_ref(), false).display("", 28);
                format!(
                    "{} [{}]",
                    display.strip_suffix('|').unwrap_or(&display),
                    unsupported
                )
            };
            return i18n::tr_fmt("Edition", target.key(), &[("value", &value)]);
        }
        let mut value = if self.text_target() == Some(target) {
            self.text_buffer
                .as_ref()
                .map(|b| b.display(&self.label_preedit, 48))
                .unwrap_or_default()
        } else {
            let b = Buffer::new(self.field_value(target).unwrap_or_default().as_ref(), false);
            let p = b.display("", 48);
            p.strip_suffix('|').unwrap_or(&p).to_owned()
        };
        if target.is_music()
            && self.field_value(target).as_deref() == Some("")
            && (value.is_empty() || value == "|")
        {
            let hint = i18n::tr(
                "Edition",
                if target.is_preview() {
                    "SongPreviewMusicEmpty"
                } else if target.is_song() {
                    "SongMusicEmpty"
                } else {
                    "ChartMusicInherited"
                },
            );
            value = if value.is_empty() {
                hint.to_string()
            } else {
                format!("{value} {hint}")
            };
        }
        i18n::tr_fmt("Edition", target.key(), &[("value", &value)])
    }
}

#[cfg(test)]
mod chart_authoring_tests {
    use super::*;
    #[test]
    fn keyboard_text_fields_are_all_editable_in_sm_and_ssc_en_fr() {
        for fr in [false, true] {
            if fr {
                i18n::init(deadsync_assets::language::load_for_tests("fr"));
            } else {
                i18n::init_for_tests();
            }
            for sm in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join(if sm { "song.sm" } else { "song.ssc" });
                let notes = if sm {
                    "#NOTES:dance-single::Easy:1::1000;"
                } else {
                    "#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:1;#NOTES:1000;"
                };
                std::fs::write(
                    &path,
                    format!("#VERSION:0.83;#TITLE:source;#BPMS:0=120;{notes}"),
                )
                .unwrap();
                let mut state = State::open(&path, 0).unwrap();
                state.open_chart_properties();
                state.open_chart_text();
                assert!(matches!(state.panel, Some(Panel::ChartText(0))));
                for (index, field) in ChartField::ALL.into_iter().enumerate() {
                    assert!(state.field_supported(Target::Chart(field)));
                    assert!(matches!(state.panel,Some(Panel::ChartText(i)) if i==index));
                    state.text_buffer.as_mut().unwrap().select_all();
                    assert!(state.edit_text_input("  authored  "));
                    state.text_field_key(index, KeyCode::ArrowDown);
                }
                state.text_field_key(ChartField::ALL.len(), KeyCode::Escape);
                state.apply_chart_properties();
                assert!(state.panel.is_none(), "{}", state.status);
                let text = deadsync_editor::chart_text::Text::read(
                    state.workspace.file().session.document(),
                    0,
                )
                .unwrap();
                for field in ChartField::ALL {
                    assert_eq!(text.get(field), "  authored  ");
                }
                state
                    .workspace
                    .execute(deadsync_editor::workspace::Command::Undo)
                    .unwrap();
                state
                    .workspace
                    .execute(deadsync_editor::workspace::Command::Redo)
                    .unwrap();
                let text = deadsync_editor::chart_text::Text::read(
                    state.workspace.file().session.document(),
                    0,
                )
                .unwrap();
                for field in ChartField::ALL {
                    assert_eq!(text.get(field), "  authored  ");
                }
            }
        }
    }
    fn audio_state(timing: &str) -> (tempfile::TempDir, State) {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("song.sm");
        for name in ["music.wav", "alternate.wav"] {
            std::fs::write(root.path().join(name), b"command-only fixture").unwrap();
        }
        std::fs::write(&path,format!("#TITLE:source;#MUSIC:music.wav;#BPMS:0=120;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};{timing}#NOTES:dance-single::Easy:1::0000;")).unwrap();
        let mut state = State::open(&path, 0).unwrap();
        state.take_audio_commands();
        state.update_audio(deadsync_gameplay::GameplayAudioSnapshot::default());
        state.take_audio_commands();
        (root, state)
    }
    #[test]
    fn chart_metadata_preserves_exact_warp_focus_and_paused_stop_resume() {
        i18n::init_for_tests();
        let (_root, mut state) = audio_state("#WARPS:1=2;");
        state.navigate_exact_position(60.5, true);
        let before = state.focus_position.as_ref().unwrap().clone();
        state.take_audio_commands();
        state.open_chart_properties();
        state
            .chart_properties
            .as_mut()
            .unwrap()
            .set_text(ChartField::Name, "name")
            .unwrap();
        state.apply_chart_properties();
        let focus = state.focus_position.as_ref().unwrap();
        assert!(focus.is_current(&state.workspace));
        assert_eq!(focus.row().to_bits(), before.row().to_bits());
        assert_eq!(focus.music_time().to_bits(), before.music_time().to_bits());
        assert!(state.take_audio_commands().is_empty());
        let (_root, mut state) = audio_state("#STOPS:1=1;");
        state.toggle_audio();
        let mut audio = deadsync_gameplay::GameplayAudioSnapshot::default();
        audio.stream_clock.valid_at = std::time::Instant::now();
        audio.stream_clock.stream_seconds = 1.0;
        audio.stream_clock.has_music_mapping = true;
        audio.stream_clock.music_nanos = 750_000_000;
        audio.stream_clock.music_seconds_per_second = 1.0;
        state.update_audio(audio);
        state.toggle_audio();
        assert_eq!(state.playback_position(), Some((0.75, 48.0)));
        state.take_audio_commands();
        state.open_chart_properties();
        state
            .chart_properties
            .as_mut()
            .unwrap()
            .set_text(ChartField::Description, "name")
            .unwrap();
        state.apply_chart_properties();
        assert_eq!(state.playback_position(), Some((0.75, 48.0)));
        assert!(state.take_audio_commands().is_empty());
        state.toggle_audio();
        assert!(state.is_playing());
        assert!(state.take_audio_commands().iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorResumeMusic{seconds,..} if *seconds==0.75)));
    }
    #[test]
    fn effective_music_and_offset_changes_keep_main_and_retarget_current_row() {
        i18n::init_for_tests();
        let (root, mut state) = audio_state("");
        let mut ack = deadsync_gameplay::GameplayAudioSnapshot::default();
        ack.editor.owner = state.editor_asset_owner();
        ack.editor.output.available = true;
        ack.editor.output.leased = true;
        ack.editor.output.period_control = deadsync_gameplay::EditorBufferControl::Adjustable;
        ack.editor.output.requested_period = Some(512);
        ack.editor.output.negotiated_period = Some(512);
        state.update_audio(ack);
        state.take_audio_commands();
        state.toggle_audio();
        assert!(state.chart_transport_is_playing());
        state.take_audio_commands();
        let before = state.capture_song_property_presentation().unwrap();
        state.open_chart_properties();
        state
            .chart_properties
            .as_mut()
            .unwrap()
            .set_text(ChartField::Music, "alternate.wav")
            .unwrap();
        state.apply_chart_properties();
        assert!(state.chart_transport_is_playing());
        assert_eq!(
            state.transport_music_time().unwrap().to_bits(),
            before.music_time().to_bits()
        );
        let commands = state.take_audio_commands();
        assert!(commands.iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorSwapMusicAsset{path,..}if *path==root.path().join("alternate.wav").canonicalize().unwrap())));
        assert!(!commands.iter().any(|command| matches!(
            command,
            deadsync_gameplay::GameplayAudioCommand::EditorPauseMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorStopMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorStartMusic { .. }
        )));
        state.navigate_exact_position(60.5, true);
        state.take_audio_commands();
        state.open_chart_properties();
        let draft = state.chart_properties.as_mut().unwrap();
        draft.set_use_chart_offset(true);
        draft.set_music_offset(0.25).unwrap();
        state.apply_chart_properties();
        assert!(state.chart_transport_is_playing());
        let focus = state.focus_position.as_ref().unwrap();
        assert!(focus.is_current(&state.workspace));
        assert_eq!(focus.row(), 60.5);
        let seconds = focus.music_time() + state.audio_preferences.effective().offset;
        let commands = state.take_audio_commands();
        assert!(commands.iter().any(|command|matches!(command,deadsync_gameplay::GameplayAudioCommand::EditorSeekMusic{seconds:actual,..}if actual.to_bits()==seconds.to_bits())));
        assert!(!commands.iter().any(|command| matches!(
            command,
            deadsync_gameplay::GameplayAudioCommand::EditorPauseMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorStopMusic { .. }
                | deadsync_gameplay::GameplayAudioCommand::EditorStartMusic { .. }
        )));
    }
}
