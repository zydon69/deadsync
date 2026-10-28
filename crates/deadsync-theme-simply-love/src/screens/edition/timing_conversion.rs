//! Detached review for source Warp / negative Stop actions. F7 in timing opens it.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::timing_edit::conversion::{Direction, Plan};
use std::collections::BTreeSet;
#[derive(Default)]
pub(super) struct Settings {
    reverse: bool,
    scope: usize,
    from_picker: bool,
    plan: Option<Plan>,
    reviewed: BTreeSet<u64>,
    collision: usize,
}
impl State {
    pub(super) fn open_timing_conversion(&mut self) {
        self.timing_conversion = Default::default();
        self.panel = Some(Panel::TimingConversion(0));
        self.status = i18n::tr("Edition", "TimingConversionHelp");
    }
    pub(super) fn open_checked_timing_conversion(&mut self) {
        self.open_timing_conversion();
        self.timing_conversion.scope = 2;
        self.timing_conversion.from_picker = true;
    }
    fn close_timing_conversion(&mut self) {
        let from_picker = self.timing_conversion.from_picker;
        self.timing_conversion = Default::default();
        self.panel = Some(if from_picker { Panel::EventSelection } else { Panel::Timing(0) });
    }
    pub(super) fn timing_conversion_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.close_timing_conversion(),
            KeyCode::ArrowUp => self.panel = Some(Panel::TimingConversion((index + 5) % 6)),
            KeyCode::ArrowDown | KeyCode::Tab => self.panel = Some(Panel::TimingConversion((index + 1) % 6)),
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index < 2 => {
                let settings = &mut self.timing_conversion;
                if index == 0 { settings.reverse = !settings.reverse; } else {
                    let step = if code == KeyCode::ArrowLeft { 2 } else { 1 };
                    settings.scope = (settings.scope + step) % 3;
                    if settings.scope == 2 && self.event_selection.is_none() { settings.scope = (settings.scope + step) % 3; }
                }
                settings.plan = None;
                settings.reviewed.clear();
                settings.collision = 0;
            }
            KeyCode::Enter if index == 2 => {
                let mut settings = std::mem::take(&mut self.timing_conversion);
                let direction = if settings.reverse { Direction::NegativeStopsToWarps } else { Direction::WarpsToNegativeStops };
                let result = if settings.scope == 2 {
                    self.prepare_checked_timing_conversion(direction)
                } else { self.workspace.prepare_timing_conversion_detailed(direction, settings.scope == 1) };
                match result {
                    Ok(plan) => {
                        settings.plan = Some(plan);
                        settings.reviewed.clear();
                        settings.collision = 0;
                        self.status = i18n::tr("Edition", "TimingConversionPrepared");
                    }
                    Err(error) => self.status = i18n::tr("Edition", timing_refusal_key(error.kind())),
                }
                self.timing_conversion = settings;
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index == 3 => {
                let settings = &mut self.timing_conversion;
                if let Some(plan) = &settings.plan {
                    let rows = &plan.report().deleted_on_collision;
                    if let Some(&row) = rows.iter().nth(settings.collision) {
                        if code == KeyCode::Enter { settings.reviewed.insert(row); }
                        let count = rows.len();
                        settings.collision = (settings.collision + if code == KeyCode::ArrowLeft { count - 1 } else { 1 }) % count;
                    }
                }
            }
            KeyCode::Enter if index == 4 => {
                let settings = &self.timing_conversion;
                if let Some(plan) = &settings.plan {
                    match self.workspace.apply_timing_conversion_detailed(plan, &settings.reviewed) {
                        Ok(changed) => {
                            self.timing = None;
                            self.event_selection = None;
                            self.timing_conversion = Default::default();
                            self.panel = None;
                            self.status = i18n::tr("Edition", if changed { "TimingConversionApplied" } else { "TimingConversionNoChange" });
                            self.refresh();
                        }
                        Err(error) => self.status = i18n::tr("Edition", timing_refusal_key(error.kind())),
                    }
                } else { self.status = i18n::tr("Edition", "TimingConversionPrepareFirst"); }
            }
            KeyCode::Enter => self.close_timing_conversion(),
            _ => {}
        }
    }
    pub(super) fn timing_conversion_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let settings = &self.timing_conversion;
        let value = match index {
            0 => i18n::tr("Edition", if settings.reverse { "TimingConversionStopToWarp" } else { "TimingConversionWarpToStop" }).to_string(),
            1 => i18n::tr("Edition", ["TimingConversionSelection", "TimingConversionAll", "TimingConversionChecked"][settings.scope]).to_string(),
            2 => settings.plan.as_ref().map(|p| format!("{} / {} / {}", p.report().converted, p.report().deleted_on_collision.len(), p.report().overlaps.len())).unwrap_or_else(|| i18n::tr("Edition", "TimingConversionNotPrepared").to_string()),
            3 => settings.plan.as_ref().and_then(|p| p.report().deleted_on_collision.iter().nth(settings.collision).map(|row| format!("{} / {}: {} ({}/{})", settings.collision + 1, p.report().deleted_on_collision.len(), row, settings.reviewed.len(), p.report().deleted_on_collision.len()))).unwrap_or_else(|| i18n::tr("Edition", "TimingConversionNoCollision").to_string()),
            _ => return i18n::tr("Edition", key),
        };
        i18n::tr_fmt("Edition", key, &[("value", &value)])
    }
}

fn timing_refusal_key(reason: deadsync_editor::timing_edit::conversion::Refusal) -> &'static str {
    use deadsync_editor::timing_edit::conversion::Refusal;
    match reason {
        Refusal::ZeroDurationStop => "TimingConversionZeroStop",
        Refusal::TargetFormat => "TimingConversionRequiresSsc",
        Refusal::CollisionReview => "TimingConversionMissingReview",
        Refusal::Stale => "TimingConversionStale",
        Refusal::Invalid => "TimingConversionFailed",
    }
}
