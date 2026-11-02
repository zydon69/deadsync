//! Optional compact source quick-add panel; all edits use the shared typed drafts.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::quick_add::{Kind, Plan, Refusal};
const KINDS: [Kind; 13] = [
    Kind::Tempo,
    Kind::Speed,
    Kind::Scroll,
    Kind::Stop,
    Kind::Delay,
    Kind::Warp,
    Kind::Fake,
    Kind::Ticks,
    Kind::Combo,
    Kind::Signature,
    Kind::Label,
    Kind::Attack,
    Kind::Pattern,
];
const KEYS: [&str; 13] = [
    "QuickAddTempo",
    "QuickAddSpeed",
    "QuickAddScroll",
    "QuickAddStop",
    "QuickAddDelay",
    "QuickAddWarp",
    "QuickAddFake",
    "QuickAddTicks",
    "QuickAddCombo",
    "QuickAddSignature",
    "QuickAddLabel",
    "QuickAddAttack",
    "QuickAddPattern",
];
#[derive(Default)]
pub(super) struct Settings {
    kind: usize,
    plan: Option<Plan>,
}
impl State {
    pub(super) fn command_quick_add_review(&mut self, kind: Kind) {
        self.open_quick_add();
        if !matches!(self.panel, Some(Panel::QuickAdd(_))) {
            return;
        }
        self.quick_add.kind = KINDS
            .iter()
            .position(|candidate| *candidate == kind)
            .expect("all quick-add kinds listed");
        self.prepare_quick_add_review();
        self.panel = Some(Panel::QuickAdd(2));
    }

    pub(super) fn open_quick_add(&mut self) {
        if self
            .timing
            .as_ref()
            .is_some_and(|settings| settings.has_changes())
        {
            self.status = i18n::tr("Edition", "TimingConversionPending");
            return;
        }
        self.quick_add = Default::default();
        self.panel = Some(Panel::QuickAdd(0));
        self.status = i18n::tr("Edition", "QuickAddHelp");
    }
    pub(super) fn quick_add_key(&mut self, index: usize, code: KeyCode) {
        match code {
            KeyCode::Escape => self.close_quick_add(),
            KeyCode::ArrowUp => self.panel = Some(Panel::QuickAdd((index + 4) % 5)),
            KeyCode::ArrowDown | KeyCode::Tab => {
                self.panel = Some(Panel::QuickAdd((index + 1) % 5))
            }
            KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter if index == 0 => {
                self.quick_add.kind =
                    (self.quick_add.kind + if code == KeyCode::ArrowLeft { 12 } else { 1 }) % 13;
                self.quick_add.plan = None;
            }
            KeyCode::Enter if index == 2 => {
                self.prepare_quick_add_review();
            }
            KeyCode::Enter if index == 3 => {
                let Some(plan) = &self.quick_add.plan else {
                    self.status = i18n::tr("Edition", "NoteOperationPrepareFirst");
                    return;
                };
                let anchor = self.capture_history_row_anchor();
                match self.workspace.apply_quick_add(plan) {
                    Ok(_) => {
                        if let Some(anchor) = anchor {
                            if let Err(error) = self.restore_history_row_anchor(anchor) {
                                self.status = i18n::tr_fmt(
                                    "Edition",
                                    "Error",
                                    &[("message", &i18n::editor_error(&error))],
                                );
                                return;
                            }
                        }
                        self.quick_add = Default::default();
                        self.timing = None;
                        self.panel = None;
                        self.status = i18n::tr("Edition", "QuickAddApplied");
                        self.refresh();
                    }
                    Err(error) => self.status = i18n::tr("Edition", refusal(error.reason)),
                }
            }
            KeyCode::Enter if index == 4 => self.close_quick_add(),
            _ => {}
        }
    }
    fn prepare_quick_add_review(&mut self) {
        let position = match self.capture_song_property_presentation() {
            Ok(value) => value.row(),
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &i18n::editor_error(&error))],
                );
                return;
            }
        };
        match deadsync_editor::quick_add::Plan::prepare(
            &self.workspace.file().session,
            self.workspace.chart(),
            KINDS[self.quick_add.kind],
            position,
        ) {
            Ok(plan) => {
                self.quick_add.plan = Some(plan);
                self.status = i18n::tr("Edition", "QuickAddPrepared");
            }
            Err(error) => {
                self.quick_add.plan = None;
                self.status = i18n::tr("Edition", refusal(error.reason));
            }
        }
    }
    fn close_quick_add(&mut self) {
        self.quick_add = Default::default();
        self.panel = Some(Panel::Timing(0));
    }
    pub(super) fn quick_add_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let text = match index {
            0 => i18n::tr("Edition", key)
                .replace("{value}", &i18n::tr("Edition", KEYS[self.quick_add.kind])),
            1 => i18n::tr("Edition", key).replace("{row}", &self.workspace.row().to_string()),
            _ => i18n::tr("Edition", key).to_string(),
        };
        text.into()
    }
}
fn refusal(reason: Refusal) -> &'static str {
    match reason {
        Refusal::Occupied => "QuickAddOccupied",
        Refusal::InvalidPosition => "QuickAddInvalidPosition",
        Refusal::UnsupportedLayout => "QuickAddUnsupportedLayout",
        Refusal::Stale => "QuickAddStale",
        Refusal::Invalid => "QuickAddRefused",
    }
}

