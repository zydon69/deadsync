use super::State;
use crate::i18n;

impl State {
    pub(super) fn paste_selected_events(&mut self) {
        let result=(||->Result<deadsync_editor::event_clipboard::PasteReport,deadsync_editor::Error>{
            let before=self.capture_song_property_presentation()?;let row=before.row();
            let Some((report,application))=self.workspace.paste_events_at_fractional_row_with_presentation_receipt(row)? else{return Ok(Default::default());};
            if report.changed {
                if let Some(receipt)=application.presentation_receipt(){self.rebind_song_property_presentation(before,receipt)?;}
                else{self.stop_audio();self.focus_position=deadsync_editor::chart_focus::Position::at_fractional_row(&self.workspace.file().session,self.workspace.chart(),row).ok();}
            }
            Ok(report)
        })();
        self.sync_event_selection_history();
        match result {
            Ok(report) if report.patterns_skipped > 0 || report.notes_skipped > 0 => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    if report.changed {
                        "EventPasteAppliedSkipped"
                    } else {
                        "EventPasteUnchangedSkipped"
                    },
                    &[
                        ("patterns", &report.patterns_skipped.to_string()),
                        ("notes", &report.notes_skipped.to_string()),
                    ],
                );
            }
            Ok(report) => {
                self.status = i18n::tr(
                    "Edition",
                    if report.changed {
                        "EventPasteApplied"
                    } else {
                        "EventPasteNoChange"
                    },
                )
            }
            Err(error) => {
                self.status = i18n::tr_fmt(
                    "Edition",
                    "Error",
                    &[("message", &crate::i18n::editor_error(&error))],
                )
            }
        }
        self.refresh();
    }
}

#[cfg(test)]
mod selection_history_tests {
    use super::*;
    use deadsync_editor::{
        event_clipboard::{Event, TimingKind},
        notes::Position,
        workspace::Command,
    };

    #[test]
    fn mixed_and_note_only_paste_restore_target_selection_through_history_en_fr() {
        for fr in [false, true] {
            if fr {
                crate::i18n::init(deadsync_assets::language::load_for_tests("fr"));
            } else {
                crate::i18n::init_for_tests();
            }
            for mixed in [false, true] {
                let root = tempfile::tempdir().unwrap();
                let path = root.path().join("paste.ssc");
                std::fs::write(&path, b"#VERSION:0.83;#BPMS:0=120,2=150;#LABELS:1=marker;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0000;").unwrap();
                let mut state = State::open(&path, 0).unwrap();
                if mixed {
                    let mut selected = state.workspace.event_selection_between(48, 96).unwrap();
                    selected.select_all(true);
                    state.workspace.copy_event_selection(&selected).unwrap();
                } else {
                    for command in [
                        Command::Seek(48),
                        Command::ToggleSelection,
                        Command::Seek(96),
                        Command::Copy,
                    ] {
                        state.workspace.execute(command).unwrap();
                    }
                }
                state.navigate_exact_position(384.0, true);
                let before = state.workspace.file().session.serialize().unwrap();
                if mixed {
                    state.paste_selected_events();
                } else {
                    state.command(Command::Paste);
                }
                let events = |state: &State| {
                    state
                        .event_selection
                        .as_ref()
                        .map(|settings| settings.selection.selected_events().collect::<Vec<_>>())
                };
                let mut expected = Vec::new();
                if mixed {
                    expected.push(Event::Timing {
                        row: 384,
                        kind: TimingKind::Label,
                    });
                }
                expected.push(Event::Note(Position {
                    row: 384,
                    lane: 1,
                    player: 0,
                }));
                if mixed {
                    expected.push(Event::Timing {
                        row: 432,
                        kind: TimingKind::Tempo,
                    });
                }
                expected.push(Event::Note(Position {
                    row: 432,
                    lane: 2,
                    player: 0,
                }));
                assert_eq!(
                    events(&state),
                    Some(expected.clone()),
                    "mixed={mixed}: {}",
                    state.status
                );
                let after = state.workspace.file().session.serialize().unwrap();
                assert_ne!(after, before);
                state.command(Command::Undo);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
                assert!(state.event_selection.is_none());
                state.command(Command::Redo);
                assert_eq!(state.workspace.file().session.serialize().unwrap(), after);
                assert_eq!(events(&state), Some(expected));
                assert_eq!(state.workspace.row(), 384);
            }
        }
    }
}
