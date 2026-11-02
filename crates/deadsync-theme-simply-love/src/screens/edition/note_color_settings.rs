//! Live color draft over the shared Arc; Save is one non-song history action.
use super::*;
use deadsync_editor::note_color_inspector::{Draft, FIELD_COUNT, LABELS};
impl State {
    pub(super) fn open_note_color_settings(&mut self) {
        self.minimap_release_input();
        self.receptor_views.release_input();
        match Draft::capture(Arc::clone(&self.note_colors)) {
            Ok(draft) => {
                self.note_color_draft = Some(draft);
                self.panel = Some(Panel::NoteColorSettings(0));
                self.status = i18n::tr("Edition", "NoteColorSettingsHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    pub(super) fn sync_note_color_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_note_color_preferences_update() {
            let result = self.configure_note_color_arc(update.preferences);
            if let Some(file) = update.file {
                self.publish_application_preferences_owner(file);
            }
            if let Err(error) = result {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                );
            }
        }
    }
    pub(super) fn note_color_preview(&mut self) {
        let Some(draft) = self.note_color_draft.as_ref() else {
            return;
        };
        if let Err(error) = self.configure_note_color_arc(draft.staged()) {
            self.status = i18n::tr_fmt(
                "Edition",
                "Error",
                &[("message", &crate::i18n::editor_error(&error))],
            );
        }
    }
    pub(super) fn note_color_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 6 && enter) {
            let Some(draft) = self.note_color_draft.as_ref() else {
                self.panel = None;
                return;
            };
            match self.configure_note_color_arc(draft.baseline()) {
                Ok(()) => {
                    self.note_color_draft = None;
                    self.panel = None;
                    self.status = i18n::tr("Edition", "ScrollSettingsDiscarded");
                }
                Err(error) => {
                    self.status = i18n::tr_fmt(
                        "Edition",
                        "Error",
                        &[("message", &crate::i18n::editor_error(&error))],
                    )
                }
            }
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::NoteColorSettings(
                (index + if back { 6 } else { 1 }) % 7,
            ));
            return;
        }
        if matches!(code, KeyCode::PageUp | KeyCode::PageDown)
            || (index == 0 && matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight))
        {
            if let Some(draft) = self.note_color_draft.as_mut() {
                draft.cycle_field(matches!(code, KeyCode::ArrowRight | KeyCode::PageDown));
            }
            return;
        }
        if (1..=3).contains(&index)
            && (enter
                || matches!(
                    code,
                    KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Home
                ))
        {
            let result = self.note_color_draft.as_mut().map(|draft| {
                if index > draft.components() {
                    return Ok(false);
                }
                if code == KeyCode::Home {
                    draft.reset_component(index - 1)
                } else {
                    draft.adjust(index - 1, code != KeyCode::ArrowLeft, self.shift)
                }
            });
            match result {
                Some(Ok(true)) => self.note_color_preview(),
                Some(Err(error)) => {
                    self.status = i18n::tr_fmt(
                        "Edition",
                        "Error",
                        &[("message", &crate::i18n::editor_error(&error))],
                    )
                }
                _ => {}
            }
            return;
        }
        if !enter {
            return;
        }
        if index == 4 {
            if let Some(draft) = self.note_color_draft.as_mut() {
                match draft.defaults() {
                    Ok(true) => self.note_color_preview(),
                    Err(error) => {
                        self.status = i18n::tr_fmt(
                            "Edition",
                            "Error",
                            &[("message", &crate::i18n::editor_error(&error))],
                        )
                    }
                    _ => {}
                }
            }
            return;
        }
        if index != 5 {
            return;
        }
        let Some(draft) = self.note_color_draft.as_ref() else {
            return;
        };
        let (before, after) = (draft.baseline(), draft.staged());
        let result = (|| -> Result<bool, deadsync_editor::Error> {
            let file = self.capture_application_preferences_file()?;
            let changed = self.workspace.set_note_color_preferences(
                Arc::clone(&before),
                after,
                Some(&file),
            )?;
            self.sync_note_color_preferences_history();
            if !changed {
                self.configure_note_color_arc(before)?;
            }
            Ok(changed)
        })();
        match result {
            Ok(_) => {
                self.note_color_draft = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "ScrollSettingsSaved");
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
    }
    pub(super) fn note_color_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let Some(draft) = self.note_color_draft.as_ref() else {
            return;
        };
        let x = screen_width() * 0.5;
        let top = screen_height() * 0.5 - 140.0;
        actors.push(
            act!(quad:xy(x,top+100.0):setsize(570.0,290.0):diffuse(0.04,0.06,0.1,0.98):z(8050)),
        );
        for row in 0..7 {
            let ui_row_start = actors.len();
            let label = if row == 0 {
                i18n::tr_fmt(
                    "Edition",
                    "NoteColorField",
                    &[
                        ("field", &i18n::tr("Edition", LABELS[draft.field()])),
                        ("index", &(draft.field() + 1).to_string()),
                        ("total", &FIELD_COUNT.to_string()),
                    ],
                )
            } else if row <= 3 {
                if row > draft.components() {
                    continue;
                }
                let key = if draft.components() == 3 {
                    ["NoteColorRed", "NoteColorGreen", "NoteColorBlue"][row - 1]
                } else {
                    "NoteColorValue"
                };
                let mut value = draft.value(row - 1).unwrap_or_else(|_| "?".into());
                if value == "true" || value == "false" {
                    value = i18n::tr(
                        "Edition",
                        if value == "true" {
                            "WaveformYes"
                        } else {
                            "WaveformNo"
                        },
                    )
                    .to_string();
                }
                i18n::tr_fmt("Edition", key, &[("value", &value)])
            } else {
                i18n::tr(
                    "Edition",
                    [
                        "ScrollSettingsDefaults",
                        "ScrollSettingsSave",
                        "ScrollSettingsCancel",
                    ][row - 4],
                )
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+row as f32*30.0):zoom(0.65):maxwidth(530.0):diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0):z(8051)));

            self.ui_row(actors, ui_row_start, row == index, 31.);
        }
        // The preview uses already prepared caches; no palette rebuild per frame.
        let field = draft.field();
        let selected = matches!(field, 25 | 28);
        let player = if (15..=24).contains(&field) {
            (field - 15) as u32
        } else {
            0
        };
        let routine = (15..=24).contains(&field) || matches!(field, 29 | 32);
        let pump = matches!(field, 12..=14 | 30 | 31);
        let row = if field < 9 {
            [0, 24, 16, 12, 8, 6, 4, 3, 1][field]
        } else {
            0
        };
        let lane = match field {
            12 => 1,
            13 => 2,
            _ => 0,
        };
        let layout = if routine {
            "dance-routine"
        } else if pump {
            "pump-single"
        } else {
            "dance-single"
        };
        let hold = matches!(field, 9 | 10 | 26 | 30 | 32);
        let roll = field == 10;
        let mine = field == 11;
        let fills =
            self.hold_colors
                .fills(layout, row, lane, player, selected, roll, field == 26, true);
        let normal = if mine {
            self.hold_colors.mine(layout, player, selected)
        } else {
            fills.map(|c| if hold { c.body } else { c.head })
        }
        .unwrap_or([0, 0, 0, 255]);
        let family = if routine {
            deadsync_editor::minimap_palette::Family::RoutineRow
        } else if pump && !self.note_colors.piu_row_based {
            deadsync_editor::minimap_palette::Family::PumpLane {
                start_index: 0,
                routine: false,
            }
        } else {
            deadsync_editor::minimap_palette::Family::Row
        };
        let ui = self
            .minimap
            .content
            .palette()
            .colors(family, row, lane, player, selected)
            .map(|c| {
                if mine {
                    c.mine
                } else if roll {
                    c.roll
                } else if hold {
                    c.hold
                } else {
                    c.head
                }
            })
            .unwrap_or(0xff000000);
        for (dx, rgb, key) in [
            (
                -160.0,
                normal[..3].try_into().unwrap(),
                "NoteColorChartPreview",
            ),
            (
                130.0,
                [
                    (ui & 255) as u8,
                    ((ui >> 8) & 255) as u8,
                    ((ui >> 16) & 255) as u8,
                ],
                "NoteColorUiPreview",
            ),
        ] {
            let [r, g, b] = rgb.map(|n| f32::from(n) / 255.0);
            actors
                .push(act!(quad:xy(x+dx,top+225.0):setsize(35.0,18.0):diffuse(r,g,b,1.0):z(8051)));
            actors.push(act!(text:font("miso"):settext(i18n::tr("Edition",key)):xy(x+dx+25.0,top+225.0):align(0.0,0.5):zoom(0.45):maxwidth(210.0):z(8051)));
        }
    }
}
