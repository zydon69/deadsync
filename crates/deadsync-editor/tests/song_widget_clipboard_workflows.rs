use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{
        Event, Selection, WidgetKind,
        presets::{Mode, Preset},
    },
    history::Session,
    notes::{Kind, Note, Position},
    playback::Clock,
    song_properties::Draft,
};
fn session(format: Format) -> Session {
    let chart = match format {
        Format::Sm => "#NOTES:dance-single::Hard:9::0000;",
        Format::Ssc => "#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;",
    };
    Session::new(Document::parse(format!("#VERSION:0.83;#BPMS:0=120;#OFFSET:0.25;#SAMPLESTART:1.25;#SAMPLELENGTH:5;#LASTSECONDHINT:2;#VENDOR:keep;{chart}").into_bytes(),format).unwrap())
}
fn widgets(session: &mut Session, kind: &str) -> Selection {
    let mut selected = Selection::capture(session, 0, 0, i32::MAX as u64).unwrap();
    selected.select_all(false);
    selected.include_song_widgets(session).unwrap();
    for i in 0..selected.events().len() {
        let Event::SongWidget(w) = selected.events()[i] else {
            continue;
        };
        if kind == "both" || (kind == "preview") == (w.kind() == WidgetKind::Preview) {
            selected.set_selected(i, true).unwrap();
        }
    }
    selected
}
fn state(session: &Session) -> [f64; 3] {
    let d = Draft::capture(session).unwrap();
    [d.sample_start(), d.last_second_hint(), d.sample_length()]
}
#[test]
fn original_widget_callbacks_match_with_detached_metadata_and_reliable_atomic_undo() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/song-widget-actions-source.json"
    ))
    .unwrap();
    for format in [Format::Sm, Format::Ssc] {
        for case in cases.as_array().unwrap() {
            let mut source = session(format);
            let kind = case["kind"].as_str().unwrap();
            let selected = widgets(&mut source, kind);
            let original = source.serialize().unwrap();
            let before = state(&source);
            let op = case["operation"].as_str().unwrap();
            match op {
                "copy" => assert!(selected.copy(&mut source).unwrap().is_some()),
                "cut" => assert!(selected.cut(&mut source).unwrap().is_some()),
                "delete" => assert!(!selected.delete(&mut source).unwrap()),
                "shift" => {
                    let p = deadsync_editor::event_shift::Plan::capture(&mut source, &selected, 48)
                        .unwrap();
                    assert_eq!(p.report().immutable, selected.selected_count());
                    assert!(!p.apply(&mut source).unwrap().changed);
                }
                "paste" => {
                    let clip = selected.copy(&mut source).unwrap().unwrap();
                    let target = case["target"].as_f64().unwrap();
                    let report = clip
                        .paste_at_fractional_row(&mut source, 0, target, 0)
                        .unwrap();
                    let actual = state(&source);
                    for (name, value) in [
                        ("preview", actual[0]),
                        ("hint", actual[1]),
                        ("length", actual[2]),
                    ] {
                        assert!(
                            (value - case["after"][name].as_f64().unwrap()).abs() < 1e-12,
                            "{case} {name}: {value}"
                        );
                    }
                    if report.changed {
                        assert!(source.undo().unwrap());
                        assert_eq!(
                            source.serialize().unwrap(),
                            original,
                            "native intentionally repairs missing source metadata inverse"
                        );
                        assert_eq!(state(&source), before);
                        assert!(source.redo().unwrap());
                        assert_eq!(state(&source), actual);
                    }
                    continue;
                }
                _ => unreachable!(),
            }
            assert_eq!(source.serialize().unwrap(), original);
            assert_eq!(state(&source), before);
        }
    }
}
#[test]
fn target_song_and_clock_receive_cross_song_paste_and_source_is_untouched() {
    let mut source = session(Format::Ssc);
    let selected = widgets(&mut source, "both");
    let clip = selected.copy(&mut source).unwrap().unwrap();
    let original_source = source.serialize().unwrap();
    let mut target = session(Format::Sm);
    let mut draft = Draft::capture(&target).unwrap();
    draft.set_music_offset(-0.5).unwrap();
    draft.set_sync_offset(0.031).unwrap();
    draft.set_sample_length(9.0).unwrap();
    draft.apply(&mut target).unwrap();
    let original = target.serialize().unwrap();
    assert!(
        clip.paste_at_fractional_row(&mut target, 0, 48.75, 0)
            .unwrap()
            .changed
    );
    let clock = Clock::capture(&target, 0).unwrap();
    let actual = state(&target);
    assert_eq!(
        actual,
        [
            clock.music_time(49).unwrap(),
            clock.music_time(121).unwrap(),
            9.0
        ]
    );
    assert_eq!(source.serialize().unwrap(), original_source);
    assert!(target.undo().unwrap());
    assert_eq!(target.serialize().unwrap(), original);
    assert!(target.redo().unwrap());
    assert_eq!(state(&target), actual);
    // A second paste snapshots the new target, never the old source Session.
    assert!(
        clip.paste_at_fractional_row(&mut target, 0, 96.5, 0)
            .unwrap()
            .changed
    );
    assert_eq!(state(&target)[0], clock.music_time(96).unwrap());
}
#[test]
fn signed_fractional_widget_origin_offsets_content_and_failures_are_atomic() {
    let mut source = session(Format::Ssc);
    let mut draft = Draft::capture(&source).unwrap();
    draft.set_music_offset(0.0).unwrap();
    draft.set_sample_start(-0.125).unwrap();
    draft.set_last_second_hint(0.0).unwrap();
    draft.apply(&mut source).unwrap();
    let note = Position {
        row: 48,
        lane: 2,
        player: 0,
    };
    source
        .edit_notes(
            0,
            "note",
            [(
                note,
                Some(Note {
                    kind: Kind::Tap,
                    end: None,
                    attributes: String::new().into(),
                    tail_attributes: String::new().into(),
                }),
            )],
        )
        .unwrap();
    let mut selected = widgets(&mut source, "preview");
    let i = selected
        .events()
        .iter()
        .position(|e| *e == Event::Note(note))
        .unwrap();
    selected.set_selected(i, true).unwrap();
    let w = selected
        .events()
        .iter()
        .find_map(|e| match e {
            Event::SongWidget(w) => Some(*w),
            _ => None,
        })
        .unwrap();
    assert!(w.position() < 0.0);
    assert!(w.position().fract() != 0.0);
    let clip = selected.copy(&mut source).unwrap().unwrap();
    let mut target = session(Format::Ssc);
    let original = target.serialize().unwrap();
    assert!(clip.paste(&mut target, 0, 0, 0).unwrap());
    let destination = (48 - w.source_row()) as u64;
    assert!(
        target
            .grid(0)
            .unwrap()
            .get(Position {
                row: destination,
                ..note
            })
            .is_some()
    );
    assert_eq!(
        state(&target)[0],
        Clock::capture(&target, 0).unwrap().music_time(0).unwrap()
    );
    assert!(target.undo().unwrap());
    assert_eq!(target.serialize().unwrap(), original);
    assert!(clip.paste(&mut target, 0, i32::MAX as u64, 0).is_err());
    assert_eq!(target.serialize().unwrap(), original);
    assert!(target.redo().unwrap(), "failed paste preserves redo");
}
#[test]
fn widget_presence_presets_and_stale_identity_have_explicit_contracts() {
    let mut source = session(Format::Ssc);
    let mut d = Draft::capture(&source).unwrap();
    d.set_sample_length(0.0).unwrap();
    d.set_last_second_hint(0.0).unwrap();
    d.apply(&mut source).unwrap();
    let mut selected = widgets(&mut source, "both");
    assert_eq!(
        selected
            .events()
            .iter()
            .filter(|e| matches!(e, Event::SongWidget(_)))
            .count(),
        1,
        "zero-length preview still has a header"
    );
    assert_eq!(
        selected
            .apply_preset(Preset::Miscellaneous, None, Mode::Replace)
            .unwrap()
            .matched,
        selected
            .events()
            .iter()
            .filter(|e| !matches!(e, Event::SongWidget(_) | Event::Note(_)))
            .count()
    );
    let d = Draft::capture(&source).unwrap();
    let mut changed = d;
    changed
        .set_text(
            deadsync_editor::song_text::Field::PreviewMusic,
            "dedicated.ogg",
        )
        .unwrap();
    changed.apply(&mut source).unwrap();
    assert!(selected.copy(&mut source).is_err());
    let selected = widgets(&mut source, "both");
    assert!(
        !selected
            .events()
            .iter()
            .any(|e| matches!(e, Event::SongWidget(_)))
    );
}
