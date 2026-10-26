use deadsync_editor::{
    attacks::{Modifier, inspector::Draft as Attacks, placement::Timeline},
    document::{Document, Format},
    event_clipboard::Clipboard,
    history::Session,
    notes::{Kind, Note, Position},
    timing_edit::Scope,
};
fn position(row: u64, lane: u16) -> Position {
    Position {
        row,
        lane,
        player: 0,
    }
}
fn note(end: Option<u64>) -> Note {
    Note {
        kind: if end.is_some() { Kind::Hold } else { Kind::Tap },
        end,
        attributes: "".into(),
        tail_attributes: "".into(),
    }
}
fn source() -> Session {
    let mut session = Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=120,4=180;#STOPS:4=0.5;#DELAYS:4=0.25;#TIMESIGNATURES:0=4=4,4=3=4;#TICKCOUNTS:4=8;#COMBOS:4=2=3;#WARPS:4=1;#FAKES:5=1;#SCROLLS:4=-1;#SPEEDS:4=2=1=0;#LABELS:4=Verse;#NOTES:1000;".as_slice(), Format::Ssc).unwrap());
    session
        .edit_notes(
            0,
            "Notes",
            [
                (position(192, 0), Some(note(None))),
                (position(240, 1), Some(note(Some(336)))),
            ],
        )
        .unwrap();
    let mut attacks = Attacks::capture(&session, 0, Scope::Chart).unwrap();
    attacks
        .add(
            216,
            Modifier {
                name: "Drunk".into(),
                length: 3.0,
                ..Modifier::default()
            },
        )
        .unwrap();
    attacks.apply(&mut session, 0).unwrap();
    session
}
fn target(format: Format) -> Session {
    let notes = if format == Format::Sm {
        "#NOTES:dance-single::Hard:9::1000;"
    } else {
        "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
    };
    Session::new(
        Document::parse(
            format!("#VERSION:0.83;#BPMS:0=90;#UNKNOWN:retain;{notes}").into_bytes(),
            format,
        )
        .unwrap(),
    )
}
#[test]
fn mixed_paste_transfers_every_timing_family_notes_holds_and_resident_attacks_as_one_gesture() {
    let mut source = source();
    let source_bytes = source.serialize().unwrap();
    // The first actual event, rather than the empty beginning of the rectangle, anchors paste.
    let clipboard = Clipboard::copy(&mut source, 0, 180, 240).unwrap().unwrap();
    let mut target = target(Format::Ssc);
    target
        .edit_notes(0, "Collision", [(position(360, 0), Some(note(Some(450))))])
        .unwrap();
    let original = target.serialize().unwrap();
    assert!(clipboard.paste(&mut target, 0, 384, 0).unwrap());
    assert_eq!(source.serialize().unwrap(), source_bytes);
    assert_eq!(
        target.grid(0).unwrap().get(position(384, 0)).unwrap().kind,
        Kind::Tap
    );
    assert_eq!(
        target.grid(0).unwrap().get(position(432, 1)).unwrap().end,
        Some(528)
    );
    assert_eq!(
        target.grid(0).unwrap().get(position(360, 0)).unwrap().end,
        Some(372)
    );
    for (tag, expected) in [
        ("BPMS", "0=90,8=180"),
        ("STOPS", "8=0.5"),
        ("DELAYS", "8=0.25"),
        ("TIMESIGNATURES", "0=4=4,8=3=4"),
        ("TICKCOUNTS", "8=8"),
        ("COMBOS", "8=2=3"),
        ("WARPS", "8=1"),
        ("FAKES", "9=1"),
        ("SCROLLS", "8=-1"),
        ("SPEEDS", "8=2=1=0"),
        ("LABELS", "8=Verse"),
    ] {
        assert_eq!(
            target.document().text(Some(0), tag).unwrap(),
            expected,
            "{tag}"
        );
    }
    let timeline = Timeline::capture(&target, 0).unwrap();
    assert_eq!(timeline.markers()[0].row, 408);
    assert_eq!(timeline.markers()[0].modifiers[0].name, "Drunk");
    let pasted = target.serialize().unwrap();
    let mut reopened = Session::new(Document::parse(pasted.clone(), Format::Ssc).unwrap());
    assert_eq!(
        Timeline::capture(&reopened, 0).unwrap().markers()[0].row,
        408
    );
    assert_eq!(
        reopened.grid(0).unwrap().get(position(432, 1)).unwrap().end,
        Some(528)
    );
    target.undo().unwrap();
    assert_eq!(target.serialize().unwrap(), original);
    target.redo().unwrap();
    assert_eq!(target.serialize().unwrap(), pasted);
}
#[test]
fn mixed_cut_keeps_initial_timing_and_undo_restores_all_families_and_note_bytes() {
    let mut session = source();
    let original = session.serialize().unwrap();
    let clipboard = Clipboard::cut(&mut session, 0, 0, 240).unwrap().unwrap();
    assert_eq!(session.grid(0).unwrap().iter().count(), 0);
    assert!(Timeline::capture(&session, 0).unwrap().markers().is_empty());
    assert_eq!(session.document().text(Some(0), "BPMS").unwrap(), "0=120");
    assert_eq!(
        session.document().text(Some(0), "TIMESIGNATURES").unwrap(),
        "0=4=4"
    );
    let cut = session.serialize().unwrap();
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), cut);
    clipboard.paste(&mut session, 0, 0, 0).unwrap();
    assert_eq!(session.grid(0).unwrap().iter().count(), 3);
    assert_eq!(
        Timeline::capture(&session, 0).unwrap().markers()[0].row,
        216
    );
}
#[test]
fn unsupported_format_overflow_and_layout_failures_cannot_publish_partial_notes_or_erase_redo() {
    let mut source = source();
    assert!(Clipboard::copy(&mut source, 0, 20, 10).is_err());
    assert!(Clipboard::copy(&mut source, 0, 400, 500).unwrap().is_none());
    let clipboard = Clipboard::copy(&mut source, 0, 192, 240).unwrap().unwrap();
    for (format, row) in [(Format::Sm, 384), (Format::Ssc, i32::MAX as u64)] {
        let mut target = target(format);
        target.set_text(None, "TITLE", "Redo").unwrap();
        target.undo().unwrap();
        let original = target.serialize().unwrap();
        let redo = target.redo_label().unwrap().to_owned();
        assert!(clipboard.paste(&mut target, 0, row, 0).is_err());
        assert_eq!(target.serialize().unwrap(), original);
        assert_eq!(target.redo_label(), Some(redo.as_str()));
        assert_eq!(target.grid(0).unwrap().iter().count(), 1);
    }
    let bytes = String::from_utf8(target(Format::Ssc).serialize().unwrap())
        .unwrap()
        .replace("dance-single", "pump-single")
        .replace("#NOTES:1000;", "#NOTES:10000;");
    let mut other_layout = Session::new(Document::parse(bytes.as_bytes(), Format::Ssc).unwrap());
    let before = other_layout.serialize().unwrap();
    assert!(clipboard.paste(&mut other_layout, 0, 384, 0).is_err());
    assert_eq!(other_layout.serialize().unwrap(), before);
}

