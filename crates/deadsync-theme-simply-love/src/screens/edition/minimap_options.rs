//! Advanced options stay in an optional, keyboard-accessible scrolling panel.
use super::minimap_options_draft::{Draft, Field};
use super::*;
impl State {
    pub(super) fn open_minimap_options(&mut self) {
        self.minimap_release_input();
        self.stop_audio();
        self.minimap.options_draft = Some(Draft {
            position: self.minimap.position,
            content: self.minimap.content.options,
        });
        self.panel = Some(Panel::MinimapOptions(0));
    }
    pub(super) fn minimap_options_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 15 && enter) {
            if let Some(file) = self.audio_preferences_file.as_mut() {
                file.set_minimap_position(self.minimap.position);
                let _ = file.set_minimap_content(self.minimap.content.options);
            }
            self.minimap.options_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::MinimapOptions(
                (index + if backward { 15 } else { 1 }) % 16,
            ));
            return;
        }
        if index < 13
            && (enter
                || matches!(
                    code,
                    KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home
                ))
        {
            if let Some(draft) = self.minimap.options_draft.as_mut() {
                draft.adjust(
                    Field::ALL[index],
                    if code == KeyCode::ArrowLeft { -1 } else { 1 },
                    code == KeyCode::Home,
                );
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 13 {
            self.minimap.options_draft = Some(Draft {
                position: Default::default(),
                content: Default::default(),
            });
            return;
        }
        if index != 14 {
            return;
        }
        let Some(draft) = self.minimap.options_draft else {
            return;
        };
        let result = (|| -> Result<(), deadsync_editor::Error> {
            if self.audio_preferences_file.is_none() {
                let path = self.audio_preferences_path.as_ref().ok_or_else(|| {
                    deadsync_editor::Error::InvalidEdit("Missing preferences path".into())
                })?;
                self.audio_preferences_file =
                    Some(deadsync_editor::audio_preferences::File::open(path)?);
            }
            let file = self.audio_preferences_file.as_mut().unwrap();
            file.set_minimap_content(draft.content)?;
            file.set_minimap_position(draft.position);
            file.save()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                self.configure_minimap_position_preferences(draft.position);
                self.configure_minimap_content_preferences(draft.content);
                self.minimap.options_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ScrollSettingsSaved");
            }
            Err(error) => {
                log::warn!("Editor minimap options: {error}");
                self.status = i18n::tr("Edition", "MinimapFailed");
            }
        }
    }
    pub(super) fn minimap_options_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let Some(draft) = self.minimap.options_draft else {
            return;
        };
        let yes = |value: bool| {
            i18n::tr("Edition", if value { "WaveformYes" } else { "WaveformNo" }).to_string()
        };
        let mount = match draft.position.mount {
            deadsync_editor::minimap_position_preferences::Mount::RightSideOfWindow => {
                "MinimapMountRight"
            }
            deadsync_editor::minimap_position_preferences::Mount::LeftSideOfWindow => {
                "MinimapMountLeft"
            }
            deadsync_editor::minimap_position_preferences::Mount::FocusedChartWithoutScaling => {
                "MinimapMountFocused"
            }
            deadsync_editor::minimap_position_preferences::Mount::FocusedChartWithScaling => {
                "MinimapMountFocusedScaled"
            }
        };
        let values = [
            yes(draft.content.visible),
            i18n::tr("Edition", mount).to_string(),
            i18n::tr(
                "Edition",
                if draft.position.select_mode
                    == deadsync_editor::minimap_position_preferences::SelectMode::MoveToCursor
                {
                    "MinimapSelectCursor"
                } else {
                    "MinimapSelectArea"
                },
            )
            .to_string(),
            draft.position.offset(draft.position.mount).to_string(),
            draft.content.width.to_string(),
            draft.content.note_width.to_string(),
            draft.content.note_spacing.to_string(),
            yes(draft.content.patterns),
            draft.content.patterns_width.to_string(),
            yes(draft.content.preview),
            draft.content.preview_width.to_string(),
            yes(draft.content.labels),
            draft.content.max_notes.to_string(),
        ];
        let keys = [
            "MinimapVisible",
            "MinimapMount",
            "MinimapSelectMode",
            "MinimapOffset",
            "MinimapWidth",
            "MinimapNoteWidth",
            "MinimapNoteSpacing",
            "MinimapPatterns",
            "MinimapPatternsWidth",
            "MinimapPreview",
            "MinimapPreviewWidth",
            "MinimapLabels",
            "MinimapMaxNotes",
            "ScrollSettingsDefaults",
            "ScrollSettingsSave",
            "ScrollSettingsCancel",
        ];
        let first = index.saturating_sub(3).min(9);
        let x = screen_width() * 0.5;
        let y = (screen_height() * 0.5 - 110.0).max(104.0);
        actors.push(
            act!(quad: xy(x,y+90.0): setsize(560.0,244.0): diffuse(0.04,0.06,0.1,0.98): z(8050)),
        );
        for row in first..(first + 7).min(keys.len()) {
            let label = if row < 13 {
                i18n::tr_fmt("Edition", keys[row], &[("value", &values[row])])
            } else {
                i18n::tr("Edition", keys[row])
            };
            actors.push(act!(text: font("miso"): settext(label): xy(x,y+(row-first) as f32*30.0): zoom(0.65): maxwidth(520.0): diffuse(1.0,if row==index {0.8}else{1.0},if row==index {0.3}else{1.0},1.0): z(8051)));
        }
    }
}
