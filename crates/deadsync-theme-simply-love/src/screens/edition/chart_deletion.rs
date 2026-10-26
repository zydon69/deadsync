use super::{Panel, State};
use crate::{act, i18n};
use deadlib_platform::input::KeyCode;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};
use deadsync_editor::workspace::Command;
impl State {
    pub(super) fn open_chart_deletion(&mut self) {
        match self.workspace.deletion_draft() {
            Ok(draft) => {
                self.deletion = Some(draft);
                self.panel = Some(Panel::DeleteChart(0));
                self.status = i18n::tr("Edition", "DeleteChartHelp");
            }
            Err(error) => {
                self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
            }
        }
    }
    pub(super) fn deletion_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::DeleteChart(1 - index));
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if index == 0 {
                self.deletion = None;
                self.panel = None;
                self.status = i18n::tr("Edition", "Ready");
                return;
            }
            let Some(draft) = self.deletion.as_ref() else {
                return;
            };
            match self.workspace.delete_chart(draft) {
                Ok(()) => {
                    self.deletion = None;
                    self.panel = None;
                    self.status = i18n::tr("Edition", "ChartDeleted");
                    self.refresh();
                }
                Err(error) => {
                    self.status =
                        i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())])
                }
            }
        }
    }
    pub(super) fn deletion_summary(&self) -> std::sync::Arc<str> {
        let Some(draft) = &self.deletion else {
            return i18n::tr("Edition", "DeleteChartHelp");
        };
        i18n::tr_fmt(
            "Edition",
            if draft.is_last() {
                "DeleteLastChartSummary"
            } else {
                "DeleteChartSummary"
            },
            &[
                ("layout", draft.layout()),
                ("description", draft.description()),
            ],
        )
    }
    pub(super) fn no_chart_key(&mut self, code: KeyCode) -> bool {
        if self.control {
            let command = match code {
                KeyCode::KeyN => {
                    self.open_new_chart();
                    return false;
                }
                KeyCode::KeyS if self.shift => Some(Command::SaveRecovery),
                KeyCode::KeyS => Some(Command::Save),
                KeyCode::KeyZ if self.shift => Some(Command::Redo),
                KeyCode::KeyZ => Some(Command::Undo),
                KeyCode::KeyY => Some(Command::Redo),
                KeyCode::KeyR => Some(Command::Recover),
                _ => None,
            };
            if let Some(command) = command {
                self.command(command);
            }
        } else {
            match code {
                KeyCode::F1 => self.panel = Some(Panel::EmptyMenu(0)),
                KeyCode::F11 => self.open_song_properties(),
                KeyCode::Escape | KeyCode::F2 => return self.request_close(),
                _ => self.status = i18n::tr("Edition", "NoChartHelp"),
            }
        }
        false
    }
    pub(super) fn empty_menu_key(&mut self, index: usize, code: KeyCode) -> bool {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::EmptyMenu(
                (index + if code == KeyCode::ArrowUp { 7 } else { 1 }) % 8,
            ));
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            self.panel = None;
            match index {
                0 => self.open_new_chart(),
                1 => self.command(Command::Undo),
                2 => self.command(Command::Redo),
                3 => self.command(Command::Save),
                4 => self.command(Command::SaveRecovery),
                5 => self.command(Command::Recover),
                7 => self.open_pack_properties(),
                _ => return self.request_close(),
            }
        }
        false
    }
    pub(super) fn no_chart_actors(&self, actors: &mut Vec<Actor>) {
        let help = i18n::tr("Edition", "NoChartHelp");
        actors.push(act!(text: font("miso"): settext(help): align(0.0,0.0): xy(30.0,screen_height()*0.4): zoom(0.65): wrapwidthpixels((screen_width()-60.0)/0.65): maxwidth(screen_width()-60.0): maxheight(100.0): z(8006)));
        if let Some(panel) = self.panel {
            self.panel_actors(panel, actors);
        }
    }
}
