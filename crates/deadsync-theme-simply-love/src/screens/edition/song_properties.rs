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
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::SongProperties(
                (index + if code == KeyCode::ArrowUp { 3 } else { 1 }) % 4,
            ));
        } else if index == 0
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
                let value = match code {
                    KeyCode::Home => deadsync_editor::song_properties::DEFAULT_SYNC_OFFSET,
                    KeyCode::Digit0 | KeyCode::Numpad0 => 0.0,
                    KeyCode::ArrowLeft => draft.sync_offset() - delta,
                    _ => draft.sync_offset() + delta,
                };
                if let Err(error) = draft.set_sync_offset(value) {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
                }
            }
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 3 {
                self.open_song_text();
                return;
            }
            if index == 2 {
                self.song_properties = None;
                self.panel = None;
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
    }
}
