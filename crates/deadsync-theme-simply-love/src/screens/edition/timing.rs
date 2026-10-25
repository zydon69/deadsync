//! Optional BPM inspector. Drafts remain private until one explicit acceptance.
use super::*;
use deadsync_editor::timing_edit::{Scope, TempoDraft};

pub(super) struct Settings {
    draft: TempoDraft,
    beat: String,
    bpm: String,
    node: Option<u64>,
    fresh: bool,
    pending: bool,
}
impl Settings {
    fn new(draft: TempoDraft, row: u64) -> Self {
        let bpm = draft.tempos().at(row).to_string();
        let node = draft.tempos().values().contains_key(&row).then_some(row);
        Self {
            draft,
            beat: beat_text(row),
            bpm,
            node,
            fresh: true,
            pending: false,
        }
    }
    fn row(&self) -> Result<u64, &'static str> {
        let beat = self.beat.parse::<f64>().map_err(|_| "TimingInvalidBeat")?;
        let row = (beat * 48.0).round();
        if !beat.is_finite() || beat < 0.0 || row > i32::MAX as f64 {
            return Err("TimingInvalidBeat");
        }
        Ok(row as u64)
    }
}
fn beat_text(row: u64) -> String {
    format!("{:.9}", row as f64 / 48.0)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
impl State {
    pub(super) fn open_timing(&mut self) {
        if self.is_generating() {
            return;
        }
        if self
            .chart_properties
            .as_ref()
            .is_some_and(|draft| draft.has_changes())
        {
            self.status = i18n::tr("Edition", "TimingPropertiesPending");
            return;
        }
        let source = self.workspace.file().session.document();
        let scope = if deadsync_editor::timing_edit::supports_chart_timing(source) {
            Scope::Chart
        } else {
            Scope::Song
        };
        match self.workspace.tempo_draft(scope) {
            Ok(draft) => {
                self.timing = Some(Settings::new(draft, self.workspace.row()));
                self.chart_properties = None;
                self.panel = Some(Panel::Timing(0));
                self.status = i18n::tr("Edition", "TimingHelp");
            }
            Err(error) => self.timing_error(error),
        }
    }
    fn timing_error(&mut self, error: deadsync_editor::Error) {
        self.status = i18n::tr_fmt("Edition", "Error", &[("message", &error.to_string())]);
    }
    pub(super) fn timing_key(&mut self, index: usize, code: KeyCode) {
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::Timing(
                (index + if code == KeyCode::ArrowUp { 9 } else { 1 }) % 10,
            ));
            if let Some(settings) = &mut self.timing {
                settings.fresh = true;
            }
            return;
        }
        if matches!(index, 1 | 2) && self.timing_number(index, code) {
            return;
        }
        if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
            let forward = code == KeyCode::ArrowRight;
            if index == 0 {
                self.timing_scope();
            } else if let Some(settings) = &mut self.timing {
                if index == 1 {
                    match settings.row() {
                        Ok(row) => {
                            let step = if self.shift {
                                192
                            } else {
                                SNAP_ROWS[self.workspace.snap()]
                            };
                            let row = if forward {
                                row.saturating_add(step).min(i32::MAX as u64)
                            } else {
                                row.saturating_sub(step)
                            };
                            settings.beat = beat_text(row);
                            settings.pending = true;
                            settings.fresh = true;
                        }
                        Err(key) => self.status = i18n::tr("Edition", key),
                    }
                } else if index == 2 {
                    match settings.bpm.parse::<f64>() {
                        Ok(value) if value.is_finite() => {
                            let delta = if self.shift { 0.1 } else { 1.0 }
                                * if forward { 1.0 } else { -1.0 };
                            settings.bpm = (value + delta)
                                .max(deadsync_editor::timing_edit::tempo::MIN_BPM)
                                .to_string();
                            settings.pending = true;
                            settings.fresh = true;
                        }
                        _ => self.status = i18n::tr("Edition", "TimingInvalidBpm"),
                    }
                }
            }
        } else if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            match index {
                0 => self.timing_scope(),
                1 | 2 | 3 => {
                    self.timing_stage();
                }
                4 => self.timing_remove(),
                5 => self.timing_move(),
                6 => self.timing_navigate(false),
                7 => self.timing_navigate(true),
                8 => self.timing_apply(),
                9 => {
                    self.timing = None;
                    self.panel = None;
                    self.status = i18n::tr("Edition", "TimingDiscarded");
                }
                _ => {}
            }
        }
    }
    fn timing_number(&mut self, index: usize, code: KeyCode) -> bool {
        let Some(settings) = &mut self.timing else {
            return false;
        };
        let field = if index == 1 {
            &mut settings.beat
        } else {
            &mut settings.bpm
        };
        let digit = match code {
            KeyCode::Digit0 | KeyCode::Numpad0 => '0',
            KeyCode::Digit1 | KeyCode::Numpad1 => '1',
            KeyCode::Digit2 | KeyCode::Numpad2 => '2',
            KeyCode::Digit3 | KeyCode::Numpad3 => '3',
            KeyCode::Digit4 | KeyCode::Numpad4 => '4',
            KeyCode::Digit5 | KeyCode::Numpad5 => '5',
            KeyCode::Digit6 | KeyCode::Numpad6 => '6',
            KeyCode::Digit7 | KeyCode::Numpad7 => '7',
            KeyCode::Digit8 | KeyCode::Numpad8 => '8',
            KeyCode::Digit9 | KeyCode::Numpad9 => '9',
            KeyCode::Period | KeyCode::Comma | KeyCode::NumpadDecimal => '.',
            KeyCode::Backspace => {
                field.pop();
                settings.fresh = false;
                settings.pending = true;
                return true;
            }
            KeyCode::Delete => {
                field.clear();
                settings.fresh = false;
                settings.pending = true;
                return true;
            }
            _ => return false,
        };
        if settings.fresh {
            field.clear();
            settings.fresh = false;
        }
        if field.len() < 24 && (digit != '.' || !field.contains('.')) {
            field.push(digit);
        }
        settings.pending = true;
        true
    }
    fn timing_scope(&mut self) {
        let Some(settings) = &self.timing else {
            return;
        };
        if settings.pending || settings.draft.has_changes() {
            self.status = i18n::tr("Edition", "TimingScopePending");
            return;
        }
        let scope = if settings.draft.scope() == Scope::Chart {
            Scope::Song
        } else {
            Scope::Chart
        };
        match self.workspace.tempo_draft(scope) {
            Ok(draft) => self.timing = Some(Settings::new(draft, self.workspace.row())),
            Err(_) => self.status = i18n::tr("Edition", "TimingSharedOnly"),
        }
    }
    fn timing_stage(&mut self) -> bool {
        let Some(settings) = &mut self.timing else {
            return false;
        };
        let row = match settings.row() {
            Ok(row) => row,
            Err(key) => {
                self.status = i18n::tr("Edition", key);
                return false;
            }
        };
        let bpm = match settings.bpm.parse::<f64>() {
            Ok(value)
                if value.is_finite()
                    && value >= deadsync_editor::timing_edit::tempo::MIN_BPM
                    && (value as f32).is_finite() =>
            {
                value
            }
            _ => {
                self.status = i18n::tr("Edition", "TimingInvalidBpm");
                return false;
            }
        };
        match settings.draft.set(row, bpm) {
            Ok(_) => {
                settings.beat = beat_text(row);
                settings.node = Some(row);
                settings.pending = false;
                settings.fresh = true;
                self.status = i18n::tr("Edition", "TimingStaged");
                true
            }
            Err(error) => {
                self.timing_error(error);
                false
            }
        }
    }
    fn timing_remove(&mut self) {
        let Some(settings) = &mut self.timing else {
            return;
        };
        let row = match settings.row() {
            Ok(row) => row,
            Err(key) => {
                self.status = i18n::tr("Edition", key);
                return;
            }
        };
        if row == 0 {
            self.status = i18n::tr("Edition", "TimingInitialImmutable");
            return;
        }
        match settings.draft.remove(row) {
            Ok(changed) => {
                settings.node = None;
                settings.bpm = settings.draft.tempos().at(row).to_string();
                settings.pending = false;
                settings.fresh = true;
                self.status = i18n::tr(
                    "Edition",
                    if changed {
                        "TimingRemoved"
                    } else {
                        "TimingMissing"
                    },
                );
            }
            Err(error) => self.timing_error(error),
        }
    }
    fn timing_move(&mut self) {
        let Some(settings) = &mut self.timing else {
            return;
        };
        let Some(from) = settings.node else {
            self.status = i18n::tr("Edition", "TimingMissing");
            return;
        };
        let to = match settings.row() {
            Ok(row) => row,
            Err(key) => {
                self.status = i18n::tr("Edition", key);
                return;
            }
        };
        if from == 0 && to != 0 {
            self.status = i18n::tr("Edition", "TimingInitialImmutable");
            return;
        }
        if from != to && settings.draft.tempos().values().contains_key(&to) {
            self.status = i18n::tr("Edition", "TimingCollision");
            return;
        }
        if settings.bpm.parse::<f64>().ok() != Some(settings.draft.tempos().at(from)) {
            self.status = i18n::tr("Edition", "TimingMoveValue");
            return;
        }
        match settings.draft.move_node(from, to) {
            Ok(_) => {
                settings.node = Some(to);
                settings.beat = beat_text(to);
                settings.bpm = settings.draft.tempos().at(to).to_string();
                settings.pending = false;
                settings.fresh = true;
                self.status = i18n::tr("Edition", "TimingMoved");
            }
            Err(error) => self.timing_error(error),
        }
    }
    fn timing_navigate(&mut self, forward: bool) {
        let Some(settings) = &mut self.timing else {
            return;
        };
        if settings.pending {
            self.status = i18n::tr("Edition", "TimingInputPending");
            return;
        }
        let row = match settings.row() {
            Ok(row) => row,
            Err(key) => {
                self.status = i18n::tr("Edition", key);
                return;
            }
        };
        let nodes = settings.draft.tempos().values();
        let next = if forward {
            nodes
                .range((std::ops::Bound::Excluded(row), std::ops::Bound::Unbounded))
                .next()
                .or_else(|| nodes.first_key_value())
        } else {
            nodes
                .range(..row)
                .next_back()
                .or_else(|| nodes.last_key_value())
        };
        if let Some((&row, &bpm)) = next {
            settings.node = Some(row);
            settings.beat = beat_text(row);
            settings.bpm = bpm.to_string();
            settings.fresh = true;
            if let Err(error) = self.workspace.execute(Command::Seek(row)) {
                self.timing_error(error);
            }
            self.refresh();
        }
    }
    fn timing_apply(&mut self) {
        if self
            .timing
            .as_ref()
            .is_some_and(|settings| settings.pending)
            && !self.timing_stage()
        {
            return;
        }
        let Some(settings) = &self.timing else {
            return;
        };
        match self.workspace.apply_tempo(&settings.draft) {
            Ok(changed) => {
                self.timing = None;
                self.panel = None;
                self.refresh();
                self.status = i18n::tr(
                    "Edition",
                    if changed {
                        "TimingApplied"
                    } else {
                        "TimingNoChanges"
                    },
                );
            }
            Err(error) => self.timing_error(error),
        }
    }
    pub(super) fn timing_label(&self, index: usize, key: &str) -> Arc<str> {
        let Some(settings) = &self.timing else {
            return i18n::tr("Edition", key);
        };
        match index {
            0 => i18n::tr_fmt(
                "Edition",
                key,
                &[
                    (
                        "scope",
                        &i18n::tr(
                            "Edition",
                            if settings.draft.scope() == Scope::Song {
                                "TimingSong"
                            } else {
                                "TimingChart"
                            },
                        ),
                    ),
                    ("count", &settings.draft.affected_charts().len().to_string()),
                    (
                        "ownership",
                        &i18n::tr(
                            "Edition",
                            if settings.draft.inherited() {
                                "TimingInherited"
                            } else {
                                "TimingOwned"
                            },
                        ),
                    ),
                ],
            ),
            1 => i18n::tr_fmt("Edition", key, &[("beat", &settings.beat)]),
            2 => i18n::tr_fmt("Edition", key, &[("bpm", &settings.bpm)]),
            5 => i18n::tr_fmt(
                "Edition",
                key,
                &[(
                    "beat",
                    &settings.node.map(beat_text).unwrap_or_else(|| "-".into()),
                )],
            ),
            8 => i18n::tr_fmt(
                "Edition",
                key,
                &[("count", &settings.draft.tempos().values().len().to_string())],
            ),
            _ => i18n::tr("Edition", key),
        }
    }
}
