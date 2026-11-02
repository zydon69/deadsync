use super::*;
impl State {
    pub(super) fn song_selectable_label(&self,key:&str)->Arc<str>{
        let code=self.song_properties.as_ref().map(|draft|draft.selectable().source_name()).unwrap_or_else(||"YES".into());
        let key_value=match code.as_str(){"YES"=>Some("SongSelectableYes"),"NO"=>Some("SongSelectableNo"),"ROULETTE"=>Some("SongSelectableRoulette"),"ES"=>Some("SongSelectableEs"),"OMES"=>Some("SongSelectableOmes"),_=>None};
        let value=key_value.map(|name|i18n::tr("Edition",name)).unwrap_or_else(||code.into());
        i18n::tr_fmt("Edition",key,&[("value",&value)])
    }

    /// Capture actual fractional transport/focus time before mutating metadata;
    /// use a fresh complete song draft, independently of any open inspector.
    pub(super) fn move_source_song_marker(&mut self, end_hint: bool) {
        if self.file_io_active() {
            self.status = i18n::tr("Edition", "FileBusy");
            return;
        }
        let result = (|| -> Result<bool, deadsync_editor::Error> {
            if !self.has_song() || !self.workspace.has_chart() {
                return Err(deadsync_editor::Error::InvalidEdit(
                    "No active chart".into(),
                ));
            }
            let presentation = self.capture_song_property_presentation()?;
            let seconds = presentation.music_time();
            if !seconds.is_finite() {
                return Err(deadsync_editor::Error::InvalidEdit(
                    "Nonfinite song position".into(),
                ));
            }
            let mut draft = self.workspace.song_properties()?;
            draft.move_marker_to_time(
                if end_hint {
                    deadsync_editor::song_properties::Marker::EndHint
                } else {
                    deadsync_editor::song_properties::Marker::Preview
                },
                seconds,
            )?;
            if !draft.has_changes() {
                return Ok(false);
            }
            let application=self.workspace.apply_song_properties_with_presentation_receipt(&draft)?;
            let changed=application.changed();
            if changed {
                self.song_properties=None;
                let receipt=application.presentation_receipt().ok_or_else(||deadsync_editor::Error::InvalidEdit("Marker edit unexpectedly changed the presentation clock".into()))?;
                self.rebind_song_property_presentation(presentation,receipt)?;
            }
            Ok(changed)
        })();
        self.status = match result {
            Ok(true) => i18n::tr(
                "Edition",
                if end_hint {
                    "SongEndHintMoved"
                } else {
                    "SongPreviewMoved"
                },
            ),
            Ok(false) => i18n::tr("Edition", "SongMarkerUnchanged"),
            Err(error) => i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))]),
        };
        self.refresh();
    }
    pub(super) fn open_song_properties(&mut self) {
        self.finish_preference_numeric();
        if !self.has_song(){self.status=i18n::tr("Edition","FileNoSongHelp");return;}
        match self.workspace.song_properties() {
            Ok(draft) => {
                self.song_properties = Some(draft);
                self.panel = Some(Panel::SongProperties(0));
                // The first frame already substitutes draft song widgets in
                // source_viewport_plan; prepare that cache before presentation.
                self.refresh_sample_region();
                self.status = i18n::tr("Edition", "SongPropertiesHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))])
            }
        }
    }
    pub(super) fn song_properties_key(&mut self, index: usize, code: KeyCode) {
        if index==7&&matches!(code,KeyCode::ArrowLeft|KeyCode::ArrowRight|KeyCode::Enter|KeyCode::NumpadEnter|KeyCode::Home){
            if let Some(draft)=self.song_properties.as_mut(){let value=if code==KeyCode::Home{Default::default()}else{draft.selectable().step(code!=KeyCode::ArrowLeft)};draft.set_selectable(value);}
            return;
        }
        if index==6&&matches!(code,KeyCode::ArrowLeft|KeyCode::ArrowRight){self.queue_song_timing_order(super::song_timing_chart::Purpose::Cycle(code!=KeyCode::ArrowLeft));return;}
        if index==6&&matches!(code,KeyCode::Enter|KeyCode::NumpadEnter){self.open_song_timing_event_apply();return;}
        if code == KeyCode::F4 {
            self.find_all_song_media(!self.shift);
            return;
        }
        if code == KeyCode::F6 {
            self.panel = Some(Panel::SongSample(0));
            self.status = i18n::tr("Edition", "SongSampleHelp");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::SongProperties(
                (index + if code == KeyCode::ArrowUp { 7 } else { 1 }) % 8,
            ));
        } else if matches!(index, 0 | 4)
            && matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Digit0
                    | KeyCode::Numpad0
            )
        {
            if let Some(draft) = self.song_properties.as_mut() {
                let delta = if self.shift { 0.0001 } else { 0.001 };
                let current = if index == 4 {
                    draft.music_offset()
                } else {
                    draft.sync_offset()
                };
                let value = match code {
                    KeyCode::Home if index == 0 => {
                        deadsync_editor::song_properties::DEFAULT_SYNC_OFFSET
                    }
                    KeyCode::Home => 0.0,
                    KeyCode::Digit0 | KeyCode::Numpad0 => 0.0,
                    KeyCode::ArrowLeft => current - delta,
                    _ => current + delta,
                };
                let result = if index == 4 {
                    draft.set_music_offset(value)
                } else {
                    draft.set_sync_offset(value.max(0.0))
                };
                if let Err(error) = result {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))]);
                }
            }
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 5 {
                if let Some(draft) = self.song_properties.as_mut() {
                    if !draft.can_apply_itg_offset() { return; }
                    self.status = match draft.apply_itg_offset() {
                        Ok(()) => i18n::tr("Edition", "SongItgStaged"),
                        Err(error) => {
                            i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))])
                        }
                    };
                }
                self.refresh_sample_region();
                return;
            }
            if index == 3 {
                self.open_song_text();
                return;
            }
            if index == 2 {
                self.song_properties = None;
                self.panel = None;
                self.refresh_sample_region();
                return;
            }
            if index != 1 {
                return;
            }
            if self.song_timing_request.is_some(){self.status=i18n::tr("Edition","SongTimingLoading");return;}
            self.apply_presented_song_properties();
            return;
        }
        self.refresh_sample_region();
    }
    pub(super) fn song_sample_label(&self, index: usize, key: &str) -> Arc<str> {
        let seconds = self
            .song_properties
            .as_ref()
            .map_or(0.0, |draft| match index {
                0 => draft.sample_start(),
                1 => draft.sample_length(),
                _ => draft.last_second_hint(),
            });
        i18n::tr_fmt("Edition", key, &[("seconds", &format!("{seconds:.6}"))])
    }
    pub(super) fn song_sample_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::F4 {
            self.toggle_sample_audio();
            return;
        }
        if code == KeyCode::F5 {
            self.open_audio_settings();
            return;
        }

        if index < 3 && matches!(code, KeyCode::F2 | KeyCode::F3) {
            let exact_row=self.playback_position().map_or(self.workspace.row() as f64,|(_,row)|row);
            let result = (|| -> Result<(), deadsync_editor::Error> {
                let Some(draft) = self.song_properties.as_mut() else {
                    return Ok(());
                };
                let clock = draft.clock(&self.workspace.file().session, self.workspace.chart())?;
                if code == KeyCode::F2 {
                    let seconds = clock.fractional_music_time(exact_row)?;
                    if index == 0 {
                        draft.set_sample_start_from_time(seconds)?;
                    } else if index == 1 {
                        draft.set_sample_end_from_time(seconds)?;
                    } else {
                        draft.set_last_second_hint(seconds.max(0.0))?;
                    }
                } else {
                    let seconds = if index == 0 {
                        draft.sample_start()
                    } else if index == 1 {
                        draft.sample_end()?
                    } else {
                        draft.last_second_hint()
                    };
                    let row = clock.position(seconds)?;
                    self.wheel.cancel();
                    self.navigate_wheel_position(false,row,true)?;
                }
                Ok(())
            })();
            self.status = match result {
                Ok(()) => i18n::tr(
                    "Edition",
                    if code == KeyCode::F2 {
                        "SongSampleStaged"
                    } else {
                        "SongSampleJumped"
                    },
                ),
                Err(error) => i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))]),
            };
            self.refresh_sample_region();
            return;
        }
        if code == KeyCode::Escape
            || (index == 3 && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter))
        {
            self.panel = Some(Panel::SongProperties(0));
            self.status = i18n::tr("Edition", "SongSampleStaged");
        } else if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backwards = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            let next = (index + if backwards { 3 } else { 1 }) % 4;
            self.panel = Some(Panel::SongSample(next));
            self.status = i18n::tr(
                "Edition",
                if next == 2 {
                    "SongLastSecondHintHelp"
                } else {
                    "SongSampleHelp"
                },
            );
        } else if index < 3
            && matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Digit0
                    | KeyCode::Numpad0
            )
        {
            if let Some(draft) = self.song_properties.as_mut() {
                let current = if index == 0 {
                    draft.sample_start()
                } else if index == 1 {
                    draft.sample_length()
                } else {
                    draft.last_second_hint()
                };
                let delta = if self.shift { 0.0001 } else { 0.001 };
                let value = match code {
                    KeyCode::Home | KeyCode::Digit0 | KeyCode::Numpad0 => 0.0,
                    KeyCode::ArrowLeft => current - delta,
                    _ => current + delta,
                };
                let result = if index == 0 {
                    draft.set_sample_start(value)
                } else if index == 1 {
                    draft.set_sample_length(value.max(0.0))
                } else {
                    draft.set_last_second_hint(value.max(0.0))
                };
                if let Err(error) = result {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &crate::i18n::editor_error(&error))]);
                }
            }
        }
        self.refresh_sample_region();
    }
}
#[cfg(test)]
mod source_selectable_tests {
    use super::*;
    #[test]
    fn selectable_keyboard_cycle_restore_cancel_apply_history_and_reopen_en_fr() {
        for language in ["en", "fr"] {
            crate::i18n::init_for_tests();
            crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let path = root.path().join("song.ssc");
            std::fs::write(&path,"#VERSION:0.83;#TITLE:Selection;#SELECTABLE:future;#BPMS:0=120;#WARPS:1=1;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000;").unwrap();
            let mut state = State::open(&path, 0).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            state.open_song_properties();
            state.song_properties_key(7, KeyCode::ArrowRight);
            assert_eq!(
                state
                    .song_properties
                    .as_ref()
                    .unwrap()
                    .selectable()
                    .source_name(),
                "NO"
            );
            state.song_properties_key(2, KeyCode::Enter);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            state.open_song_properties();
            for expected in ["NO", "ROULETTE", "ES", "OMES", "YES"] {
                state.song_properties_key(7, KeyCode::ArrowRight);
                assert_eq!(
                    state
                        .song_properties
                        .as_ref()
                        .unwrap()
                        .selectable()
                        .source_name(),
                    expected
                );
            }
            state.song_properties_key(7, KeyCode::Home);
            state.song_properties_key(1, KeyCode::Enter);
            assert!(state.song_properties.is_none());
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(None, "SELECTABLE")
                    .as_deref(),
                Some("YES")
            );
            state.command(Command::Undo);
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            state.command(Command::Redo);
            state.workspace.execute(Command::Save).unwrap();
            let reopened = State::open(&path, 0).unwrap();
            assert_eq!(
                reopened
                    .workspace
                    .song_properties()
                    .unwrap()
                    .selectable()
                    .source_name(),
                "YES"
            );
            assert!(!i18n::tr("Edition", "SongSelectable").contains("SongSelectable"));
        }
    }
}

