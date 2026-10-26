use deadsync_editor::{
    attacks::{Modifier, inspector::Draft, placement::Timeline},
    document::{Document, Format},
    history::Session,
    timing_edit::Scope,
};
fn session(format: Format, tempo: u32) -> Session {
    let notes = if format == Format::Sm {
        "#NOTES:dance-single::Hard:9::1000,0100;"
    } else {
        "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100;"
    };
    Session::new(
        Document::parse(
            format!("#VERSION:0.83;#BPMS:0={tempo};#UNKNOWN:retain;{notes}").into_bytes(),
            format,
        )
        .unwrap(),
    )
}
fn scope(format: Format) -> Scope {
    if format == Format::Sm {
        Scope::Song
    } else {
        Scope::Chart
    }
}
#[test]
fn detached_batch_paste_uses_destination_rows_replaces_attacks_and_preserves_other_events() {
    let source = session(Format::Ssc, 120);
    let mut source_draft = Draft::capture(&source, 0, Scope::Chart).unwrap();
    let first = source_draft.add(48, Modifier::default()).unwrap();
    source_draft
        .add_modifier(
            first,
            Modifier {
                name: "Drunk".into(),
                length: 3.0,
                ..Modifier::default()
            },
        )
        .unwrap();
    let second = source_draft.add(96, Modifier::default()).unwrap();
    source_draft.remove_modifier(second, 0).unwrap();
    assert!(source_draft.copy_events(&[]).is_err());
    assert!(source_draft.copy_events(&[0, 2]).is_err());
    let clip = source_draft.copy_events(&[1, 0, 0]).unwrap();
    // The clipboard owns a snapshot, including the empty event and every modifier.
    source_draft.remove(0).unwrap();
    drop(source_draft);
    for format in [Format::Sm, Format::Ssc] {
        let mut target = session(format, 240);
        let mut draft = Draft::capture(&target, 0, scope(format)).unwrap();
        draft
            .add(
                192,
                Modifier {
                    name: "Reverse".into(),
                    ..Modifier::default()
                },
            )
            .unwrap();
        draft.add(384, Modifier::default()).unwrap();
        draft.apply(&mut target, 0).unwrap();
        let original = target.serialize().unwrap();
        let mut draft = Draft::capture(&target, 0, scope(format)).unwrap();
        let before = draft.attacks().events().to_vec();
        assert!(draft.paste_events(&clip, i32::MAX as u64 - 24).is_err());
        assert!(draft.paste_events(&clip, u64::MAX).is_err());
        assert_eq!(draft.attacks().events(), before);
        assert_eq!(draft.paste_events(&clip, 192).unwrap(), [0, 1]);
        draft.apply(&mut target, 0).unwrap();
        let pasted = target.serialize().unwrap();
        let reopened = Session::new(Document::parse(pasted.clone(), format).unwrap());
        let timeline = Timeline::capture(&reopened, 0).unwrap();
        assert_eq!(
            timeline.markers().iter().map(|m| m.row).collect::<Vec<_>>(),
            [192, 240, 384]
        );
        assert_eq!(timeline.markers()[0].chart_time, 1.0);
        assert_eq!(timeline.markers()[0].modifiers.len(), 2);
        assert_eq!(timeline.markers()[0].modifiers[1].name, "Drunk");
        assert_eq!(timeline.markers()[0].modifiers[1].length, 3.0);
        assert!(timeline.markers()[1].modifiers.is_empty());
        assert_eq!(reopened.document().text(None, "UNKNOWN").unwrap(), "retain");
        assert_eq!(
            reopened.document().raw_property(None, "BPMS"),
            Some(b"0=240".as_slice())
        );
        assert!(
            String::from_utf8(pasted.clone())
                .unwrap()
                .contains("1000,0100")
        );
        target.undo().unwrap();
        assert_eq!(target.serialize().unwrap(), original);
        target.redo().unwrap();
        assert_eq!(target.serialize().unwrap(), pasted);
    }
}

#[test]
fn pasted_empty_and_nonempty_events_keep_distinct_rows_inside_a_destination_warp() {
    let source = session(Format::Sm, 120);
    let mut draft = Draft::capture(&source, 0, Scope::Song).unwrap();
    draft.add(0, Modifier::default()).unwrap();
    let empty = draft.add(48, Modifier::default()).unwrap();
    draft.remove_modifier(empty, 0).unwrap();
    let clip = draft.copy_events(&[0, 1]).unwrap();
    let mut target = session(Format::Ssc, 240);
    let mut timing =
        deadsync_editor::timing_edit::region::Draft::capture(&target, 0, Scope::Song).unwrap();
    timing
        .set(deadsync_editor::timing_edit::region::Kind::Warp, 0, 192)
        .unwrap();
    timing.apply(&mut target, 0).unwrap();
    let original = target.serialize().unwrap();
    let mut draft = Draft::capture(&target, 0, Scope::Chart).unwrap();
    draft.paste_events(&clip, 48).unwrap();
    draft.apply(&mut target, 0).unwrap();
    let saved = target.serialize().unwrap();
    let reopened = Session::new(Document::parse(saved.clone(), Format::Ssc).unwrap());
    let timeline = Timeline::capture(&reopened, 0).unwrap();
    assert!(timeline.has_resident_rows());
    assert_eq!(
        timeline.markers().iter().map(|m| m.row).collect::<Vec<_>>(),
        [48, 96]
    );
    assert_eq!(timeline.markers()[0].modifiers[0].name, "Dizzy");
    assert!(timeline.markers()[1].modifiers.is_empty());
    target.undo().unwrap();
    assert_eq!(target.serialize().unwrap(), original);
    target.redo().unwrap();
    assert_eq!(target.serialize().unwrap(), saved);
}
