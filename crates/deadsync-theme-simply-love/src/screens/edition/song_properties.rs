use super::*;
impl State {
    pub(super) fn open_song_properties(&mut self) {
        match self.workspace.song_properties() {
            Ok(draft) => {
                self.song_properties = Some(draft);
                self.panel = Some(Panel::SongProperties(0));
                self.status = i18n::tr("Edition", "SongPropertiesHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    pub(super) fn song_properties_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::F6 {
            self.panel = Some(Panel::SongSample(0));
            self.status = i18n::tr("Edition", "SongSampleHelp");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::SongProperties(
                (index + if code == KeyCode::ArrowUp { 5 } else { 1 }) % 6,
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
                    draft.set_sync_offset(value)
                };
                if let Err(error) = result {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
                }
            }
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 5 {
                if let Some(draft) = self.song_properties.as_mut() {
                    self.status = match draft.apply_itg_offset() {
                        Ok(()) => i18n::tr("Edition", "SongItgStaged"),
                        Err(error) => {
                            i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
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
            let Some(draft) = &self.song_properties else {
                return;
            };
            match self.workspace.apply_song_properties(draft) {
                Ok(_) => {
                    self.song_properties = None;
                    self.panel = None;
                    self.refresh();
                    self.status = i18n::tr("Edition", "SongPropertiesApplied");
                }
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                }
            }
        }
        self.refresh_sample_region();
    }
    pub(super) fn song_sample_key(&mut self, index: usize, code: KeyCode) {
        if index < 2 && matches!(code, KeyCode::F2 | KeyCode::F3) {
            let result = (|| -> Result<(), deadsync_editor::Error> {
                let Some(draft) = self.song_properties.as_mut() else {
                    return Ok(());
                };
                let clock = draft.clock(&self.workspace.file().session, self.workspace.chart())?;
                if code == KeyCode::F2 {
                    let seconds = clock.music_time(self.workspace.row())?;
                    if index == 0 {
                        draft.set_sample_start_from_time(seconds)?;
                    } else {
                        draft.set_sample_end_from_time(seconds)?;
                    }
                } else {
                    let seconds = if index == 0 {
                        draft.sample_start()
                    } else {
                        draft.sample_end()?
                    };
                    let row = clock
                        .position(seconds)?
                        .max(0.0)
                        .min(i32::MAX as f64)
                        .floor() as u64;
                    self.workspace.execute(Command::Seek(row))?;
                    self.refresh();
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
                Err(error) => i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]),
            };
            self.refresh_sample_region();
            return;
        }
        if code == KeyCode::Escape
            || (index == 2 && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter))
        {
            self.panel = Some(Panel::SongProperties(0));
            self.status = i18n::tr("Edition", "SongSampleStaged");
        } else if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backwards = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::SongSample(
                (index + if backwards { 2 } else { 1 }) % 3,
            ));
        } else if index < 2
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
                } else {
                    draft.sample_length()
                };
                let delta = if self.shift { 0.0001 } else { 0.001 };
                let value = match code {
                    KeyCode::Home | KeyCode::Digit0 | KeyCode::Numpad0 => 0.0,
                    KeyCode::ArrowLeft => current - delta,
                    _ => current + delta,
                };
                let result = if index == 0 {
                    draft.set_sample_start(value)
                } else {
                    draft.set_sample_length(value.max(0.0))
                };
                if let Err(error) = result {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
                }
            }
        }
        self.refresh_sample_region();
    }
}
