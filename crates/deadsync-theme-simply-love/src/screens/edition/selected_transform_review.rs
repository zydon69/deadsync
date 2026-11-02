//! Exact checked spatial/owner actions, with individual side-effect review.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::{
    note_operation::Scope,
    selected_transform::{Impact, ImpactKind, Plan, Refusal},
    transform::Transform,
};
use std::{
    collections::BTreeSet,
    ops::Bound::{Excluded, Unbounded},
};
#[derive(Default)]
pub(super) struct Settings {
    operation: usize,
    scope: usize,
    a: u32,
    b: u32,
    from_picker: bool,
    plan: Option<Plan>,
    reviewed: BTreeSet<Impact>,
    current: Option<Impact>,
    effect_index: usize,
}
impl State {
    pub(super) fn command_selected_transform_review(&mut self, transform: Transform) {
        if self.event_selection.is_none() {
            self.status = i18n::tr("Edition", "NoteOperationMissingSelection");
            return;
        }
        let (operation, a, b) = match transform {
            Transform::Mirror => (0, 0, 0),
            Transform::Flip => (1, 0, 0),
            Transform::MirrorAndFlip => (2, 0, 0),
            Transform::ShiftLanes {
                right: false,
                wrap: false,
            } => (3, 0, 0),
            Transform::ShiftLanes {
                right: true,
                wrap: false,
            } => (4, 0, 0),
            Transform::ShiftLanes {
                right: false,
                wrap: true,
            } => (5, 0, 0),
            Transform::ShiftLanes {
                right: true,
                wrap: true,
            } => (6, 0, 0),
            Transform::AssignPlayer(player) => (9, player, 0),
            Transform::SwapPlayers(a, b) => (10, a, b),
            _ => {
                self.status = i18n::tr("Edition", "SelectedTransformFailed");
                return;
            }
        };
        self.open_selected_transform_review(true);
        self.selected_transform_review.operation = operation;
        self.selected_transform_review.a = a;
        self.selected_transform_review.b = b;
        self.prepare_selected_transform_review();
        self.panel = Some(Panel::SelectedTransformReview(4));
    }

