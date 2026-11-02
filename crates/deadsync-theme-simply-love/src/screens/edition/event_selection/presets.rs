//! Keyboard equivalent of original whole-chart Select All menus.
use super::super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::event_clipboard::presets::{Mode, Preset};
#[derive(Default)]
pub(super) struct Settings {
    preset: usize,
    player: usize,
    mode: usize,
}
impl State {
    pub(in crate::screens::edition) fn open_selection_presets(&mut self) {
        if let Some(settings) = &mut self.event_selection {
            settings.presets = Some(Settings::default());
            self.panel = Some(Panel::SelectionPresets(0));
            self.status = i18n::tr("Edition", "SelectionPresetsHelp");
        }
    }
    pub(in crate::screens::edition) fn selection_presets_key(
        &mut self,
        index: usize,
        code: KeyCode,
    ) {
        let Some(mut settings) = self.event_selection.take() else {
            return;
        };
        let Some(draft) = &mut settings.presets else {
            self.event_selection = Some(settings);
            self.panel = Some(Panel::EventSelection);
            return;
        };
        match code {
            KeyCode::Escape => {
                settings.presets = None;
                self.panel = Some(Panel::EventSelection);
            }
            KeyCode::ArrowUp => self.panel = Some(Panel::SelectionPresets((index + 4) % 5)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::SelectionPresets((index + 1) % 5))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 3 => {
                let forward = code != KeyCode::ArrowLeft;
                let (count, value) = match index {
                    0 => (Preset::ALL.len(), &mut draft.preset),
                    1 if Preset::ALL[draft.preset].supports_player() => (
                        self.workspace.grid().players() as usize + 2,
                        &mut draft.player,
                    ),
                    1 => {
                        self.event_selection = Some(settings);
                        return;
                    }
                    _ => (3, &mut draft.mode),
                };
                *value = (*value + if forward { 1 } else { count - 1 }) % count;
                if !Preset::ALL[draft.preset].supports_player() {
                    draft.player = 0;
                }
            }
            KeyCode::Enter if index == 3 => {
                let player = match draft.player {
                    0 => None,
                    1 => Some(self.workspace.player()),
                    p => Some((p - 2) as u32),
                };
                let preset = Preset::ALL[draft.preset];
                let mode = [Mode::Add, Mode::Replace, Mode::Remove][draft.mode];
                match self
                    .workspace
                    .expand_event_selection(&settings.selection)
                    .and_then(|mut selection| {
                        let report = selection.apply_preset(preset, player, mode)?;
                        Ok((selection, report))
                    }) {
                    Ok((selection, report)) => {
                        settings.index = report
                            .last_match
                            .and_then(|event| {
                                selection
                                    .events()
                                    .iter()
                                    .position(|candidate| *candidate == event)
                            })
                            .unwrap_or(0);
                        settings.selection = selection;
                        settings.presets = None;
                        self.panel = Some(Panel::EventSelection);
                        self.status = i18n::tr_fmt(
                            "Edition",
                            "SelectionPresetsApplied",
                            &[
                                ("matched", &report.matched.to_string()),
                                ("selected", &report.selected.to_string()),
                            ],
                        );
                    }
                    Err(_) => self.status = i18n::tr("Edition", "SelectionPresetsFailed"),
                }
            }
            KeyCode::Enter => {
                settings.presets = None;
                self.panel = Some(Panel::EventSelection);
            }
            _ => {}
        }
        self.event_selection = Some(settings);
    }
    pub(in crate::screens::edition) fn selection_presets_label(
        &self,
        index: usize,
        key: &str,
    ) -> std::sync::Arc<str> {
        let Some(draft) = self
            .event_selection
            .as_ref()
            .and_then(|s| s.presets.as_ref())
        else {
            return i18n::tr("Edition", key);
        };
        let value = match index {
            0 => i18n::tr("Edition", preset_key(Preset::ALL[draft.preset])).to_string(),
            1 => match draft.player {
                0 => i18n::tr("Edition", "SelectionPresetsAllPlayers").to_string(),
                1 => i18n::tr_fmt(
                    "Edition",
                    "SelectionPresetsCurrentPlayer",
                    &[("value", &(self.workspace.player() + 1).to_string())],
                )
                .to_string(),
                p => i18n::tr_fmt(
                    "Edition",
                    "SelectionPresetsSpecificPlayer",
                    &[("value", &(p - 1).to_string())],
                )
                .to_string(),
            },
            2 => i18n::tr(
                "Edition",
                [
                    "SelectionPresetsAdd",
                    "SelectionPresetsReplace",
                    "SelectionPresetsRemove",
                ][draft.mode],
            )
            .to_string(),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
fn preset_key(preset: Preset) -> &'static str {
    match preset {
        Preset::Notes => "SelectionPresetsNotes",
        Preset::Taps => "Tap",
        Preset::Mines => "Mine",
        Preset::Fakes => "Fake",
        Preset::Lifts => "Lift",
        Preset::Holds => "Hold",
        Preset::Rolls => "Roll",
        Preset::HoldsAndRolls => "SelectionPresetsHoldsRolls",
        Preset::Miscellaneous => "SelectionPresetsMisc",
        Preset::NotesAndMiscellaneous => "SelectionPresetsNotesMisc",
        Preset::Patterns => "SelectionPresetsPatterns",
    }
}