#[test]
fn timing_only_clipboard_can_cross_layouts_without_imposing_note_lane_constraints() {
    let mut source = Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=120,4=180;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                .as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    let clipboard = Clipboard::copy(&mut source, 0, 192, 192).unwrap().unwrap();
    let mut target = Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=90;#NOTEDATA:;#STEPSTYPE:pump-single;#NOTES:10000;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    clipboard.paste(&mut target, 0, 96, 0).unwrap();
    assert_eq!(
        target.document().text(Some(0), "BPMS").unwrap(),
        "0=90,2=180"
    );
    assert_eq!(target.grid(0).unwrap().iter().count(), 1);
}

#[test]
fn atomic_chart_gestures_see_other_unsaved_grids_without_allowing_other_chart_note_changes() {
    let mut session = target(Format::Ssc);
    session.duplicate_chart(0, session.revision()).unwrap();
    session
        .edit_notes(1, "Other chart", [(position(48, 1), Some(note(None)))])
        .unwrap();
    let before = session.serialize().unwrap();
    session
        .edit_chart_atomic(0, "Read all pending notes", |staged| {
            assert!(staged.grid(1)?.get(position(48, 1)).is_some());
            staged.edit_notes(0, "Target", [(position(96, 2), Some(note(None)))])?;
            Ok(())
        })
        .unwrap();
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    let redo = session.redo_label().unwrap().to_owned();
    assert!(
        session
            .edit_chart_atomic(0, "Forbidden", |staged| {
                staged.edit_notes(1, "Other chart", [(position(96, 2), Some(note(None)))])?;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(session.serialize().unwrap(), before);
    assert_eq!(session.redo_label(), Some(redo.as_str()));
}

#[test]
fn cloning_loaded_timing_preserves_legacy_values_and_label_text_without_entry_sanitization() {
    let bytes = br"#VERSION:0.83;#BPMS:0=120;#TICKCOUNTS:4=-4;#COMBOS:4=-2=3;#SPEEDS:4=-2=-1=1;#LABELS:4=Intro\/Outro ,;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let mut source = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    let source_bytes = source.serialize().unwrap();
    let clipboard = Clipboard::copy(&mut source, 0, 192, 192).unwrap().unwrap();
    let mut target = target(Format::Ssc);
    clipboard.paste(&mut target, 0, 384, 0).unwrap();
    assert_eq!(
        target.document().text(Some(0), "TICKCOUNTS").unwrap(),
        "8=-4"
    );
    assert_eq!(target.document().text(Some(0), "COMBOS").unwrap(), "8=-2=3");
    assert_eq!(
        target.document().text(Some(0), "SPEEDS").unwrap(),
        "8=-2=-1=1"
    );
    let reopened = Session::new(Document::parse(target.serialize().unwrap(), Format::Ssc).unwrap());
    assert_eq!(
        deadsync_editor::timing_edit::label::Labels::from_document(reopened.document(), 0)
            .unwrap()
            .values()
            .get(&384)
            .unwrap(),
        "Intro/Outro "
    );
    assert_eq!(source.serialize().unwrap(), source_bytes);
}
