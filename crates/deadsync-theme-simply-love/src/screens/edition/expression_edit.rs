use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::generation::{
    config::{BracketDetermination, BracketMethod, ExpressedConfig},
    metadata::Guid,
    profiles::{Config, Kind, Profile},
};
pub(super) struct Draft {
    source: Guid,
    profile: Profile,
    config: ExpressedConfig,
    input: Option<(usize, String)>,
}
impl State {
    pub(super) fn open_expression_edit(&mut self) {
        let result: Result<Draft, String> = (|| {
            let settings = self
                .profile_selection
                .as_ref()
                .ok_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string())?;
            let entry = settings
                .expressed
                .and_then(|i| settings.catalog.entries(Kind::Expressed).get(i))
                .ok_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string())?;
            let source = settings
                .catalog
                .profile(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    Kind::Expressed,
                    &entry.guid,
                )
                .map_err(|e| e.to_string())?;
            let name = i18n::tr_fmt(
                "Edition",
                "ExpressionEditCopyName",
                &[("name", &super::profile_entry_name(entry))],
            );
            let profile = source.duplicate(&name).map_err(|e| e.to_string())?;
            let Config::Expressed(config) = profile.config().clone() else {
                return Err(i18n::tr("Edition", "ProfileMissing").to_string());
            };
            Ok(Draft {
                source: entry.guid.clone(),
                profile,
                config,
                input: None,
            })
        })();
        match result {
            Ok(draft) => {
                self.expression_edit = Some(draft);
                self.panel = Some(Panel::ExpressionEdit(0));
                self.status = i18n::tr("Edition", "ExpressionEditHelp");
            }
            Err(error) => self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error)]),
        }
    }
    fn stage_expression_input(&mut self) -> bool {
        let Some(draft) = self.expression_edit.as_mut() else {
            return false;
        };
        let Some((index, text)) = &draft.input else {
            return true;
        };
        if *index == 2 {
            let Ok(value) = text.trim().parse::<i32>() else {
                self.status = i18n::tr("Edition", "ExpressionEditInvalid");
                return false;
            };
            draft.config.min_level_for_brackets = value;
        } else {
            let Some(value) = text
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite() && *v >= 0.0)
            else {
                self.status = i18n::tr("Edition", "ExpressionEditInvalid");
                return false;
            };
            if *index == 4 {
                draft.config.balanced_bpm_for_aggressive = value;
            } else {
                draft.config.balanced_bpm_for_no_brackets = value;
            }
        }
        draft.input = None;
        true
    }
    pub(super) fn expression_edit_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape || (index == 7 && code == KeyCode::Enter) {
            self.expression_edit = None;
            self.panel = Some(Panel::ChartExpression(0));
            self.status = i18n::tr("Edition", "ExpressionEditDiscarded");
            return;
        }
        if !self.control && matches!(index, 2 | 4 | 5) {
            let character = match code {
                KeyCode::Digit0 | KeyCode::Numpad0 => Some('0'),
                KeyCode::Digit1 | KeyCode::Numpad1 => Some('1'),
                KeyCode::Digit2 | KeyCode::Numpad2 => Some('2'),
                KeyCode::Digit3 | KeyCode::Numpad3 => Some('3'),
                KeyCode::Digit4 | KeyCode::Numpad4 => Some('4'),
                KeyCode::Digit5 | KeyCode::Numpad5 => Some('5'),
                KeyCode::Digit6 | KeyCode::Numpad6 => Some('6'),
                KeyCode::Digit7 | KeyCode::Numpad7 => Some('7'),
                KeyCode::Digit8 | KeyCode::Numpad8 => Some('8'),
                KeyCode::Digit9 | KeyCode::Numpad9 => Some('9'),
                KeyCode::Minus | KeyCode::NumpadSubtract => Some('-'),
                KeyCode::Period | KeyCode::NumpadDecimal => Some('.'),
                KeyCode::KeyE => Some('e'),
                KeyCode::NumpadAdd => Some('+'),
                KeyCode::Equal if self.shift => Some('+'),
                _ => None,
            };
            if let Some(draft) = self.expression_edit.as_mut() {
                if let Some(c) = character {
                    let (_, text) = draft.input.get_or_insert_with(|| (index, String::new()));
                    if text.len() < 64 {
                        text.push(c);
                    }
                    return;
                }
                if code == KeyCode::Delete {
                    draft.input = Some((index, String::new()));
                    return;
                }
                if code == KeyCode::Backspace {
                    let value = match index {
                        2 => draft.config.min_level_for_brackets.to_string(),
                        4 => draft.config.balanced_bpm_for_aggressive.to_string(),
                        _ => draft.config.balanced_bpm_for_no_brackets.to_string(),
                    };
                    draft.input.get_or_insert((index, value)).1.pop();
                    return;
                }
            }
        }
        if !self.stage_expression_input() {
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            self.panel = Some(Panel::ExpressionEdit(
                (index
                    + if code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift) {
                        7
                    } else {
                        1
                    })
                    % 8,
            ));
            return;
        }
        if index == 6 && code == KeyCode::Enter {
            self.save_expression_copy();
            return;
        }
        if !matches!(
            code,
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter
        ) {
            return;
        }
        let forward = code != KeyCode::ArrowLeft;
        let Some(draft) = self.expression_edit.as_mut() else {
            return;
        };
        let c = &mut draft.config;
        match index {
            0 => {
                let values = [
                    BracketMethod::Aggressive,
                    BracketMethod::Balanced,
                    BracketMethod::NoBrackets,
                ];
                let i = values
                    .iter()
                    .position(|v| *v == c.default_bracket_parsing_method)
                    .unwrap();
                c.default_bracket_parsing_method = values[(i + if forward { 1 } else { 2 }) % 3];
            }
            1 => {
                c.bracket_parsing_determination = if c.bracket_parsing_determination
                    == BracketDetermination::ChooseMethodDynamically
                {
                    BracketDetermination::UseDefaultMethod
                } else {
                    BracketDetermination::ChooseMethodDynamically
                }
            }
            3 => c.aggressive_when_uncoverable = !c.aggressive_when_uncoverable,
            2 if code != KeyCode::Enter => {
                c.min_level_for_brackets =
                    c.min_level_for_brackets
                        .saturating_add(if forward { 1 } else { -1 })
            }
            4 | 5 if code != KeyCode::Enter => {
                let value = if index == 4 {
                    &mut c.balanced_bpm_for_aggressive
                } else {
                    &mut c.balanced_bpm_for_no_brackets
                };
                *value = (*value + if forward { 1.0 } else { -1.0 }).max(0.0);
            }
            _ => {}
        }
    }
    fn save_expression_copy(&mut self) {
        let result: Result<Guid, String> = (|| {
            let directory = self
                .profile_directory
                .as_deref()
                .ok_or_else(|| i18n::tr("Edition", "ExpressionEditUnavailable").to_string())?;
            let draft = self
                .expression_edit
                .as_mut()
                .ok_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string())?;
            let settings = self
                .profile_selection
                .as_ref()
                .ok_or_else(|| i18n::tr("Edition", "ProfileMissing").to_string())?;
            settings
                .catalog
                .profile(
                    &self.workspace.file().session,
                    self.workspace.chart(),
                    Kind::Expressed,
                    &draft.source,
                )
                .map_err(|e| e.to_string())?;
            if draft.config.balanced_bpm_for_aggressive != 0.0
                && draft.config.balanced_bpm_for_no_brackets != 0.0
                && draft.config.balanced_bpm_for_aggressive
                    <= draft.config.balanced_bpm_for_no_brackets
            {
                return Err(i18n::tr("Edition", "ExpressionEditThresholdOrder").to_string());
            }
            draft
                .profile
                .set_config(Config::Expressed(draft.config.clone()))
                .map_err(|e| e.to_string())?;
            draft
                .profile
                .export_new(directory)
                .map_err(|e| e.to_string())?;
            Ok(draft.profile.guid().clone())
        })();
        match result {
            Ok(guid) => {
                self.expression_edit = None;
                self.open_chart_expression();
                if self.catalog_job.is_some() {
                    self.expression_saved_guid = Some(guid);
                }
            }
            Err(error) => self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error)]),
        }
    }
    pub(super) fn expression_edit_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let Some(draft) = &self.expression_edit else {
            return i18n::tr("Edition", key);
        };
        let c = &draft.config;
        let value = match index {
            0 => i18n::tr(
                "Edition",
                match c.default_bracket_parsing_method {
                    BracketMethod::Aggressive => "ExpressionMethodAggressive",
                    BracketMethod::Balanced => "ExpressionMethodBalanced",
                    BracketMethod::NoBrackets => "ExpressionMethodNone",
                },
            )
            .to_string(),
            1 => i18n::tr(
                "Edition",
                if c.bracket_parsing_determination == BracketDetermination::ChooseMethodDynamically
                {
                    "ExpressionDeterminationDynamic"
                } else {
                    "ExpressionDeterminationDefault"
                },
            )
            .to_string(),
            2 => c.min_level_for_brackets.to_string(),
            3 => i18n::tr(
                "Edition",
                if c.aggressive_when_uncoverable {
                    "PatternYes"
                } else {
                    "PatternNo"
                },
            )
            .to_string(),
            4 => format!("{:.6}", c.balanced_bpm_for_aggressive),
            5 => format!("{:.6}", c.balanced_bpm_for_no_brackets),
            _ => return i18n::tr("Edition", key),
        };
        let value = draft
            .input
            .as_ref()
            .filter(|(i, _)| *i == index)
            .map(|(_, text)| text.as_str())
            .unwrap_or(&value);
        i18n::tr_fmt("Edition", key, &[("value", value)])
    }
}
