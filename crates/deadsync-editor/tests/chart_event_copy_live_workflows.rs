use deadsync_editor::{
    chart_event_copy::{EventType as T, live::Plan},
    chart_focus::Position,
    document::{Document, Format},
    history::Session,
    timing_edit::live::Anchor,
};
fn session() -> Session {
    Session::new(Document::parse(b"// untouched\n#VERSION:0.83;#MUSIC:main.wav;#OFFSET:0.125;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Focus A;#BPMS:0=120;#SCROLLS:0=1;#LABELS:1=old;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Timing B;#MUSIC:other.wav;#OFFSET:0.75;#BPMS:0=240;#SCROLLS:0=2;#LABELS:2=source;#NOTES:0100;".as_slice(),Format::Ssc).unwrap())
}
#[test]
fn source_b_copies_to_focus_a_without_copying_music_offsets_or_song_timing_choice() {
    for main in [false, true] {
        let mut s = session();
        let original = s.serialize().unwrap();
        let choice = s.song_timing_chart().unwrap();
        let mut draft = deadsync_editor::song_properties::Draft::capture(&s).unwrap();
        draft.set_timing_chart(&s, Some(1)).unwrap();
        let old = Position::at_fractional_row(&s, 0, 120.75).unwrap();
        let anchor = if main {
            Anchor::MusicTime(old.music_time())
        } else {
            Anchor::Row(old.row())
        };
        let plan = Plan::prepare(
            &mut s,
            1,
            [0],
            [T::Tempo, T::Scroll],
            Some((old.clone(), anchor)),
        )
        .unwrap();
        let result = plan.apply(&mut s).unwrap();
        assert!(result.report().changed);
        draft.rebind_chart_event_copy(&s, result.receipt()).unwrap();
        assert_eq!(draft.timing_chart(&s).unwrap(), Some(1));
        assert_eq!(s.song_timing_chart().unwrap(), choice);
        let position = result.into_position().unwrap();
        assert!(position.is_bound_to(&s, 0));
        if main {
            assert_eq!(position.music_time().to_bits(), old.music_time().to_bits());
            assert_ne!(position.row().to_bits(), old.row().to_bits());
        } else {
            assert_eq!(position.row().to_bits(), old.row().to_bits());
            assert_ne!(position.music_time().to_bits(), old.music_time().to_bits());
        }
        assert_eq!(s.document().text(Some(0), "BPMS").as_deref(), Some("0=240"));
        assert_eq!(
            s.document().text(Some(0), "SCROLLS").as_deref(),
            Some("0=2")
        );
        assert_eq!(
            s.document().text(Some(0), "LABELS").as_deref(),
            Some("1=old")
        );
        assert_eq!(s.document().text(Some(0), "MUSIC"), None);
        assert_eq!(s.document().text(Some(0), "OFFSET"), None);
        assert_eq!(s.document().text(None, "OFFSET").as_deref(), Some("0.125"));
        let changed = s.serialize().unwrap();
        assert!(s.history_changes_chart_event_copy(false));
        let reverse = Plan::prepare_history(
            &s,
            Some((
                position.clone(),
                if main {
                    Anchor::MusicTime(position.music_time())
                } else {
                    Anchor::Row(position.row())
                },
            )),
            false,
        )
        .unwrap()
        .apply(&mut s)
        .unwrap();
        assert_eq!(s.serialize().unwrap(), original);
        assert!(s.history_changes_chart_event_copy(true));
        let reverted = reverse.into_position().unwrap();
        assert_eq!(reverted.row().to_bits(), old.row().to_bits());
        Plan::prepare_history(&s, Some((reverted, anchor)), true)
            .unwrap()
            .apply(&mut s)
            .unwrap();
        assert_eq!(s.serialize().unwrap(), changed);
    }
}
#[test]
fn chartless_copy_history_and_invalid_empty_types_preserve_noop_and_exact_bytes() {
    let mut s = session();
    let original = s.serialize().unwrap();
    let revision = s.revision();
    assert!(Plan::prepare(&mut s, 1, [0], [], None).is_err());
    assert!(
        !Plan::prepare(&mut s, 1, [0], [T::Stop], None)
            .unwrap()
            .apply(&mut s)
            .unwrap()
            .report()
            .changed
    );
    assert_eq!(s.revision(), revision);
    assert_eq!(s.serialize().unwrap(), original);
    assert!(!s.history_changes_chart_event_copy(false));
    let result = Plan::prepare(&mut s, 1, [0], [T::Label], None)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert!(result.into_position().is_none());
    let changed = s.serialize().unwrap();
    Plan::prepare_history(&s, None, false)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(s.serialize().unwrap(), original);
    Plan::prepare_history(&s, None, true)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(s.serialize().unwrap(), changed);
}
#[test]
fn plans_reject_edit_undo_aba_and_wrong_session_without_partial_publication() {
    let mut s = session();
    let plan = Plan::prepare(&mut s, 1, [0], [T::Tempo], None).unwrap();
    s.set_text(None, "TITLE", "intervening").unwrap();
    s.undo().unwrap();
    let original = s.serialize().unwrap();
    assert!(plan.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), original);
    let plan = Plan::prepare(&mut s, 1, [0], [T::Tempo], None).unwrap();
    let mut other = session();
    assert!(plan.apply(&mut other).is_err());
    assert_eq!(other.serialize().unwrap(), original);
}
#[test]
fn unfocused_destination_preserves_stop_interior_time_and_chosen_equal_time_warp_row() {
    for warp in [false, true] {
        let pause = if warp { "#WARPS:1=2;" } else { "#STOPS:1=1;" };
        let data = format!(
            "#VERSION:0.83;#OFFSET:0;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=120;{pause}#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=180;#NOTES:0100;#NOTEDATA:;#STEPSTYPE:dance-single;#BPMS:0=90;#NOTES:0010;"
        );
        let mut s = Session::new(Document::parse(data.into_bytes(), Format::Ssc).unwrap());
        let old = if warp {
            Position::at_fractional_row(&s, 0, 96.).unwrap()
        } else {
            Position::at_time(&s, 0, 0.75).unwrap()
        };
        let app = Plan::prepare(
            &mut s,
            1,
            [2],
            [T::Tempo],
            Some((old.clone(), Anchor::MusicTime(old.music_time()))),
        )
        .unwrap()
        .apply(&mut s)
        .unwrap();
        assert!(app.report().changed);
        let new = app.into_position().unwrap();
        assert!(new.is_bound_to(&s, 0));
        assert_eq!(new.row().to_bits(), old.row().to_bits());
        assert_eq!(new.music_time().to_bits(), old.music_time().to_bits());
        assert_eq!(new.chart_time().to_bits(), old.chart_time().to_bits());
    }
}

