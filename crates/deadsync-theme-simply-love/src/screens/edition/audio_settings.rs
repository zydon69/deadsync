//! Compact keyboard inspector for all source Audio fields.
use super::*;
use deadsync_editor::audio_preferences::{Audio, Field, FieldValue};
pub(super) const AUDIO_KEYS: [&str; 5] = [
    "AudioField",
    "AudioValue",
    "AudioSettingsDefaults",
    "AudioSettingsApply",
    "AudioSettingsCancel",
];
impl State {
    pub(super) fn sync_audio_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_audio_preferences_update() {
            let previous = self.audio_preferences;
            self.audio_preferences = update.audio;
            if let Some(file) = update.file {self.publish_application_preferences_owner(file);}
            self.sync_editor_audio_settings(previous);
        }
    }
    pub(super) fn commit_audio_preferences(
        &mut self,
        audio: Audio,
    ) -> Result<bool, deadsync_editor::Error> {
        let file = self.capture_application_preferences_file()?;
        let changed = self.workspace.set_audio_preferences(
            self.audio_preferences,
            audio,
            Some(&file),
        )?;
        self.sync_audio_preferences_history();
        Ok(changed)
    }
    pub(super) fn step_music_rate(&mut self, increase: bool) {
        let mut audio = self.audio_preferences;
        audio.step_music_rate(increase);
        match self.commit_audio_preferences(audio) {
            Ok(_) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "AudioMusicRate",
                    &[("rate", &format!("{:.3}", audio.music_rate()))],
                )
            }
            Err(error) => self.status = i18n::tr_fmt("Edition", "AudioSaveFailedReason", &[("message", &i18n::editor_error(&error))]),
        }
    }
    pub(super) fn toggle_source_tick(&mut self, beat: bool) {
        let field = if beat {
            Field::UseBeatTick
        } else {
            Field::UseAssistTick
        };
        let mut audio = self.audio_preferences;
        let FieldValue::Bool(enabled) = audio.value(field) else {
            unreachable!()
        };
        if audio.set_field(field, FieldValue::Bool(!enabled)).is_err() {
            return;
        }
        let result=(|| {
            let file = self.capture_application_preferences_file()?;
            let changed=self.workspace.set_audio_preferences_untracked(self.audio_preferences,audio,Some(&file))?;
            self.sync_audio_preferences_history();
            Ok::<_,deadsync_editor::Error>(changed)
        })();
        match result {
            Ok(_) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "AudioTickChanged",
                    &[
                        (
                            "kind",
                            &i18n::tr(
                                "Edition",
                                if beat {
                                    "AudioNameBeat"
                                } else {
                                    "AudioNameAssist"
                                },
                            ),
                        ),
                        (
                            "value",
                            &i18n::tr(
                                "Edition",
                                if !enabled {
                                    "WaveformYes"
                                } else {
                                    "WaveformNo"
                                },
                            ),
                        ),
                    ],
                )
            }
            Err(error) => self.status = i18n::tr_fmt("Edition", "AudioSaveFailedReason", &[("message", &i18n::editor_error(&error))]),
        }
    }
    pub(super) fn open_audio_settings(&mut self) {
        self.audio_numeric_text=None;
        self.audio_preferences_draft = Some(self.audio_preferences);
        self.panel = Some(Panel::AudioSettings(0));
        self.status = i18n::tr("Edition", "AudioSettingsHelp");
    }
    pub(super) fn audio_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if self.audio_numeric_text.is_some() {
            if code==KeyCode::Escape { self.finish_audio_numeric(); return; }
            if enter {
                if !self.sync_text() {return;}
                let field=Field::ALL[self.audio_field];
                let text=self.audio_numeric_text.as_deref().unwrap_or_default().trim();
                let value=match self.audio_preferences_draft.map(|audio|audio.value(field)) {
                    Some(FieldValue::Int(_))=>text.parse::<i32>().ok().map(FieldValue::Int),
                    Some(FieldValue::Float(_))=>text.parse::<f64>().ok().filter(|v|v.is_finite()).map(FieldValue::Float),
                    _=>None,
                };
                let accepted=value.zip(self.audio_preferences_draft.as_mut()).is_some_and(|(value,draft)|draft.set_field(field,value).is_ok());
                if accepted {self.finish_audio_numeric();} else {self.status=i18n::tr("Edition","AudioValueInvalid");}
                return;
            }
            if matches!(code,KeyCode::ArrowUp|KeyCode::ArrowDown|KeyCode::Tab|KeyCode::PageUp|KeyCode::PageDown){return;}
            self.text_field_key(1,code);return;
        }
        if index==1 && enter {
            let field=Field::ALL[self.audio_field];
            let value=self.audio_preferences_draft.map(|audio|audio.value(field));
            let text=match value {Some(FieldValue::Float(v))=>Some(v.to_string()),Some(FieldValue::Int(v))=>Some(v.to_string()),_=>None};
            if let Some(text)=text {self.audio_numeric_text=Some(text);self.focus_text();return;}
        }
        if code == KeyCode::Escape || (index == 4 && enter) {
            self.audio_preferences_draft = None;
            self.panel = Some(Panel::SongSample(0));
            self.status = i18n::tr("Edition", "AudioSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::AudioSettings((index + if back { 4 } else { 1 }) % 5));
            return;
        }
        if index == 2 && enter {
            if let Some(draft) = self.audio_preferences_draft.as_mut() {
                draft.restore_source_defaults();
            }
            return;
        }
        if index == 3 && enter {
            let Some(audio) = self.audio_preferences_draft else {
                return;
            };
            self.begin_audio_preferences_apply(audio);
            return;
        }
        let left = code == KeyCode::ArrowLeft;
        let right = code == KeyCode::ArrowRight;
        if index == 0 && (left || right) {
            self.audio_field = (self.audio_field + if left { 15 } else { 1 }) % 16;
            return;
        }
        if index != 1
            || !(left
                || right
                || enter
                || matches!(code, KeyCode::Home | KeyCode::Digit0 | KeyCode::Numpad0))
        {
            return;
        }
        let field = Field::ALL[self.audio_field];
        let Some(draft) = self.audio_preferences_draft.as_mut() else {
            return;
        };
        let value = if code == KeyCode::Home {
            Audio::default().value(field)
        } else {
            match draft.value(field) {
                FieldValue::Bool(value) => FieldValue::Bool(!value),
                FieldValue::Int(value) => {
                    if enter {
                        return;
                    }
                    let (lo, hi, step) = if field == Field::DspBufferSize {
                        (32, 8192, if self.shift { 32 } else { 1 })
                    } else {
                        (2, 32, 1)
                    };
                    FieldValue::Int(if matches!(code, KeyCode::Digit0 | KeyCode::Numpad0) {
                        lo
                    } else {
                        value
                            .saturating_add(if left { -step } else { step })
                            .clamp(lo, hi)
                    })
                }
                FieldValue::Float(value) => {
                    if enter {
                        return;
                    }
                    let (lo, hi, step) = match field {
                        Field::MusicRate => (0.25, 4., if self.shift { 0.0001 } else { 0.001 }),
                        Field::AudioOffset => {
                            (f64::MIN, f64::MAX, if self.shift { 0.0001 } else { 0.001 })
                        }
                        Field::MainVolume
                        | Field::MusicVolume
                        | Field::AssistTickVolume
                        | Field::BeatTickVolume => (0., 1., if self.shift { 0.001 } else { 0.01 }),
                        _ => (0., f64::MAX, if self.shift { 0.0001 } else { 0.001 }),
                    };
                    let zero = if field == Field::MusicRate { 1. } else { 0. };
                    FieldValue::Float(if matches!(code, KeyCode::Digit0 | KeyCode::Numpad0) {
                        zero
                    } else {
                        (value + if left { -step } else { step }).clamp(lo, hi)
                    })
                }
            }
        };
        if draft.set_field(field, value).is_err() {
            self.status = i18n::tr("Edition", "AudioValueInvalid");
        }
    }
    fn finish_audio_numeric(&mut self) {
        self.audio_numeric_text=None;self.text_buffer=None;self.label_preedit.clear();
        self.text_edit_serial=self.text_edit_serial.wrapping_add(1);
    }
    pub(super) fn audio_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let audio = self
            .audio_preferences_draft
            .unwrap_or(self.audio_preferences);
        let field = Field::ALL[self.audio_field];
        let name = i18n::tr("Edition", audio_field_name(field));
        let value = match audio.value(field) {
            FieldValue::Bool(value) => {
                i18n::tr("Edition", if value { "WaveformYes" } else { "WaveformNo" }).to_string()
            }
            FieldValue::Int(value) => value.to_string(),
            FieldValue::Float(value) => match field {
                Field::AudioOffset | Field::AssistTickAttackTime | Field::BeatTickAttackTime=>format!("{value:.6}"),
                Field::MusicRate | Field::PreviewFadeInTime | Field::PreviewFadeOutTime=>format!("{value:.3}"),
                _=>format!("{value:.4}"),
            },
        };
        let value=if self.audio_numeric_text.is_some() {
            self.text_buffer.as_ref().map(|buffer|buffer.display(&self.label_preedit,48)).unwrap_or(value)
        }else{value};
        let x = screen_width() * 0.5;
        let top = (screen_height() * 0.5 - 135.).max(12.);
        actors
            .push(act!(quad:xy(x,top+140.):zoomto(600.,350.):diffuse(0.04,0.06,0.1,0.98):z(8050)));
        let heading = i18n::tr_fmt(
            "Edition",
            "AudioInspectorHeading",
            &[
                ("index", &(self.audio_field + 1).to_string()),
                ("count", "16"),
            ],
        );
        actors.push(
            act!(text:font("miso"):settext(heading):xy(x,top):zoom(0.65):maxwidth(570.):z(8051)),
        );
        for (row, key) in AUDIO_KEYS.iter().enumerate() {
            let ui_row_start=actors.len();
            let label = match row {
                0 => i18n::tr_fmt("Edition", key, &[("value", &name)]),
                1 => i18n::tr_fmt("Edition", key, &[("value", &value)]),
                _ => i18n::tr("Edition", key),
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+38.+row as f32*31.):zoom(0.66):maxwidth(570.):diffuse(1.,if row==index {0.8}else{1.},if row==index {0.3}else{1.},1.):z(8051)));

            self.ui_row(actors,ui_row_start,row==index,31.);
        }
        if field.is_dsp() {
            let state = self.audio_output_snapshot();
            let control = if field == Field::DspBufferSize {
                state.period_control
            } else {
                state.count_control
            };
            let (requested, actual) = if field == Field::DspBufferSize {
                (state.requested_period, state.negotiated_period)
            } else {
                (state.requested_count, state.negotiated_count)
            };
            let requested = requested.map_or_else(
                || i18n::tr("Edition", "AudioBackendDefault").to_string(),
                |value| value.to_string(),
            );
            let actual = actual.map_or_else(
                || i18n::tr("Edition", "AudioBackendUnknown").to_string(),
                |value| value.to_string(),
            );
            let label = i18n::tr_fmt(
                "Edition",
                "AudioOutputActual",
                &[("requested", &requested), ("actual", &actual)],
            );
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+263.):zoom(0.48):maxwidth(570.):z(8051)));
            if control != deadsync_gameplay::EditorBufferControl::Adjustable {
                let note = i18n::tr(
                    "Edition",
                    if control == deadsync_gameplay::EditorBufferControl::ServerControlled {
                        "AudioOutputServer"
                    } else {
                        "AudioDspUnsupportedNote"
                    },
                );
                actors.push(act!(text:font("miso"):settext(note):xy(x,top+282.):zoom(0.44):maxwidth(570.):z(8051)));
            }
        }
        if self.audio_output_restart_required() {
            let note = i18n::tr("Edition", "AudioDspRestartRequired");
            actors.push(act!(text:font("miso"):settext(note):xy(x,top+303.):zoom(0.46):maxwidth(570.):z(8051)));
        }
        let help = i18n::tr("Edition", "AudioInspectorHelp");
        actors.push(act!(text:font("miso"):settext(help):xy(x,top+227.):zoom(0.48):wrapwidthpixels(1120.):maxwidth(570.):maxheight(45.):z(8051)));
    }
}
fn audio_field_name(field: Field) -> &'static str {
    match field {
        Field::MusicRate => "AudioNameRate",
        Field::AudioOffset => "AudioNameOffset",
        Field::MainVolume => "AudioNameMain",
        Field::MusicVolume => "AudioNameMusic",
        Field::AssistTickVolume => "AudioNameAssistVolume",
        Field::AssistTickAttackTime => "AudioNameAssistAttack",
        Field::UseAssistTick => "AudioNameAssist",
        Field::SkipAssistTickOnBeatTick => "AudioNameSkipAssist",
        Field::BeatTickVolume => "AudioNameBeatVolume",
        Field::BeatTickAttackTime => "AudioNameBeatAttack",
        Field::UseBeatTick => "AudioNameBeat",
        Field::SkipBeatTickOnAssistTick => "AudioNameSkipBeat",
        Field::DspBufferSize => "AudioNameDspSize",
        Field::DspNumBuffers => "AudioNameDspCount",
        Field::PreviewFadeInTime => "AudioNameFadeIn",
        Field::PreviewFadeOutTime => "AudioNameFadeOut",
    }
}
#[cfg(test)]
mod source_audio_numeric_tests {
    use super::*;
    fn state() -> (tempfile::TempDir,State) {
        i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");
        std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:1;#NOTES:1000;").unwrap();
        let state=State::open(&path,0).unwrap();(root,state)
    }
    fn enter_number(state:&mut State,field:Field,text:&str) {
        state.open_audio_settings();state.panel=Some(Panel::AudioSettings(1));state.audio_field=Field::ALL.iter().position(|f|*f==field).unwrap();
        state.audio_settings_key(1,KeyCode::Enter);assert!(state.wants_text_input());assert!(state.text_input(text));state.audio_settings_key(1,KeyCode::Enter);
    }
    #[test]
    fn arbitrary_source_numeric_precision_is_reachable_from_keyboard() {
        let (_root,mut state)=state();let bytes=state.workspace.file().session.serialize().unwrap();
        enter_number(&mut state,Field::AssistTickAttackTime,"1e-6");
        assert!(state.audio_numeric_text.is_none());
        let FieldValue::Float(value)=state.audio_preferences_draft.unwrap().value(Field::AssistTickAttackTime)else{panic!()};
        assert_eq!(value,1e-6_f32 as f64);assert_eq!(state.audio_preferences,Audio::default());
        assert_eq!(state.workspace.file().session.serialize().unwrap(),bytes);
        enter_number(&mut state,Field::AudioOffset,"-0.123456789012345");
        assert_eq!(state.audio_preferences_draft.unwrap().value(Field::AudioOffset),FieldValue::Float(-0.123456789012345));
        enter_number(&mut state,Field::DspBufferSize,"16384");
        assert_eq!(state.audio_preferences_draft.unwrap().value(Field::DspBufferSize),FieldValue::Int(8192));
    }
    #[test]
    fn raw_keyboard_numeric_entry_apply_and_escape_work_in_both_locales() {
        fn press(state: &mut State, code: KeyCode) {
            for pressed in [true, false] {
                state.raw_key(&deadlib_platform::input::RawKeyboardEvent {
                    code, pressed, repeat: false, timestamp: std::time::Instant::now(), host_nanos: 0,
                });
            }
        }
        for locale in ["en", "fr"] {
            let (_root, mut state) = state();
            i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
            state.open_audio_settings();
            state.audio_field = Field::ALL.iter().position(|field| *field == Field::AudioOffset).unwrap();
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::Enter);
            assert!(state.wants_text_input());
            assert!(state.text_input("-0.123456789012345"));
            press(&mut state, KeyCode::Enter);
            assert!(!state.wants_text_input());
            assert_eq!(state.audio_preferences_draft.unwrap().value(Field::AudioOffset), FieldValue::Float(-0.123456789012345));
            assert_eq!(state.audio_preferences, Audio::default());
            press(&mut state, KeyCode::Enter);
            assert!(state.text_input("0.9"));
            press(&mut state, KeyCode::Escape);
            assert!(matches!(state.panel, Some(Panel::AudioSettings(1))));
            assert_eq!(state.audio_preferences_draft.unwrap().value(Field::AudioOffset), FieldValue::Float(-0.123456789012345));
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::ArrowDown);
            press(&mut state, KeyCode::Enter);
            assert_eq!(state.audio_preferences.value(Field::AudioOffset), FieldValue::Float(-0.123456789012345));
            state.command(Command::Undo);
            assert_eq!(state.audio_preferences, Audio::default());
        }
    }
    #[test]
    fn invalid_number_stays_editable_and_escape_discards_only_numeric_edit() {
        let (_root,mut state)=state();
        enter_number(&mut state,Field::AudioOffset,"NaN");assert!(state.audio_numeric_text.is_some());assert_eq!(state.audio_preferences_draft,Some(Audio::default()));
        state.audio_settings_key(1,KeyCode::Escape);assert!(state.audio_numeric_text.is_none());assert!(!state.wants_text_input());assert!(matches!(state.panel,Some(Panel::AudioSettings(1))));
        enter_number(&mut state,Field::DspNumBuffers,"999999999999999999999");assert!(state.audio_numeric_text.is_some());assert_eq!(state.audio_preferences_draft,Some(Audio::default()));
    }
}
#[cfg(test)]
mod canonical_audio_memory_workflows {
 use super::*;
 #[test]
 fn all_sixteen_audio_controls_use_canonical_memory_and_real_history_consumers(){
  i18n::init_for_tests();
  for field in Field::ALL {
   let dir=tempfile::tempdir().unwrap();let mut state=State::without_song(dir.path()).unwrap();
   let file=state.capture_application_preferences_file().unwrap();let draft=state.workspace.prepare_source_preferences(&file,br#"{"VendorPreserved":{"x":1}}"#,deadsync_editor::key_binding_preferences::source_profile::Platform::Mac).unwrap();state.workspace.import_source_preferences(draft,&file,true).unwrap();state.sync_source_preferences_history().unwrap();
   let before=state.audio_preferences;let mut after=before;let value=match before.value(field){FieldValue::Bool(v)=>FieldValue::Bool(!v),FieldValue::Int(v)=>FieldValue::Int(v+1),FieldValue::Float(v)=>FieldValue::Float(if v>0.5{v*0.75}else{v+0.1})};after.set_field(field,value).unwrap();
   state.commit_audio_preferences(after).unwrap();assert_eq!(state.audio_preferences,after,"{field:?}");assert!(state.audio_preferences_file.is_none());assert!(state.audio_preferences_path.is_none());assert_eq!(state.application_preferences_owner().unwrap().audio(),after);assert!(!state.application_preferences_owner().unwrap().has_changes());
   state.command(Command::Undo);assert_eq!(state.audio_preferences,before,"{field:?}");
   state.command(Command::Redo);assert_eq!(state.audio_preferences,after,"{field:?}");assert_eq!(state.application_preferences_owner().unwrap().audio(),after);assert!(!state.application_preferences_owner().unwrap().has_changes());
  }
 }
}
