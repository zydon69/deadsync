//! Explicit source Shift All row route, using the mixed-event transaction.
use super::{Panel, State};
use crate::i18n;
use deadlib_platform::input::KeyCode;
use deadsync_editor::shift_all::Plan;
#[derive(Default)]
pub(super) struct Settings {
    later: bool,
    checked: bool,
    return_panel: Option<Panel>,
    plan: Option<Plan>,
}

#[cfg(test)]
mod selection_history_tests {
    use super::*;
    use deadsync_editor::{event_clipboard::{Event, TimingKind}, notes::Position, workspace::Command};

    #[test]
    fn checked_and_global_shift_select_survivors_and_undo_restores_deleted_originals_en_fr() {
        for fr in [false, true] {
            if fr { crate::i18n::init(deadsync_assets::language::load_for_tests("fr")); }
            else { crate::i18n::init_for_tests(); }
            for checked in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("shift.ssc");
                std::fs::write(&path, b"#VERSION:0.83;#BPMS:0=120,2=150;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0000\n0000;").unwrap();
                let mut state = State::open(&path, 0).unwrap();
                let before = state.workspace.file().session.serialize().unwrap();
                let rows = deadsync_editor::workspace::snap_shift_rows(state.workspace.snap());
                assert!((1..=48).contains(&rows));
                if checked {
                    let mut selection = state.workspace.event_selection_between(0, 192).unwrap();
                    selection.select_all(true);
                    state.replace_region_event_selection(selection);
                    state.command_checked_shift_review(false);
                } else { state.command_shift_all_review(false); }
                assert!(matches!(state.panel, Some(Panel::ShiftAll(1))));
                state.shift_all_key(2, KeyCode::Enter);
                assert!(state.panel.is_none(), "{}", state.status);
                let events = |state: &State| state.event_selection.as_ref().map(|settings| settings.selection.selected_events().collect::<Vec<_>>());
                let moved = vec![
                    Event::Note(Position { row: 48-rows, lane: 1, player: 0 }),
                    Event::Timing { row: 96-rows, kind: TimingKind::Tempo },
                ];
                assert_eq!(events(&state), Some(moved.clone()));
                let after = state.workspace.file().session.serialize().unwrap();
                state.command(Command::Undo);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                assert_eq!(events(&state), Some(vec![
                    Event::Note(Position { row: 0, lane: 0, player: 0 }),
                    Event::Note(Position { row: 48, lane: 1, player: 0 }),
                    Event::Timing { row: 96, kind: TimingKind::Tempo },
                ]));
                state.command(Command::Redo);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), after);
                assert_eq!(events(&state), Some(moved));
                assert_eq!(state.workspace.row(), 0);
            }
        }
    }
}
impl State {
    pub(super) fn command_checked_shift_review(&mut self, later: bool) {
        if self
            .event_selection
            .as_ref()
            .is_none_or(|settings| settings.selection.selected_count() == 0)
        {
            self.status = i18n::tr("Edition", "NoteOperationMissingSelection");
            return;
        }
        self.shift_all = Settings {
            later,
            checked: true,
            return_panel: self.panel,
            plan: None,
        };
        self.prepare_shift_all_review();
        self.panel = Some(Panel::ShiftAll(1));
    }
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
                        self.sync_event_selection_history();
                        self.panel = None;
                        self.status = i18n::tr(
                            "Edition",
                            if self.shift_all.checked {
                                "ShiftCheckedApplied"
                            } else {
                                "ShiftAllApplied"
                            },
                        );
                        self.refresh();
                    }
                    Err(error) => {
                        self.status = i18n::tr(
                            "Edition",
                            match error.reason {
                                deadsync_editor::shift_all::Refusal::Stale => {
                                    if self.shift_all.checked {
                                        "ShiftCheckedStale"
                                    } else {
                                        "ShiftAllStale"
                                    }
                                }
                                deadsync_editor::shift_all::Refusal::MissingReview => {
                                    "NoteOperationPrepareFirst"
                                }
                                deadsync_editor::shift_all::Refusal::Invalid => {
                                    if self.shift_all.checked {
                                        "ShiftCheckedRefused"
                                    } else {
                                        "ShiftAllRefused"
                                    }
                                }
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
        let result = if self.shift_all.checked {
            self.event_selection
                .as_ref()
                .ok_or_else(|| {
                    deadsync_editor::shift_all::Failure::from(deadsync_editor::Error::InvalidEdit(
                        "No checked events".into(),
                    ))
                })
                .and_then(|settings| {
                    self.workspace
                        .prepare_checked_shift(&settings.selection, self.shift_all.later)
                })
        } else {
            self.workspace.prepare_shift_all(self.shift_all.later)
        };
        match result {
            Ok(plan) => {
                self.shift_all.plan = Some(plan);
                self.status = i18n::tr(
                    "Edition",
                    if self.shift_all.checked {
                        "ShiftCheckedPrepared"
                    } else {
                        "ShiftAllPrepared"
                    },
                );
            }
            Err(_) => {
                self.shift_all.plan = None;
                self.status = i18n::tr(
                    "Edition",
                    if self.shift_all.checked {
                        "ShiftCheckedRefused"
                    } else {
                        "ShiftAllRefused"
                    },
                );
            }
        }
    }
    fn close_shift_all(&mut self) {
        self.panel = if self.shift_all.checked {
            self.shift_all.return_panel
        } else {
            Some(Panel::Transforms(0))
        };
        self.shift_all = Default::default();
    }
    pub(super) fn shift_all_label(&self, index: usize, key: &str) -> std::sync::Arc<str> {
        let key = if self.shift_all.checked {
            match index {
                0 => "ShiftCheckedDirection",
                2 => "ShiftCheckedApply",
                _ => key,
            }
        } else {
            key
        };
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

#[cfg(test)]
mod checked_tests {
    use super::*;
    use deadlib_platform::input::RawKeyboardEvent;
    use deadsync_editor::{
        event_clipboard::{Event, presets::Preset},
        key_binding_preferences::{Action, Preferences},
        workspace::{Command, SNAP_ROWS},
    };
    use std::{sync::Arc, time::Instant};
    fn key(s: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            s.raw_key(&RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn state(fr: bool) -> (tempfile::TempDir, State) {
        if fr {
            crate::i18n::init(deadsync_assets::language::load_for_tests("fr"));
        } else {
            crate::i18n::init_for_tests();
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.ssc");
        std::fs::write(&path, b"#VERSION:0.83;#BPMS:0=120,1=180;#LABELS:2=keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0000\n0000;").unwrap();
        let mut s = State::open(&path, 0).unwrap();
        let mut p = Preferences::default();
        for (action, code) in [(Action::ShiftEarlier, "F15"), (Action::ShiftLater, "F16")] {
            p.commands
                .insert(action.id().into(), vec![vec![code.into()]]);
        }
        s.configure_key_binding_preferences(Arc::new(p));
        (dir, s)
    }
    #[test]
    fn source_checked_shift_shortcut_reviews_exact_notes_with_off_snap_fallback_and_undo_selection()
    {
        for fr in [false, true] {
            let (_dir, mut s) = state(fr);
            s.workspace
                .execute(Command::SetSnap(
                    SNAP_ROWS.iter().position(|&v| v == 0).unwrap(),
                ))
                .unwrap();
            s.command_selection_preset(Preset::NotesAndMiscellaneous, false);
            let selection = &mut s.event_selection.as_mut().unwrap().selection;
            selection.select_all(false);
            let index = selection
                .events()
                .iter()
                .position(|e| matches!(e, Event::Note(p) if p.row == 0))
                .unwrap();
            let original = selection.events()[index];
            selection.set_selected(index, true).unwrap();
            s.panel = Some(Panel::EventSelection);
            let before = s.workspace.file().session.serialize().unwrap();
            key(&mut s, KeyCode::F16);
            assert!(matches!(s.panel, Some(Panel::ShiftAll(1))));
            assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
            let report = s.shift_all.plan.as_ref().unwrap().report();
            assert_eq!(report.moved, 1);
            assert!(
                matches!(report.selection_after.as_slice(), [Event::Note(p)] if p.row == 48 && p.lane == 0)
            );
            key(&mut s, KeyCode::Escape);
            assert!(matches!(s.panel, Some(Panel::EventSelection)));
            assert!(
                s.event_selection
                    .as_ref()
                    .unwrap()
                    .selection
                    .selected_events()
                    .any(|e| e == original)
            );
            key(&mut s, KeyCode::F16);
            key(&mut s, KeyCode::ArrowDown);
            key(&mut s, KeyCode::Enter);
            assert!(s.panel.is_none(), "{}", s.status);
            assert_eq!(
                s.workspace
                    .file()
                    .session
                    .document()
                    .text(None, "BPMS")
                    .as_deref(),
                Some("0=120,1=180")
            );
            assert!(
                s.event_selection
                    .as_ref()
                    .unwrap()
                    .selection
                    .selected_events()
                    .any(|e| matches!(e, Event::Note(p) if p.row == 48 && p.lane == 0))
            );
            s.command(Command::Undo);
            assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
            assert!(
                s.event_selection
                    .as_ref()
                    .unwrap()
                    .selection
                    .selected_events()
                    .any(|e| e == original)
            );
            s.command(Command::Redo);
            assert!(
                s.event_selection
                    .as_ref()
                    .unwrap()
                    .selection
                    .selected_events()
                    .any(|e| matches!(e, Event::Note(p) if p.row == 48 && p.lane == 0))
            );
        }
    }
    #[test]
    fn row_range_without_checked_events_never_becomes_global_shift() {
        for fr in [false, true] {
            let (_dir, mut s) = state(fr);
            s.command(Command::ToggleSelection);
            let before = s.workspace.file().session.serialize().unwrap();
            key(&mut s, KeyCode::F15);
            assert!(s.panel.is_none());
            assert_eq!(
                s.status,
                i18n::tr("Edition", "NoteOperationMissingSelection")
            );
            assert_eq!(s.workspace.file().session.serialize().unwrap(), before);
        }
    }
    #[test]
    fn original_mac_selected_shift_chords_route_from_picker_without_range_fallback() {
        use deadsync_editor::key_binding_preferences::source_profile::{self, Platform};
        for fr in [false, true] {
            for (terminal, later) in [(KeyCode::ArrowUp, false), (KeyCode::ArrowDown, true)] {
                let (_dir, mut s) = state(fr);
                let source =
                    source_profile::preset(&Preferences::default(), Platform::Mac).unwrap();
                s.configure_key_binding_preferences(Arc::new(source.preferences));
                s.command_selection_preset(Preset::NotesAndMiscellaneous, false);
                let selected = &mut s.event_selection.as_mut().unwrap().selection;
                selected.select_all(false);
                let index = selected
                    .events()
                    .iter()
                    .position(|e| matches!(e, Event::Note(p) if p.row == 48))
                    .unwrap();
                selected.set_selected(index, true).unwrap();
                s.panel = Some(Panel::EventSelection);
                for (code, pressed) in [
                    (KeyCode::SuperLeft, true),
                    (KeyCode::ShiftLeft, true),
                    (terminal, true),
                    (terminal, false),
                    (KeyCode::ShiftLeft, false),
                    (KeyCode::SuperLeft, false),
                ] {
                    s.raw_key(&RawKeyboardEvent {
                        code,
                        pressed,
                        repeat: false,
                        timestamp: Instant::now(),
                        host_nanos: 0,
                    });
                }
                assert!(matches!(s.panel, Some(Panel::ShiftAll(1))), "{}", s.status);
                assert!(s.shift_all.checked);
                assert_eq!(s.shift_all.later, later);
                assert_eq!(s.shift_all.plan.as_ref().unwrap().report().moved, 1);
                assert!(!s.workspace.file().session.is_dirty());
            }
        }
    }
}
