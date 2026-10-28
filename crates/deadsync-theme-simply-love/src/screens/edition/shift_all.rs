//! Explicit source Shift All row route, using the mixed-event transaction.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::shift_all::Plan;
#[derive(Default)]
pub(super) struct Settings {
    later: bool,
    plan: Option<Plan>,
}
impl State {
    /// Native menu extension: source ShiftEarlier/Later are selected mixed events.
    pub(super) fn command_shift_all_review(&mut self, later: bool) {
        self.open_shift_all();
        self.shift_all.later = later;
        self.prepare_shift_all_review();
        self.panel = Some(Panel::ShiftAll(1));
    }

    pub(super) fn open_shift_all(&mut self) {
        self.shift_all = Settings {
            later: true,
            ..Default::default()
        };
        self.panel = Some(Panel::ShiftAll(0));
        self.status = i18n::tr("Edition", "ShiftAllHelp");
    }
    pub(super) fn shift_all_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.close_shift_all(),
            KeyCode::ArrowUp => self.panel = Some(Panel::ShiftAll((index + 3) % 4)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::ShiftAll((index + 1) % 4))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index == 0 => {
                self.shift_all.later = !self.shift_all.later;
                self.shift_all.plan = None;
            }
            KeyCode::Enter if index == 1 => {
                self.prepare_shift_all_review();
            }
            KeyCode::Enter if index == 2 => {
                let Some(plan) = self.shift_all.plan.take() else {
                    self.status = i18n::tr("Edition", "NoteOperationPrepareFirst");
                    return;
                };
                let report = plan.report().clone();
                match self.workspace.apply_shift_all(plan, &report) {
                    Ok(_) => {
                        self.event_selection = None;
                        self.panel = None;
                        self.status = i18n::tr("Edition", "ShiftAllApplied");
                        self.refresh();
                    }
                    Err(error) => {
                        self.status = i18n::tr(
                            "Edition",
                            match error.reason {
                                deadsync_editor::shift_all::Refusal::Stale => "ShiftAllStale",
                                deadsync_editor::shift_all::Refusal::MissingReview => {
                                    "NoteOperationPrepareFirst"
                                }
                                deadsync_editor::shift_all::Refusal::Invalid => "ShiftAllRefused",
                            },
                        )
                    }
                }
            }
            KeyCode::Enter if index == 3 => self.close_shift_all(),
            _ => {}
        }
    }
    fn prepare_shift_all_review(&mut self) {
        match self.workspace.prepare_shift_all(self.shift_all.later) {
            Ok(plan) => {
                self.shift_all.plan = Some(plan);
                self.status = i18n::tr("Edition", "ShiftAllPrepared");
            }
            Err(_) => {
                self.shift_all.plan = None;
                self.status = i18n::tr("Edition", "ShiftAllRefused");
            }
        }
    }
    fn close_shift_all(&mut self) {
        self.shift_all = Default::default();
        self.panel = Some(Panel::Transforms(0));
    }
    pub(super) fn shift_all_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let text = match index {
            0 => i18n::tr("Edition", key)
                .replace(
                    "{value}",
                    &i18n::tr(
                        "Edition",
                        if self.shift_all.later {
                            "ShiftAllLater"
                        } else {
                            "ShiftAllEarlier"
                        },
                    ),
                )
                .replace(
                    "{rows}",
                    &deadsync_editor::workspace::snap_shift_rows(self.workspace.snap()).to_string(),
                ),
            1 => {
                let report = self.shift_all.plan.as_ref().map(|p| p.report());
                i18n::tr("Edition", key)
                    .replace("{moved}", &report.map_or(0, |r| r.moved).to_string())
                    .replace("{deleted}", &report.map_or(0, |r| r.deleted).to_string())
                    .replace(
                        "{immutable}",
                        &report.map_or(0, |r| r.immutable).to_string(),
                    )
            }
            _ => i18n::tr("Edition", key).to_string(),
        };
        text.into()
    }
}
