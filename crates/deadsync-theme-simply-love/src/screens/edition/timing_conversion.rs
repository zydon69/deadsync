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
    source_callback:bool,
    plan: Option<Plan>,
    reviewed: BTreeSet<u64>,
    collision: usize,
}
impl State {
    pub(super) fn command_selected_timing_conversion(&mut self,direction:Direction) {
        if self.event_selection.as_ref().is_none_or(|settings|settings.selection.selected_count()==0) {self.status=i18n::tr("Edition","NoteOperationMissingSelection");return;}
        let from_picker=matches!(self.panel,Some(Panel::EventSelection));
        match self.prepare_checked_timing_conversion(direction) {
            Ok(plan)=>{self.timing_conversion=Settings{reverse:direction==Direction::NegativeStopsToWarps,scope:2,source_callback:true,from_picker,plan:Some(plan),..Default::default()};self.panel=Some(Panel::TimingConversion(3));self.status=i18n::tr("Edition","TimingConversionPrepared");}
            Err(error)=>self.status=i18n::tr("Edition",timing_refusal_key(error.kind())),
        }
    }
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
        let source_callback=self.timing_conversion.source_callback;
        self.timing_conversion = Default::default();
        self.panel = if source_callback && !from_picker {None}else{Some(if from_picker { Panel::EventSelection } else { Panel::Timing(0) })};
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
                            self.sync_event_selection_history();
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

#[cfg(test)]
mod selected_callback_tests {
    use super::*;
    use deadlib_platform::input::RawKeyboardEvent;
    use deadsync_editor::{
        event_clipboard::{Event, TimingKind},
        key_binding_preferences::{Action, Preferences},
        workspace::Command,
    };
    use std::time::Instant;
    fn init(fr: bool) {
        if fr {
            crate::i18n::init(deadsync_assets::language::load_for_tests("fr"));
        } else {
            crate::i18n::init_for_tests();
        }
        deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
            854.0, 480.0,
        ));
    }
    fn key(s: &mut State, c: KeyCode) {
        for pressed in [true, false] {
            s.raw_key(&RawKeyboardEvent {
                code: c,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn state(source: &[u8]) -> (tempfile::TempDir, State) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.ssc");
        std::fs::write(&path, source).unwrap();
        let mut s = State::open(&path, 0).unwrap();
        let mut p = Preferences::default();
        p.commands.insert(
            Action::ConvertSelectedWarpsToNegativeStops.id().into(),
            vec![vec!["F15".into()]],
        );
        p.commands.insert(
            Action::ConvertSelectedNegativeStopsToWarps.id().into(),
            vec![vec!["F16".into()]],
        );
        s.configure_key_binding_preferences(std::sync::Arc::new(p));
        (dir, s)
    }
    fn bytes(s: &State) -> Vec<u8> {
        s.workspace.file().session.serialize().unwrap()
    }
    fn pick(s: &mut State, events: &[Event]) {
        s.command_selection_preset(
            deadsync_editor::event_clipboard::presets::Preset::NotesAndMiscellaneous,
            false,
        );
        let selection = &mut s.event_selection.as_mut().unwrap().selection;
        selection.select_all(false);
        for e in events {
            let i = selection.events().iter().position(|v| v == e).unwrap();
            selection.set_selected(i, true).unwrap();
        }
        s.panel = None;
    }
    fn selected(s: &State) -> Vec<Event> {
        s.event_selection
            .as_ref()
            .map(|v| v.selection.selected_events().collect())
            .unwrap_or_default()
    }
    fn timing(row: u64, kind: TimingKind) -> Event {
        Event::Timing { row, kind }
    }
    const WARPS:&[u8]=b"#VERSION:0.83;#TITLE:Mixed;#BPMS:0=60;#WARPS:1=2,2=2,5=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n1000\n1000\n1000;";
    #[test]
    fn configured_callbacks_use_global_mixed_selection_and_review_then_restore_source_selection_on_undo_redo()
     {
        for fr in [false, true] {
            init(fr);
            let (_d, mut s) = state(WARPS);
            s.command_selection_preset(
                deadsync_editor::event_clipboard::presets::Preset::NotesAndMiscellaneous,
                false,
            );
            let note = s
                .event_selection
                .as_ref()
                .unwrap()
                .selection
                .events()
                .iter()
                .find(|e| matches!(e, Event::Note(_)))
                .copied()
                .unwrap();
            pick(
                &mut s,
                &[
                    timing(96, TimingKind::Warp),
                    note,
                    timing(48, TimingKind::Warp),
                ],
            );
            let before = bytes(&s);
            key(&mut s, KeyCode::F15);
            assert!(matches!(s.panel, Some(Panel::TimingConversion(3))));
            assert_eq!(s.timing_conversion.scope, 2);
            assert_eq!(
                s.timing_conversion
                    .plan
                    .as_ref()
                    .unwrap()
                    .report()
                    .selection_after,
                vec![96, 48]
            );
            assert_eq!(bytes(&s), before);
            key(&mut s, KeyCode::ArrowDown);
            key(&mut s, KeyCode::Enter);
            assert!(s.panel.is_none());
            let after = bytes(&s);
            assert_ne!(after, before);
            assert_eq!(
                selected(&s),
                vec![timing(96, TimingKind::Stop), timing(48, TimingKind::Stop)]
            );
            s.command(Command::Undo);
            assert_eq!(bytes(&s), before);
            assert_eq!(
                selected(&s),
                vec![timing(96, TimingKind::Warp), timing(48, TimingKind::Warp)]
            );
            s.command(Command::Redo);
            assert_eq!(bytes(&s), after);
            assert_eq!(
                selected(&s),
                vec![timing(96, TimingKind::Stop), timing(48, TimingKind::Stop)]
            );
        }
    }
    #[test]
    fn missing_checked_selection_never_falls_back_to_range_or_all() {
        for fr in [false, true] {
            init(fr);
            let (_d, mut s) = state(WARPS);
            s.workspace.execute(Command::ToggleSelection).unwrap();
            s.workspace.execute(Command::Move(4)).unwrap();
            assert!(s.workspace.selection().is_some());
            let before = bytes(&s);
            for k in [KeyCode::F15, KeyCode::F16] {
                key(&mut s, k);
                assert!(s.panel.is_none());
                assert_eq!(
                    s.status,
                    crate::i18n::tr("Edition", "NoteOperationMissingSelection")
                );
                assert_eq!(bytes(&s), before);
            }
        }
    }
    #[test]
    fn reverse_callback_filters_positive_stops_and_collision_review_is_explicit() {
        for fr in [false, true] {
            init(fr);
            let source=b"#VERSION:0.83;#BPMS:0=60;#STOPS:1=-2,1.5=1,2=-1;#WARPS:2=1;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
            let (_d, mut s) = state(source);
            pick(
                &mut s,
                &[
                    timing(96, TimingKind::Stop),
                    timing(72, TimingKind::Stop),
                    timing(48, TimingKind::Stop),
                ],
            );
            let before = bytes(&s);
            key(&mut s, KeyCode::F16);
            assert!(s.timing_conversion.reverse);
            assert_eq!(
                s.timing_conversion
                    .plan
                    .as_ref()
                    .unwrap()
                    .report()
                    .deleted_on_collision,
                BTreeSet::from([96])
            );
            key(&mut s, KeyCode::ArrowDown);
            key(&mut s, KeyCode::Enter);
            assert_eq!(
                s.status,
                crate::i18n::tr("Edition", "TimingConversionMissingReview")
            );
            assert_eq!(bytes(&s), before);
            key(&mut s, KeyCode::ArrowUp);
            key(&mut s, KeyCode::Enter);
            key(&mut s, KeyCode::ArrowDown);
            key(&mut s, KeyCode::Enter);
            assert_eq!(selected(&s), vec![timing(48, TimingKind::Warp)]);
            s.command(Command::Undo);
            assert_eq!(
                selected(&s),
                vec![timing(96, TimingKind::Stop), timing(48, TimingKind::Stop)]
            );
            assert_eq!(bytes(&s), before);
        }
    }
    #[test]
    fn undo_when_focus_is_another_chart_clears_selection_without_changing_focus() {
        for fr in [false, true] {
            init(fr);
            let mut source = WARPS.to_vec();
            source.extend_from_slice(b"#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;");
            let (_d, mut s) = state(&source);
            pick(&mut s, &[timing(48, TimingKind::Warp)]);
            key(&mut s, KeyCode::F15);
            key(&mut s, KeyCode::ArrowDown);
            key(&mut s, KeyCode::Enter);
            assert!(!selected(&s).is_empty());
            s.command(Command::SelectChart(1));
            s.command(Command::Undo);
            assert_eq!(s.workspace.chart(), 1);
            assert!(selected(&s).is_empty());
        }
    }
}
