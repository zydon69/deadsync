//! Optional timing inspector. Drafts remain private until one explicit acceptance.
use super::*;
mod draft;
use deadsync_editor::timing_edit::Scope;
use draft::{Draft, EventKind};

pub(super) struct Settings {
    draft: Draft,
    beat: String,
    value: String,
    node: Option<u64>,
    fresh: bool,
    pending: bool,
}
impl Settings {
    fn new(draft: Draft, row: u64) -> Self {
        let value = draft.value(row).to_string();
        let node = draft.node(row).map(|_| row);
        Self {
            draft,
            beat: beat_text(row),
            value,
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
        match Draft::capture(&self.workspace, scope, EventKind::Bpm) {
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
        if code == KeyCode::Tab {
            self.timing_type();
            return;
        }
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
                    match settings.value.parse::<f64>() {
                        Ok(value) if value.is_finite() => {
                            let delta = if settings.draft.kind() == EventKind::Bpm {
                                if self.shift { 0.1 } else { 1.0 }
                            } else if settings.draft.kind().is_region() {
                                if self.shift {
                                    1.0 / 48.0
                                } else {
                                    SNAP_ROWS[self.workspace.snap()] as f64 / 48.0
                                }
                            } else if self.shift {
                                0.001
                            } else {
                                0.01
                            } * if forward { 1.0 } else { -1.0 };
                            let value = value + delta;
                            settings.value = match settings.draft.kind() {
                                EventKind::Bpm => {
                                    value.max(deadsync_editor::timing_edit::tempo::MIN_BPM)
                                }
                                EventKind::Delay => value.max(0.0),
                                EventKind::Stop => value,
                                EventKind::Warp | EventKind::Fake => {
                                    (value * 48.0).round().max(1.0) / 48.0
                                }
                            }
                            .to_string();
                            settings.pending = true;
                            settings.fresh = true;
                        }
                        _ => {
                            self.status =
                                i18n::tr("Edition", settings.draft.key("TimingInvalidBpm"))
                        }
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
            &mut settings.value
        };
        if index == 2 && matches!(code, KeyCode::Minus | KeyCode::NumpadSubtract) {
            if settings.fresh {
                field.clear();
                settings.fresh = false;
            }
            if field.starts_with('-') {
                field.remove(0);
            } else {
                field.insert(0, '-');
            }
            settings.pending = true;
            return true;
        }
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
    pub(super) fn timing_heading(&self) -> Arc<str> {
        let kind = self
            .timing
            .as_ref()
            .map(|s| s.draft.kind())
            .unwrap_or(EventKind::Bpm);
        i18n::tr_fmt(
            "Edition",
            "TimingType",
            &[("type", &i18n::tr("Edition", kind.label()))],
        )
    }
    fn timing_type(&mut self) {
        let Some(settings) = &self.timing else {
            return;
        };
        if settings.pending {
            self.status = i18n::tr("Edition", settings.draft.key("TimingInputPending"));
            return;
        }
        let kind = settings.draft.kind().step(self.shift);
        let row = settings.row().unwrap_or(self.workspace.row());
        let same_family = matches!(
            (&settings.draft, kind),
            (Draft::Pauses { .. }, EventKind::Stop | EventKind::Delay)
                | (Draft::Regions { .. }, EventKind::Warp | EventKind::Fake)
        );
        if same_family {
            let mut settings = self.timing.take().unwrap();
            settings.draft.select_type(kind);
            self.timing = Some(Settings::new(settings.draft, row));
            self.status = i18n::tr("Edition", "TimingHelp");
            return;
        }
        if settings.draft.has_changes() {
            self.status = i18n::tr("Edition", "TimingTypePending");
            return;
        }
        match Draft::capture(&self.workspace, settings.draft.scope(), kind) {
            Ok(draft) => {
                self.timing = Some(Settings::new(draft, row));
                self.status = i18n::tr("Edition", "TimingHelp");
            }
            Err(_)
                if kind.is_region()
                    && self.workspace.file().session.document().format
                        == deadsync_editor::document::Format::Sm =>
            {
                self.status = i18n::tr("Edition", "RegionSscRequired")
            }
            Err(error) => self.timing_error(error),
        }
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
        match Draft::capture(&self.workspace, scope, settings.draft.kind()) {
            Ok(draft) => self.timing = Some(Settings::new(draft, self.workspace.row())),
            Err(_)
                if scope == Scope::Chart
                    && !deadsync_editor::timing_edit::supports_chart_timing(
                        self.workspace.file().session.document(),
                    ) =>
            {
                self.status = i18n::tr("Edition", "TimingSharedOnly")
            }
            Err(error) => self.timing_error(error),
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
        let bpm = match settings.value.parse::<f64>() {
            Ok(value) if settings.draft.valid_value_at(row, value) => value,
            _ => {
                self.status = i18n::tr("Edition", settings.draft.key("TimingInvalidBpm"));
                return false;
            }
        };
        match settings.draft.set(row, bpm) {
            Ok(_) => {
                settings.beat = beat_text(row);
                settings.value = settings.draft.value(row).to_string();
                settings.node = Some(row);
                settings.pending = false;
                settings.fresh = true;
                self.status = i18n::tr("Edition", settings.draft.key("TimingStaged"));
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
        if row == 0 && settings.draft.kind() == EventKind::Bpm {
            self.status = i18n::tr("Edition", "TimingInitialImmutable");
            return;
        }
        match settings.draft.remove(row) {
            Ok(changed) => {
                settings.node = None;
                settings.value = settings.draft.value(row).to_string();
                settings.pending = false;
                settings.fresh = true;
                self.status = i18n::tr(
                    "Edition",
                    settings.draft.key(if changed {
                        "TimingRemoved"
                    } else {
                        "TimingMissing"
                    }),
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
            self.status = i18n::tr("Edition", settings.draft.key("TimingMissing"));
            return;
        };
        let to = match settings.row() {
            Ok(row) => row,
            Err(key) => {
                self.status = i18n::tr("Edition", key);
                return;
            }
        };
        if from == 0 && to != 0 && settings.draft.kind() == EventKind::Bpm {
            self.status = i18n::tr("Edition", "TimingInitialImmutable");
            return;
        }
        if from != to && settings.draft.node(to).is_some() {
            self.status = i18n::tr("Edition", settings.draft.key("TimingCollision"));
            return;
        }
        if !settings.draft.valid_destination(from, to) {
            self.status = i18n::tr("Edition", "RegionInvalidLength");
            return;
        }
        if !settings
            .value
            .parse::<f64>()
            .ok()
            .is_some_and(|value| settings.draft.same_value(from, value))
        {
            self.status = i18n::tr("Edition", settings.draft.key("TimingMoveValue"));
            return;
        }
        match settings.draft.move_node(from, to) {
            Ok(_) => {
                settings.node = Some(to);
                settings.beat = beat_text(to);
                settings.value = settings.draft.value(to).to_string();
                settings.pending = false;
                settings.fresh = true;
                self.status = i18n::tr("Edition", settings.draft.key("TimingMoved"));
            }
            Err(error) => self.timing_error(error),
        }
    }
    fn timing_navigate(&mut self, forward: bool) {
        let Some(settings) = &mut self.timing else {
            return;
        };
        if settings.pending {
            self.status = i18n::tr("Edition", settings.draft.key("TimingInputPending"));
            return;
        }
        let row = match settings.row() {
            Ok(row) => row,
            Err(key) => {
                self.status = i18n::tr("Edition", key);
                return;
            }
        };
        if let Some((row, value)) = settings.draft.neighbor(row, forward) {
            settings.node = Some(row);
            settings.beat = beat_text(row);
            settings.value = value.to_string();
            settings.fresh = true;
            if let Err(error) = self.workspace.execute(Command::Seek(row)) {
                self.timing_error(error);
            }
            self.refresh();
        } else {
            self.status = i18n::tr("Edition", settings.draft.key("TimingMissing"));
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
        match settings.draft.apply(&mut self.workspace) {
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
        let key = settings.draft.key(key);
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
            2 => i18n::tr_fmt(
                "Edition",
                key,
                &[
                    ("bpm", &settings.value),
                    ("seconds", &settings.value),
                    ("length", &settings.value),
                ],
            ),
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
                &[("count", &settings.draft.count().to_string())],
            ),
            _ => i18n::tr("Edition", key),
        }
    }
}
