use deadsync_editor::{
    attacks::{inspector::Draft as Attacks, placement::Timeline},
    document::{Document, Format},
    event_clipboard::{Event, Selection, TimingKind},
    history::Session,
    notes::{Kind, Position},
    timing_edit::{self, Scope},
    workspace::{Command, Workspace},
};
fn position(row: u64, lane: u16) -> Position {
    Position {
        row,
        lane,
        player: 0,
    }
}
fn source() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=120,4=180;#STOPS:1=0.5;#LABELS:2=Pick;#ATTACKS:TIME=1.5:LEN=2:MODS=100% Dizzy;#NOTES:1M00,0200,0300;".as_slice(), Format::Ssc).unwrap())
}
fn choose(selection: &mut Selection, predicate: impl Fn(Event) -> bool) {
    selection.select_all(false);
    for index in 0..selection.events().len() {
        if predicate(selection.events()[index]) {
            selection.set_selected(index, true).unwrap();
        }
    }
}
fn subset(selection: &mut Selection) {
    choose(selection, |event| {
        matches!(event, Event::Note(p) if p == position(0, 0))
            || matches!(
                event,
                Event::Timing {
                    row: 96,
                    kind: TimingKind::Label
                } | Event::Attack { row: 96 }
            )
    });
    assert_eq!(selection.selected_count(), 3);
}
#[test]
fn exact_mixed_subset_copy_and_cut_leave_same_row_notes_and_other_timing_unselected() {
    let mut source = source();
    let original = source.serialize().unwrap();
    let mut selection = Selection::capture(&mut source, 0, 0, 192).unwrap();
    assert!(
        selection
            .events()
            .windows(2)
            .all(|p| p[0].row() <= p[1].row())
    );
    assert_eq!(selection.note_kind(position(0, 1)), Some(Kind::Mine));
    subset(&mut selection);
    let clipboard = selection.copy(&mut source).unwrap().unwrap();
    assert_eq!(source.serialize().unwrap(), original);
    let mut target = Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=240;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    let before = target.serialize().unwrap();
    clipboard.paste(&mut target, 0, 384, 0).unwrap();
    assert!(target.grid(0).unwrap().get(position(384, 0)).is_some());
    assert!(target.grid(0).unwrap().get(position(384, 1)).is_none());
    assert!(target.grid(0).unwrap().get(position(576, 1)).is_none());
    assert_eq!(
        timing_edit::TempoDraft::capture(&target, 0, Scope::Chart)
            .unwrap()
            .tempos()
            .values()
            .len(),
        1
    );
    assert!(
        timing_edit::pause::Draft::capture(&target, 0, Scope::Chart)
            .unwrap()
            .pauses(timing_edit::pause::Kind::Stop)
            .values()
            .is_empty()
    );
    assert_eq!(
        timing_edit::label::Labels::from_document(target.document(), 0)
            .unwrap()
            .values()
            .get(&480)
            .unwrap(),
        "Pick"
    );
    let attacks = Timeline::capture(&target, 0).unwrap();
    assert_eq!(attacks.markers()[0].row, 480);
    assert_eq!(attacks.markers()[0].chart_time, 2.5);
    let pasted = target.serialize().unwrap();
    let reopened = Session::new(Document::parse(pasted.clone(), Format::Ssc).unwrap());
    assert_eq!(
        Timeline::capture(&reopened, 0).unwrap().markers()[0].row,
        480
    );
    target.undo().unwrap();
    assert_eq!(target.serialize().unwrap(), before);
    target.redo().unwrap();
    assert_eq!(target.serialize().unwrap(), pasted);
    selection.cut(&mut source).unwrap().unwrap();
    assert!(source.grid(0).unwrap().get(position(0, 0)).is_none());
    assert_eq!(
        source.grid(0).unwrap().get(position(0, 1)).unwrap().kind,
        Kind::Mine
    );
    assert_eq!(
        source.grid(0).unwrap().get(position(192, 1)).unwrap().end,
        Some(384)
    );
    assert_eq!(
        timing_edit::TempoDraft::capture(&source, 0, Scope::Chart)
            .unwrap()
            .tempos()
            .values()
            .len(),
        2
    );
    assert_eq!(
        timing_edit::pause::Draft::capture(&source, 0, Scope::Chart)
            .unwrap()
            .pauses(timing_edit::pause::Kind::Stop)
            .values()
            .get(&48),
        Some(&0.5)
    );
    assert!(Timeline::capture(&source, 0).unwrap().markers().is_empty());
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), original);
}
#[test]
fn deleting_only_a_stop_keeps_unselected_attack_rows_labels_and_all_notes() {
    let mut source = source();
    let original = source.serialize().unwrap();
    let notes = source.grid(0).unwrap().clone();
    let mut selection = Selection::capture(&mut source, 0, 0, 192).unwrap();
    choose(&mut selection, |event| {
        matches!(
            event,
            Event::Timing {
                row: 48,
                kind: TimingKind::Stop
            }
        )
    });
    assert!(selection.delete(&mut source).unwrap());
    assert_eq!(source.grid(0).unwrap(), &notes);
    assert_eq!(Timeline::capture(&source, 0).unwrap().markers()[0].row, 96);
    assert_eq!(
        timing_edit::label::Labels::from_document(source.document(), 0)
            .unwrap()
            .values()
            .get(&96)
            .unwrap(),
        "Pick"
    );
    source.undo().unwrap();
    assert_eq!(source.serialize().unwrap(), original);
}
#[test]
fn stale_foreign_and_invalid_choices_preserve_document_redo_and_selection() {
    let mut source = source();
    let mut selection = Selection::capture(&mut source, 0, 0, 192).unwrap();
    let count = selection.selected_count();
    assert!(selection.set_selected(usize::MAX, false).is_err());
    assert_eq!(selection.selected_count(), count);
    let mut foreign =
        Session::new(Document::parse(source.serialize().unwrap(), Format::Ssc).unwrap());
    assert!(selection.copy(&mut foreign).is_err());
    source.set_text(None, "TITLE", "Changed").unwrap();
    source.set_text(None, "TITLE", "Redo").unwrap();
    source.undo().unwrap();
    let before = source.serialize().unwrap();
    let redo = source.redo_label().unwrap().to_owned();
    assert!(selection.copy(&mut source).is_err());
    assert!(selection.cut(&mut source).is_err());
    assert!(selection.delete(&mut source).is_err());
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.redo_label(), Some(redo.as_str()));
}
#[test]
fn empty_and_protected_only_selections_are_noops_even_with_resident_attack_metadata() {
    let mut source = source();
    let mut warp = timing_edit::region::Draft::capture(&source, 0, Scope::Chart).unwrap();
    warp.set(timing_edit::region::Kind::Warp, 192, 192).unwrap();
    warp.apply(&mut source, 0).unwrap();
    let mut attacks = Attacks::capture(&source, 0, Scope::Chart).unwrap();
    let modifier = attacks.attacks().events()[0].modifiers[0].clone();
    attacks.set_at_row(0, 0, 240, modifier).unwrap();
    attacks.apply(&mut source, 0).unwrap();
    assert!(Timeline::capture(&source, 0).unwrap().has_resident_rows());
    let mut selection = Selection::capture(&mut source, 0, 0, 0).unwrap();
    let before = source.serialize().unwrap();
    let revision = source.revision();
    selection.select_all(false);
    assert!(selection.copy(&mut source).unwrap().is_none());
    assert!(selection.cut(&mut source).unwrap().is_none());
    assert!(!selection.delete(&mut source).unwrap());
    choose(&mut selection, |event| {
        matches!(
            event,
            Event::Timing {
                row: 0,
                kind: TimingKind::Tempo | TimingKind::Signature
            }
        )
    });
    assert!(!selection.delete(&mut source).unwrap());
    let clipboard = selection.copy(&mut source).unwrap().unwrap();
    assert!(!clipboard.paste(&mut source, 0, 0, 0).unwrap());
    assert_eq!(source.serialize().unwrap(), before);
    assert_eq!(source.revision(), revision);
}
#[test]
fn workspace_delete_retains_previous_clipboard_and_rejects_another_chart_selection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("selection.ssc");
    let mut source = source();
    source.duplicate_chart(0, source.revision()).unwrap();
    std::fs::write(&path, source.serialize().unwrap()).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    let mut selection = workspace.event_selection().unwrap();
    choose(&mut selection, |e| e == Event::Note(position(0, 0)));
    workspace.copy_event_selection(&selection).unwrap();
    let mut deletion = workspace.event_selection().unwrap();
    choose(&mut deletion, |e| e == Event::Note(position(0, 1)));
    workspace.delete_event_selection(&deletion).unwrap();
    workspace.execute(Command::Seek(384)).unwrap();
    workspace.execute(Command::PasteEvents).unwrap();
    assert!(workspace.grid().get(position(384, 0)).is_some());
    assert!(workspace.grid().get(position(384, 1)).is_none());
    let selection = workspace.event_selection().unwrap();
    workspace.execute(Command::SelectChart(1)).unwrap();
    let before = workspace.file().session.serialize().unwrap();
    assert!(workspace.copy_event_selection(&selection).is_err());
    assert!(workspace.cut_event_selection(&selection).is_err());
    assert!(workspace.delete_event_selection(&selection).is_err());
    assert_eq!(workspace.file().session.serialize().unwrap(), before);
}

#[test]
fn excluding_earlier_candidates_reanchors_a_noteless_choice_to_its_first_selected_event() {
    let mut source = source();
    let mut selection = Selection::capture(&mut source, 0, 0, 192).unwrap();
    choose(&mut selection, |event| {
        matches!(
            event,
            Event::Timing {
                row: 96,
                kind: TimingKind::Label
            } | Event::Attack { row: 96 }
        )
    });
    assert_eq!(selection.selected_count(), 2);
    let clipboard = selection.copy(&mut source).unwrap().unwrap();
    let mut target = Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=240;#NOTEDATA:;#STEPSTYPE:pump-single;#NOTES:10000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    clipboard.paste(&mut target, 0, 192, 0).unwrap();
    assert_eq!(target.grid(0).unwrap().iter().count(), 1);
    assert_eq!(Timeline::capture(&target, 0).unwrap().markers()[0].row, 192);
    assert_eq!(
        timing_edit::label::Labels::from_document(target.document(), 0)
            .unwrap()
            .values()
            .get(&192)
            .unwrap(),
        "Pick"
    );
}