#[test]
fn divergent_or_nonfinite_anchor_payloads_are_refused_before_copy_and_history() {
    for row_anchor in [false, true] {
        let mut s = session();
        let old = Position::at_fractional_row(&s, 0, 120.75).unwrap();
        let current = if row_anchor {
            old.row()
        } else {
            old.music_time()
        };
        let anchor = |value| {
            if row_anchor {
                Anchor::Row(value)
            } else {
                Anchor::MusicTime(value)
            }
        };
        let original = s.serialize().unwrap();
        let revision = s.revision();
        let undo = s.undo_label().map(str::to_owned);
        let redo = s.redo_label().map(str::to_owned);
        for value in [current + 1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                Plan::prepare(
                    &mut s,
                    1,
                    [0],
                    [T::Tempo],
                    Some((old.clone(), anchor(value)))
                )
                .is_err()
            );
            assert_eq!(s.serialize().unwrap(), original);
            assert_eq!(s.revision(), revision);
            assert_eq!(s.undo_label().map(str::to_owned), undo);
            assert_eq!(s.redo_label().map(str::to_owned), redo);
        }
        let applied = Plan::prepare(
            &mut s,
            1,
            [0],
            [T::Tempo],
            Some((old.clone(), anchor(current))),
        )
        .unwrap()
        .apply(&mut s)
        .unwrap();
        let mut focus = applied.into_position().unwrap();
        let changed = s.serialize().unwrap();
        for redo in [false, true] {
            let current = if row_anchor {
                focus.row()
            } else {
                focus.music_time()
            };
            let bytes = s.serialize().unwrap();
            let revision = s.revision();
            let undo_label = s.undo_label().map(str::to_owned);
            let redo_label = s.redo_label().map(str::to_owned);
            for value in [current + 1., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                assert!(
                    Plan::prepare_history(&s, Some((focus.clone(), anchor(value))), redo).is_err()
                );
                assert_eq!(s.serialize().unwrap(), bytes);
                assert_eq!(s.revision(), revision);
                assert_eq!(s.undo_label().map(str::to_owned), undo_label);
                assert_eq!(s.redo_label().map(str::to_owned), redo_label);
            }
            // Refusal has not consumed the entry; the exact current anchor
            // remains accepted and retains the established original bytes.
            focus = Plan::prepare_history(&s, Some((focus, anchor(current))), redo)
                .unwrap()
                .apply(&mut s)
                .unwrap()
                .into_position()
                .unwrap();
            assert_eq!(
                s.serialize().unwrap(),
                if redo {
                    changed.clone()
                } else {
                    original.clone()
                }
            );
        }
    }
}