#[cfg(test)]
mod fractional_focus_tests {
    use super::*;
    #[test]
    fn source_quick_add_uses_live_fractional_focus_with_half_ties_down() {
        crate::i18n::init_for_tests();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("fractional.ssc");
        std::fs::write(
            &path,
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;",
        )
        .unwrap();
        for (row, expected) in [(160.5, 160_u64), (160.75, 161_u64)] {
            let mut state = State::open(&path, 0).unwrap();
            state.workspace.seek_preserving_selection(160).unwrap();
            state.focus_position = Some(
                deadsync_editor::chart_focus::Position::at_fractional_row(
                    &state.workspace.file().session,
                    0,
                    row,
                )
                .unwrap(),
            );
            state.refresh();
            state.command_quick_add_review(Kind::Warp);
            assert!(matches!(state.panel, Some(Panel::QuickAdd(2))));
            state.quick_add_key(3, KeyCode::Enter);
            assert_eq!(
                state
                    .workspace
                    .file()
                    .session
                    .document()
                    .text(Some(0), "WARPS"),
                Some(format!("{}=1", expected as f64 / 48.0))
            );
            assert_eq!(state.focus_position.as_ref().unwrap().row(), row);
        }
    }
}

#[cfg(test)]
mod resident_sm_tests {
    use super::*;
    use deadsync_editor::{
        document::Format,
        event_clipboard::{Event, TimingKind},
        key_binding_preferences::{Action, Preferences},
        timing_edit::{Scope, region},
        workspace::Command,
    };
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&deadlib_platform::input::RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    #[test]
    fn physical_source_add_actions_author_each_sm_event_without_format_promotion_en_fr() {
        for locale in ["en", "fr"] {
            crate::i18n::init_for_tests();
            crate::i18n::set_locale(deadsync_assets::language::load_for_tests(locale));
            for (action, kind) in [
                (Action::AddEventWarp, TimingKind::Warp),
                (Action::AddEventFakeRegion, TimingKind::Fake),
                (Action::AddEventScrollRate, TimingKind::Scroll),
                (Action::AddEventInterpolatedScrollRate, TimingKind::Speed),
                (Action::AddEventComboMultipliers, TimingKind::Combo),
                (Action::AddEventLabel, TimingKind::Label),
            ] {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("source.sm");
                std::fs::write(&path,b"#BPMS:0=120;#WARPS:1=100;#NOTES:dance-single::Hard:9::1000,0100,0010;#NOTES:dance-single::Easy:2::0001,0010,0100;").unwrap();
                let mut state = State::open(&path, 0).unwrap();
                state.panel = None;
                state.workspace.execute(Command::Seek(384)).unwrap();
                let mut preferences = Preferences::default();
                preferences
                    .commands
                    .insert(action.id().into(), vec![vec!["F13".into()]]);
                state.configure_key_binding_preferences(std::sync::Arc::new(preferences));
                let before = state.workspace.file().session.serialize().unwrap();
                press(&mut state, KeyCode::F13);
                assert!(
                    matches!(state.panel, Some(Panel::QuickAdd(2))),
                    "{locale} {action:?}: {:?}",
                    state.status
                );
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                press(&mut state, KeyCode::ArrowDown);
                press(&mut state, KeyCode::Enter);
                assert!(
                    state.panel.is_none(),
                    "{locale} {action:?}: {}",
                    state.status
                );
                assert_eq!(state.workspace.file().session.document().format, Format::Sm);
                let inventory = state.workspace.event_selection_between(384, 384).unwrap();
                assert!(
                    inventory
                        .events()
                        .contains(&Event::Timing { row: 384, kind }),
                    "{action:?}"
                );
                assert_eq!(
                    state
                        .workspace
                        .file()
                        .session
                        .document()
                        .raw_property(None, "WARPS"),
                    Some(b"1=100".as_slice())
                );
                state.command(Command::Undo);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                state.command(Command::Redo);
                assert!(
                    state
                        .workspace
                        .event_selection_between(384, 384)
                        .unwrap()
                        .events()
                        .contains(&Event::Timing { row: 384, kind })
                );
                if kind == TimingKind::Warp {
                    state.open_timing();
                    assert!(matches!(state.panel, Some(Panel::Timing(0))));
                    for _ in 0..3 {
                        press(&mut state, KeyCode::Tab);
                    }
                    press(&mut state, KeyCode::ArrowDown);
                    press(&mut state, KeyCode::ArrowDown);
                    press(&mut state, KeyCode::ArrowRight);
                    for _ in 0..6 {
                        press(&mut state, KeyCode::ArrowDown);
                    }
                    press(&mut state, KeyCode::Enter);
                    assert!(state.panel.is_none(), "{}", state.status);
                    let draft =
                        region::Draft::capture(&state.workspace.file().session, 0, Scope::Chart)
                            .unwrap();
                    assert!(draft.regions(region::Kind::Warp).values()[&384] > 48);
                }
            }
        }
    }
}