#[cfg(test)]
mod source_song_sync_ui_bounds_tests {
 use super::*;
 #[test]
 fn song_sync_widget_clamps_zero_but_typed_owner_keeps_source_setter_values(){
  crate::i18n::init_for_tests();let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");
  std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":-0.25};#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Easy;#METER:1;#NOTES:1000;").unwrap();
  let mut state=State::open(&path,0).unwrap();state.open_song_properties();assert_eq!(state.song_properties.as_ref().unwrap().sync_offset(),-0.25);
  state.song_properties_key(0,KeyCode::ArrowLeft);assert_eq!(state.song_properties.as_ref().unwrap().sync_offset(),0.0);
  state.song_properties_key(2,KeyCode::Enter);assert_eq!(state.workspace.song_properties().unwrap().sync_offset(),-0.25);
 }
 #[test]
 fn selectable_values_are_localized_but_saved_tokens_stay_source_codes(){
  for language in ["en","fr"]{crate::i18n::init_for_tests();crate::i18n::set_locale(deadsync_assets::language::load_for_tests(language));
   let root=tempfile::tempdir().unwrap();let path=root.path().join("song.ssc");std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;").unwrap();
   let mut state=State::open(&path,0).unwrap();state.open_song_properties();
   for(code,key)in [("YES","SongSelectableYes"),("NO","SongSelectableNo"),("ROULETTE","SongSelectableRoulette"),("ES","SongSelectableEs"),("OMES","SongSelectableOmes")]{
    let value=deadsync_editor::song_properties::Selectable::read(Some(code));state.song_properties.as_mut().unwrap().set_selectable(value);
    let display=state.song_selectable_label("SongSelectable");assert!(display.contains(i18n::tr("Edition",key).as_ref()));assert!(!display.contains("SongSelectable"));assert_eq!(state.song_properties.as_ref().unwrap().selectable().source_name(),code);
    if language=="fr"&&matches!(code,"YES"|"NO"){assert!(!display.contains(code));}
   }
  }
 }
}
