//! Optional review for whole-chart and checked-head Convert/Swap actions.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::note_operation::{Operation, Plan, Refusal, Scope};
#[derive(Default)]
pub(super) struct Settings {
    swap: bool,
    source_operation: Option<Operation>,
    scope: usize,
    from_picker: bool,
    plan: Option<Plan>,
}
impl State {
    pub(super) fn command_note_operation_review(&mut self, operation: Operation) {
        if self.event_selection.is_none() {
            self.status = i18n::tr("Edition", "NoteOperationMissingSelection");
            return;
        }
        self.open_note_operation_review(true);
        self.note_operation_review.source_operation = Some(operation);
        self.prepare_note_operation_review();
        self.panel = Some(Panel::NoteOperationReview(2));
    }
    fn reviewed_note_operation(&self) -> Operation {
        self.note_operation_review
            .source_operation
            .unwrap_or_else(|| self.note_conversion_operation(self.note_operation_review.swap))
    }

    pub(super) fn open_note_operation_review(&mut self, from_picker: bool) {
        self.note_operation_review = Settings {
            from_picker,
            scope: usize::from(from_picker),
            ..Default::default()
        };
        self.panel = Some(Panel::NoteOperationReview(0));
        self.status = i18n::tr("Edition", "NoteOperationHelp");
    }
    pub(super) fn note_operation_review_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.close_note_operation_review(),
            KeyCode::ArrowUp => self.panel = Some(Panel::NoteOperationReview((index + 4) % 5)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::NoteOperationReview((index + 1) % 5))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 2 => {
                if index == 0 {
                    self.note_operation_review.swap = !self.note_operation_review.swap;
                    self.note_operation_review.source_operation = None;
                } else {
                    let step = if code == KeyCode::ArrowLeft { 2 } else { 1 };
                    self.note_operation_review.scope =
                        (self.note_operation_review.scope + step) % 3;
                    if self.note_operation_review.scope == 1 && self.event_selection.is_none() {
                        self.note_operation_review.scope =
                            (self.note_operation_review.scope + step) % 3;
                    }
                }
                self.note_operation_review.plan = None;
            }
            KeyCode::Enter if index == 2 => {
                self.prepare_note_operation_review();
            }
            KeyCode::Enter if index == 3 => {
                let Some(plan) = &self.note_operation_review.plan else {
                    self.status = i18n::tr("Edition", "NoteOperationPrepareFirst");
                    return;
                };
                match self.workspace.apply_note_operation(plan) {
                    Ok(changed) => {
                        self.note_operation_review = Default::default();
                        self.event_selection = None;
                        self.panel = None;
                        self.sync_event_selection_history();
                        self.status = i18n::tr(
                            "Edition",
                            if changed {
                                "NoteOperationApplied"
                            } else {
                                "NoteOperationNoChange"
                            },
                        );
                        self.refresh();
                    }
                    Err(error) => self.status = i18n::tr("Edition", refusal_key(error.kind())),
                }
            }
            KeyCode::Enter => self.close_note_operation_review(),
            _ => {}
        }
    }
    fn prepare_note_operation_review(&mut self) {
        let operation = self.reviewed_note_operation();
        let result = if self.note_operation_review.scope == 1 {
            match &self.event_selection {
                Some(settings) => self
                    .workspace
                    .prepare_selected_note_operation(&settings.selection, operation),
                None => {
                    self.status = i18n::tr("Edition", "NoteOperationMissingSelection");
                    return;
                }
            }
        } else {
            let scope = if self.note_operation_review.scope == 0 {
                Scope::All { player: None }
            } else {
                let (start, end) = self
                    .workspace
                    .selection()
                    .unwrap_or((self.workspace.row(), self.workspace.row()));
                Scope::Range {
                    start,
                    end,
                    player: None,
                }
            };
            self.workspace.prepare_note_operation(scope, operation)
        };
        match result {
            Ok(plan) => {
                self.note_operation_review.plan = Some(plan);
                self.status = i18n::tr("Edition", "NoteOperationPrepared");
            }
            Err(error) => self.status = i18n::tr("Edition", refusal_key(error.kind())),
        }
    }
    fn close_note_operation_review(&mut self) {
        let from_picker = self.note_operation_review.from_picker;
        self.note_operation_review = Default::default();
        self.panel = Some(if from_picker {
            Panel::EventSelection
        } else {
            Panel::NoteConversion(0)
        });
    }
    pub(super) fn note_operation_review_label(
        &self,
        index: usize,
        key: &str,
    ) -> std::sync::Arc<str> {
        let settings = &self.note_operation_review;
        let value = match index {
            0 => match self.reviewed_note_operation() {
                Operation::Convert {
                    from,
                    to,
                    sustain_rows,
                } => i18n::tr_fmt(
                    "Edition",
                    "NoteOperationConvertDescription",
                    &[
                        ("from", &i18n::tr("Edition", &format!("{from:?}"))),
                        ("to", &i18n::tr("Edition", &format!("{to:?}"))),
                        ("duration", &sustain_rows.to_string()),
                    ],
                )
                .to_string(),
                Operation::Swap(pair) => {
                    let (a, b) = pair.kinds();
                    i18n::tr_fmt(
                        "Edition",
                        "NoteOperationSwapDescription",
                        &[
                            ("from", &i18n::tr("Edition", &format!("{a:?}"))),
                            ("to", &i18n::tr("Edition", &format!("{b:?}"))),
                        ],
                    )
                    .to_string()
                }
            },
            1 => i18n::tr(
                "Edition",
                [
                    "NoteOperationAll",
                    "NoteOperationChecked",
                    "NoteOperationRange",
                ][settings.scope],
            )
            .to_string(),
            2 => settings
                .plan
                .as_ref()
                .map(|p| {
                    let r = p.report();
                    format!("{} / {} / {}", r.selected, r.matched, r.changed)
                })
                .unwrap_or_else(|| i18n::tr("Edition", "TimingConversionNotPrepared").to_string()),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
fn refusal_key(reason: Refusal) -> &'static str {
    match reason {
        Refusal::ZeroDurationSustain => "NoteSwapZeroSustain",
        Refusal::Stale => "NoteOperationStale",
        Refusal::Invalid => "NoteOperationFailed",
    }
}