    pub(super) fn open_selected_transform_review(&mut self, from_picker: bool) {
        self.selected_transform_review = Settings {
            from_picker,
            scope: if from_picker { 1 } else { 2 },
            a: self.workspace.player(),
            b: (self.workspace.player() + 1) % self.workspace.grid().players(),
            ..Default::default()
        };
        self.panel = Some(Panel::SelectedTransformReview(0));
        self.status = i18n::tr("Edition", "SelectedTransformHelp");
    }
    pub(super) fn selected_transform_review_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.close_selected_transform_review(),
            KeyCode::ArrowUp => self.panel = Some(Panel::SelectedTransformReview((index + 7) % 8)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::SelectedTransformReview((index + 1) % 8))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 4 => {
                let settings = &mut self.selected_transform_review;
                let forward = code != KeyCode::ArrowLeft;
                match index {
                    0 => {
                        settings.operation =
                            (settings.operation + if forward { 1 } else { 10 }) % 11
                    }
                    1 => {
                        let step = if forward { 1 } else { 2 };
                        settings.scope = (settings.scope + step) % 3;
                        if settings.scope == 1 && self.event_selection.is_none() {
                            settings.scope = (settings.scope + step) % 3;
                        }
                    }
                    2 | 3 if settings.operation >= 9 => {
                        let count = self.workspace.grid().players();
                        let value = if index == 2 {
                            &mut settings.a
                        } else {
                            &mut settings.b
                        };
                        *value = (*value + if forward { 1 } else { count - 1 }) % count;
                    }
                    _ => return,
                }
                settings.plan = None;
                settings.reviewed.clear();
                settings.current = None;
                settings.effect_index = 0;
            }
            KeyCode::Enter if index == 4 => {
                self.prepare_selected_transform_review();
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index == 5 => {
                let settings = &mut self.selected_transform_review;
                if let (Some(plan), Some(current)) = (&settings.plan, settings.current) {
                    if code == KeyCode::Enter {
                        settings.reviewed.insert(current);
                    }
                    let total = plan.impacts().len();
                    if code == KeyCode::ArrowLeft {
                        settings.current = plan
                            .impacts()
                            .range(..current)
                            .next_back()
                            .or_else(|| plan.impacts().last())
                            .copied();
                        settings.effect_index = (settings.effect_index + total - 1) % total;
                    } else {
                        settings.current = plan
                            .impacts()
                            .range((Excluded(current), Unbounded))
                            .next()
                            .or_else(|| plan.impacts().first())
                            .copied();
                        settings.effect_index = (settings.effect_index + 1) % total;
                    }
                }
            }
            KeyCode::Enter if index == 6 => {
                let settings = &self.selected_transform_review;
                let Some(plan) = &settings.plan else {
                    self.status = i18n::tr("Edition", "NoteOperationPrepareFirst");
                    return;
                };
                match self
                    .workspace
                    .apply_selected_transform(plan, &settings.reviewed)
                {
                    Ok(changed) => {
                        self.selected_transform_review = Default::default();
                        self.event_selection = None;
                        self.panel = None;
                        self.sync_event_selection_history();
                        self.status = i18n::tr(
                            "Edition",
                            if changed {
                                "SelectedTransformApplied"
                            } else {
                                "NoteOperationNoChange"
                            },
                        );
                        self.refresh();
                    }
                    Err(error) => self.status = i18n::tr("Edition", refusal_key(error.kind())),
                }
            }
            KeyCode::Enter => self.close_selected_transform_review(),
            _ => {}
        }
    }
    fn prepare_selected_transform_review(&mut self) {
        let operation = self.selected_transform_operation();
        let result = if self.selected_transform_review.scope == 1 {
            match &self.event_selection {
                Some(settings) => self
                    .workspace
                    .prepare_selected_transform(&settings.selection, operation),
                None => {
                    self.status = i18n::tr("Edition", "NoteOperationMissingSelection");
                    return;
                }
            }
        } else {
            let scope = if self.selected_transform_review.scope == 0 {
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
            self.workspace.prepare_transform_scope(scope, operation)
        };
        match result {
            Ok(plan) => {
                let settings = &mut self.selected_transform_review;
                settings.current = plan.impacts().first().copied();
                settings.effect_index = 0;
                settings.reviewed.clear();
                settings.plan = Some(plan);
                self.status = i18n::tr("Edition", "SelectedTransformPrepared");
            }
            Err(error) => self.status = i18n::tr("Edition", refusal_key(error.kind())),
        }
    }
    fn close_selected_transform_review(&mut self) {
        let from_picker = self.selected_transform_review.from_picker;
        self.selected_transform_review = Default::default();
        self.panel = Some(if from_picker {
            Panel::EventSelection
        } else {
            Panel::Transforms(0)
        });
    }
    fn selected_transform_operation(&self) -> Transform {
        let settings = &self.selected_transform_review;
        match settings.operation {
            0 => Transform::Mirror,
            1 => Transform::Flip,
            2 => Transform::MirrorAndFlip,
            3 => Transform::ShiftLanes {
                right: false,
                wrap: false,
            },
            4 => Transform::ShiftLanes {
                right: true,
                wrap: false,
            },
            5 => Transform::ShiftLanes {
                right: false,
                wrap: true,
            },
            6 => Transform::ShiftLanes {
                right: true,
                wrap: true,
            },
            7 => Transform::ShiftRows(
                -(deadsync_editor::workspace::snap_shift_rows(self.workspace.snap()) as i64),
            ),
            8 => Transform::ShiftRows(deadsync_editor::workspace::snap_shift_rows(
                self.workspace.snap(),
            ) as i64),
            9 => Transform::AssignPlayer(settings.a),
            _ => Transform::SwapPlayers(settings.a, settings.b),
        }
    }
    pub(super) fn selected_transform_review_label(
        &self,
        index: usize,
        key: &str,
    ) -> std::sync::Arc<str> {
        let settings = &self.selected_transform_review;
        let value = match index {
            0 => i18n::tr(
                "Edition",
                [
                    "Mirror",
                    "Flip",
                    "MirrorAndFlip",
                    "ShiftLanesLeft",
                    "ShiftLanesRight",
                    "WrapLanesLeft",
                    "WrapLanesRight",
                    "ShiftEarlier",
                    "ShiftLater",
                    "SelectedTransformAssign",
                    "SelectedTransformSwapPlayers",
                ][settings.operation],
            )
            .to_string(),
            1 => i18n::tr(
                "Edition",
                [
                    "NoteOperationAll",
                    "NoteOperationChecked",
                    "NoteOperationRange",
                ][settings.scope],
            )
            .to_string(),
            2 if settings.operation >= 9 => (settings.a + 1).to_string(),
            3 if settings.operation == 10 => (settings.b + 1).to_string(),
            2 | 3 => i18n::tr("Edition", "SelectedTransformNotApplicable").to_string(),
            4 => settings
                .plan
                .as_ref()
                .map(|p| {
                    let r = p.report();
                    format!(
                        "{} / {} / {} / {}",
                        r.selected, r.moved, r.deleted, r.other_affected
                    )
                })
                .unwrap_or_else(|| i18n::tr("Edition", "TimingConversionNotPrepared").to_string()),
            5 => {
                if let (Some(plan), Some(impact)) = (&settings.plan, settings.current) {
                    i18n::tr_fmt(
                        "Edition",
                        "SelectedTransformImpactDescription",
                        &[
                            ("index", &(settings.effect_index + 1).to_string()),
                            ("total", &plan.impacts().len().to_string()),
                            ("row", &impact.position.row.to_string()),
                            ("lane", &(impact.position.lane + 1).to_string()),
                            ("player", &(impact.position.player + 1).to_string()),
                            ("kind", &i18n::tr("Edition", impact_key(impact.kind))),
                            ("reviewed", &settings.reviewed.len().to_string()),
                        ],
                    )
                    .to_string()
                } else {
                    i18n::tr("Edition", "SelectedTransformNoImpacts").to_string()
                }
            }
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}
fn impact_key(kind: ImpactKind) -> &'static str {
    match kind {
        ImpactKind::SelectedDeleted => "SelectedTransformSelectedDeleted",
        ImpactKind::SelectedChanged => "SelectedTransformSelectedChanged",
        ImpactKind::OtherDeleted => "SelectedTransformOtherDeleted",
        ImpactKind::OtherChanged => "SelectedTransformOtherChanged",
    }
}
fn refusal_key(reason: Refusal) -> &'static str {
    match reason {
        Refusal::OwnerCollision => "SelectedTransformOwnerCollision",
        Refusal::MissingReview => "SelectedTransformMissingReview",
        Refusal::Stale => "NoteOperationStale",
        Refusal::Invalid => "SelectedTransformFailed",
    }
}
