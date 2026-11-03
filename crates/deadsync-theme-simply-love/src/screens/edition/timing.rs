//! Optional timing inspector. Drafts remain private until one explicit acceptance.
use super::*;
mod draft;
mod migration;
use deadsync_editor::timing_edit::Scope;
use draft::{Draft, EventKind};
use unicode_segmentation::UnicodeSegmentation;

pub(super) struct Settings {
    draft: Draft,
    beat: String,
    pub(super) value: String,
    node: Option<u64>,
    pub(super) fresh: bool,
    pub(super) pending: bool,
}
impl Settings {
    pub(super) fn has_changes(&self) -> bool {
        self.pending || self.draft.has_changes()
    }
    pub(super) fn is_label(&self) -> bool {
        self.draft.kind() == EventKind::Label
    }
    fn new(draft: Draft, row: u64) -> Self {
        let value = draft.value(row);
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
        if self.chart_rating_input.is_some()
            || self
                .chart_properties
                .as_ref()
                .is_some_and(|draft| draft.has_changes())
        {
            self.status = i18n::tr("Edition", "TimingPropertiesPending");
            return;
        }
        let source = self.workspace.file().session.document();
        if deadsync_editor::timing_edit::migration::needed(source) {
            self.open_timing_migration();
            return;
        }
        let scope = if deadsync_editor::timing_edit::supports_resident_chart_timing(source) {
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
        self.status = i18n::tr_fmt(
            "Edition",
            "Error",
            &[("message", &crate::i18n::editor_error(&error))],
        );
    }
    fn timing_attacks(&mut self) {
        if self
            .timing
            .as_ref()
            .is_some_and(|settings| settings.pending || settings.draft.has_changes())
        {
            self.status = i18n::tr("Edition", "AttacksTimingPending");
            return;
        }
        let scope = if deadsync_editor::attacks::supports_resident_chart_attacks(
            self.workspace.file().session.document(),
        ) {
            self.timing
                .as_ref()
                .map_or(Scope::Song, |settings| settings.draft.scope())
        } else {
            Scope::Song
        };
        self.open_attacks(scope);
    }
    pub(super) fn timing_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::F5 {
            self.open_quick_add();
            return;
        }
        if code == KeyCode::F7 {
            if self
                .timing
                .as_ref()
                .is_some_and(|settings| settings.pending || settings.draft.has_changes())
            {
                self.status = i18n::tr("Edition", "TimingConversionPending");
            } else {
                self.open_timing_conversion();
            }
            return;
        }
        if code == KeyCode::F6 {
            self.timing_attacks();
            return;
        }
        if code == KeyCode::Tab {
            self.timing_type();
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown) {
            self.panel = Some(Panel::Timing(
                (index + if code == KeyCode::ArrowUp { 10 } else { 1 }) % 11,
            ));
            if let Some(settings) = &mut self.timing {
                settings.fresh = true;
            }
            return;
        }
        if matches!(index, 1 | 2) && self.timing_input(index, code) {
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
                                deadsync_editor::workspace::snap_edit_rows(self.workspace.snap())
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
                    if settings.draft.kind() == EventKind::Label {
                        return;
                    }
                    if settings.draft.kind() == EventKind::Speed {
                        use deadsync_editor::timing_edit::speed::{Period, Speed};
                        match Speed::parse(&settings.value) {
                            Ok(mut value) => {
                                if self.shift {
                                    value.period = match value.period {
                                        Period::Rows(rows) => Period::Rows(if forward {
                                            rows.saturating_add(1)
                                        } else {
                                            rows.saturating_sub(1).max(0)
                                        }),
                                        Period::Seconds(seconds) => Period::Seconds(
                                            (seconds + if forward { 0.001 } else { -0.001 })
                                                .max(0.0),
                                        ),
                                    };
                                } else {
                                    value.rate =
                                        (value.rate + if forward { 0.01 } else { -0.01 }).max(0.0);
                                }
                                settings.value = value.to_string();
                                settings.pending = true;
                                settings.fresh = true;
                            }
                            Err(_) => self.status = i18n::tr("Edition", "SpeedInvalid"),
                        }
                        return;
                    }
                    if settings.draft.kind() == EventKind::Combo {
                        match deadsync_editor::timing_edit::combo::Multipliers::parse(
                            &settings.value,
                        ) {
                            Ok(mut value) => {
                                let part = if self.shift {
                                    &mut value.miss
                                } else {
                                    &mut value.hit
                                };
                                *part = if forward {
                                    part.saturating_add(1)
                                } else {
                                    part.saturating_sub(1).max(0)
                                };
                                settings.value = value.to_string();
                                settings.pending = true;
                                settings.fresh = true;
                            }
                            Err(_) => self.status = i18n::tr("Edition", "ComboInvalid"),
                        }
                        return;
                    }
                    if settings.draft.kind() == EventKind::Signature {
                        match deadsync_editor::timing_edit::signature::Signature::parse(
                            &settings.value,
                        ) {
                            Ok(mut signature) => {
                                if self.shift {
                                    let denominators = [1, 2, 4, 8, 16, 32];
                                    let index = denominators
                                        .iter()
                                        .position(|&value| value == signature.denominator)
                                        .expect("Validated signature denominator");
                                    signature.denominator = denominators[if forward {
                                        (index + 1).min(5)
                                    } else {
                                        index.saturating_sub(1)
                                    }];
                                } else {
                                    signature.numerator = if forward {
                                        signature.numerator.saturating_add(1).min(i32::MAX as u32)
                                    } else {
                                        signature.numerator.saturating_sub(1).max(1)
                                    };
                                }
                                settings.value = signature.to_string();
                                settings.pending = true;
                                settings.fresh = true;
                            }
                            Err(_) => self.status = i18n::tr("Edition", "SignatureInvalid"),
                        }
                        return;
                    }
                    match settings.value.parse::<f64>() {
                        Ok(value) if value.is_finite() => {
                            let delta = if settings.draft.kind() == EventKind::Tick {
                                1.0
                            } else if settings.draft.kind() == EventKind::Bpm {
                                if self.shift { 0.1 } else { 1.0 }
                            } else if settings.draft.kind().is_region() {
                                if self.shift {
                                    1.0 / 48.0
                                } else {
                                    deadsync_editor::workspace::snap_edit_rows(
                                        self.workspace.snap(),
                                    ) as f64
                                        / 48.0
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
                                EventKind::Tick => value.round().clamp(0.0, i32::MAX as f64),
                                EventKind::Stop | EventKind::Scroll => value,
                                EventKind::Signature
                                | EventKind::Combo
                                | EventKind::Speed
                                | EventKind::Label => {
                                    return;
                                }
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
                10 => self.timing_attacks(),
                9 => {
                    self.timing = None;
                    self.panel = None;
                    self.status = i18n::tr("Edition", "TimingDiscarded");
                }
                _ => {}
            }
        }
    }
    fn timing_input(&mut self, index: usize, code: KeyCode) -> bool {
        if self.control {
            return false;
        }
        let Some(settings) = &mut self.timing else {
            return false;
        };
        if index == 2 && settings.draft.kind() == EventKind::Label {
            match code {
                KeyCode::Backspace => {
                    let end = settings
                        .value
                        .grapheme_indices(true)
                        .next_back()
                        .map_or(0, |(index, _)| index);
                    settings.value.truncate(end);
                }
                KeyCode::Delete => settings.value.clear(),
                _ => return false,
            }
            settings.fresh = false;
            settings.pending = true;
            return true;
        }
        let signature = index == 2 && settings.draft.kind() == EventKind::Signature;
        let combo = index == 2 && settings.draft.kind() == EventKind::Combo;
        let speed = index == 2 && settings.draft.kind() == EventKind::Speed;
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
            KeyCode::Slash | KeyCode::NumpadDivide if signature || combo || speed => '/',
            KeyCode::Period if self.shift && (signature || combo || speed) => '/',
            KeyCode::KeyX | KeyCode::NumpadMultiply if combo || speed => 'x',
            KeyCode::KeyR if speed => 'r',
            KeyCode::KeyO if speed => 'o',
            KeyCode::KeyW | KeyCode::KeyZ if speed => 'w',
            KeyCode::KeyS if speed => 's',
            KeyCode::Period | KeyCode::Comma | KeyCode::NumpadDecimal if !signature && !combo => {
                '.'
            }
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
        let point_exists = if speed {
            field
                .rsplit('/')
                .next()
                .is_some_and(|part| part.contains('.'))
        } else {
            field.contains('.')
        };
        let limit = if speed { 1024 } else { 24 };
        if field.len() < limit
            && (digit != '.' || !point_exists)
            && (digit != '/' || !field.contains('/'))
        {
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
                self.status = i18n::tr(
                    "Edition",
                    if kind == EventKind::Signature {
                        "SignatureHelp"
                    } else if kind == EventKind::Label {
                        "LabelHelp"
                    } else if kind == EventKind::Speed {
                        "SpeedHelp"
                    } else if kind == EventKind::Combo {
                        "ComboHelp"
                    } else if kind == EventKind::Tick {
                        "TickHelp"
                    } else if kind == EventKind::Scroll {
                        "ScrollHelp"
                    } else {
                        "TimingHelp"
                    },
                );
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
                    && !deadsync_editor::timing_edit::supports_resident_chart_timing(
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
        if !settings.draft.valid_value_at(row, &settings.value) {
            self.status = i18n::tr("Edition", settings.draft.key("TimingInvalidBpm"));
            return false;
        }
        match settings.draft.set(row, &settings.value) {
            Ok(_) => {
                settings.beat = beat_text(row);
                settings.value = settings.draft.value(row);
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
        if row == 0 && matches!(settings.draft.kind(), EventKind::Bpm | EventKind::Signature) {
            self.status = i18n::tr(
                "Edition",
                if settings.draft.kind() == EventKind::Signature {
                    "SignatureInitialImmutable"
                } else {
                    "TimingInitialImmutable"
                },
            );
            return;
        }
        match settings.draft.remove(row) {
            Ok(changed) => {
                settings.node = None;
                settings.value = settings.draft.value(row);
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
        if from == 0
            && to != 0
            && matches!(settings.draft.kind(), EventKind::Bpm | EventKind::Signature)
        {
            self.status = i18n::tr(
                "Edition",
                if settings.draft.kind() == EventKind::Signature {
                    "SignatureInitialImmutable"
                } else {
                    "TimingInitialImmutable"
                },
            );
            return;
        }
        if from != to && settings.draft.node(to).is_some() {
            self.status = i18n::tr("Edition", settings.draft.key("TimingCollision"));
            return;
        }
        if !settings.draft.valid_destination(from, to) {
            self.status = i18n::tr("Edition", settings.draft.key("TimingInvalidBpm"));
            return;
        }
        if !settings.draft.same_value(from, &settings.value) {
            self.status = i18n::tr("Edition", settings.draft.key("TimingMoveValue"));
            return;
        }
        match settings.draft.move_node(from, to) {
            Ok(_) => {
                settings.node = Some(to);
                settings.beat = beat_text(to);
                settings.value = settings.draft.value(to);
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
            settings.value = value;
            settings.fresh = true;
            self.navigate_exact_position(row as f64, true);
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
        let Some(settings) = self.timing.take() else {
            return;
        };
        let result = (|| {
            let before = self.capture_timing_presentation()?;
            let plan = settings.draft.live_edit().prepare(
                &self.workspace.file().session,
                self.workspace.chart(),
                before.anchor(),
            )?;
            let application = self.workspace.apply_timing_live_plan(plan)?;
            let changed = application.changed();
            if changed {
                self.finish_timing_presentation(before, application.into_position())?;
            }
            Ok::<_, deadsync_editor::Error>(changed)
        })();
        self.timing = Some(settings);
        match result {
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
        if index == 2 && settings.draft.kind() == EventKind::Label && !self.label_preedit.is_empty()
        {
            let value = if settings.fresh {
                self.label_preedit.clone()
            } else {
                format!("{}{}", settings.value, self.label_preedit)
            };
            return i18n::tr_fmt("Edition", key, &[("bpm", &value)]);
        }
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
