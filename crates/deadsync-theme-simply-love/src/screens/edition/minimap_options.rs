//! Advanced options stay in an optional, keyboard-accessible scrolling panel.
use super::minimap_options_draft::{Draft, Field};
use super::*;
impl State {
    /// One pending source owner across the keyboard presentation's two pages.
    pub(super) fn pending_minimap_visual(&self) -> deadsync_editor::visual_preferences::MiniMap {
        let mut pending = self
            .minimap
            .visual_draft
            .unwrap_or_else(|| self.visual_minimap());
        if matches!(self.panel, Some(Panel::MinimapSettings(_))) {
            if let Some(viewport) = self.minimap.draft {
                pending.viewport = viewport;
            }
        } else if matches!(self.panel, Some(Panel::MinimapOptions(_))) {
            if let Some(options) = self.minimap.options_draft {
                pending.position = options.position;
                pending.content = options.content;
            }
        }
        pending
    }

    pub(super) fn open_minimap_options(&mut self) {
        self.minimap_release_input();
        let pending = if matches!(self.panel, Some(Panel::MinimapSettings(_))) {
            self.pending_minimap_visual()
        } else {
            self.visual_minimap()
        };
        self.minimap.visual_draft = Some(pending);
        self.minimap.draft = None;
        self.minimap.options_draft = Some(Draft {
            position: pending.position,
            content: pending.content,
        });
        self.panel = Some(Panel::MinimapOptions(0));
    }
    pub(super) fn minimap_options_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 15 && enter) {
            self.minimap.draft = None;
            self.minimap.visual_draft = None;
            self.minimap.options_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
            return;
        }
        if index == 16 && enter {
            self.open_minimap_settings();
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let backward = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::MinimapOptions(
                (index + if backward { 16 } else { 1 }) % 17,
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
            let restored = self.pending_minimap_visual().restored();
            self.minimap.visual_draft = Some(restored);
            self.minimap.options_draft = Some(Draft {
                position: restored.position,
                content: restored.content,
            });
            return;
        }
        if index != 14 {
            return;
        }
        let Some(draft) = self.minimap.options_draft else {
            return;
        };
        let before = self.visual_minimap();
        let mut after = self.minimap.visual_draft.unwrap_or(before);
        after.position = draft.position;
        after.content = draft.content;
        let result = self.apply_visual_preferences(
            deadsync_editor::visual_preferences::Snapshot::MiniMap(before),
            deadsync_editor::visual_preferences::Snapshot::MiniMap(after),
        );
        match result {
            Ok(()) => {
                self.minimap.draft = None;
                self.minimap.visual_draft = None;
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
            "MinimapBasic",
        ];
        let first = index.saturating_sub(3).min(keys.len().saturating_sub(7));
        let x = screen_width() * 0.5;
        let y = (screen_height() * 0.5 - 110.0).max(104.0);
        actors.push(
            act!(quad: xy(x,y+90.0): setsize(560.0,244.0): diffuse(0.04,0.06,0.1,0.98): z(8050)),
        );
        for row in first..(first + 7).min(keys.len()) {
            let ui_row_start = actors.len();
            let label = if row < 13 {
                i18n::tr_fmt("Edition", keys[row], &[("value", &values[row])])
            } else {
                i18n::tr("Edition", keys[row])
            };
            actors.push(act!(text: font("miso"): settext(label): xy(x,y+(row-first) as f32*30.0): zoom(0.65): maxwidth(520.0): diffuse(1.0,if row==index {0.8}else{1.0},if row==index {0.3}else{1.0},1.0): z(8051)));

            self.ui_row(actors, ui_row_start, row == index, 30.);
        }
    }
}

#[cfg(test)]
mod shared_draft_tests;
