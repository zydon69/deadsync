use deadsync_editor::{
    chart_focus::Position,
    document::{Document, Format},
    history::Session,
    song_properties::{Draft, Selectable},
    song_text::Field,
};
fn session(format: Format) -> Session {
    let bytes = if format == Format::Sm {
        b"#MUSIC:main.wav;#OFFSET:0;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#STOPS:1=1;#VENDOR:opaque;#NOTES:dance-single::Hard:9::1000;".as_slice()
    } else {
        b"#VERSION:0.83;#MUSIC:main.wav;#OFFSET:0;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#BPMS:0=120;#STOPS:1=1;#VENDOR:opaque;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice()
    };
    Session::new(Document::parse(bytes, format).unwrap())
}
fn change(draft: &mut Draft, kind: usize) {
    match kind {
        0 => draft.set_text(Field::Title, "Title exact").unwrap(),
        1 => draft.set_text(Field::Lyrics, "lyrics/Été.lrc").unwrap(),
        2 => draft.set_text(Field::PreviewMusic, "preview.wav").unwrap(),
        3 => {
            draft.set_sample_start(1.25).unwrap();
            draft.set_sample_length(2.375).unwrap();
        }
        4 => draft.set_last_second_hint(9.375).unwrap(),
        _ => draft.set_selectable(Selectable::ALL[1]),
    }
}
#[test]
fn typed_metadata_apply_undo_redo_preserve_exact_stop_interior_and_signed_rows_sm_ssc() {
    for format in [Format::Sm, Format::Ssc] {
        for kind in 0..6 {
            for time in [-0.125, 0.75] {
                let mut s = session(format);
                let original = s.serialize().unwrap();
                let focus = Position::at_time(&s, 0, time).unwrap();
                let coordinate = (
                    focus.row().to_bits(),
                    focus.music_time().to_bits(),
                    focus.chart_time().to_bits(),
                );
                let mut draft = Draft::capture(&s).unwrap();
                change(&mut draft, kind);
                assert!(!draft.requires_live_handoff());
                let result = draft.apply_with_presentation_receipt(&mut s).unwrap();
                assert!(result.changed());
                let receipt = result.presentation_receipt().unwrap();
                assert!(receipt.preserves_audio());
                let mut focus = focus.rebind_song_properties(&s, receipt).unwrap();
                let after = s.serialize().unwrap();
                assert!(s.history_preserves_song_properties_metadata(false));
                assert!(!s.history_changes_song_properties_live(false));
                for redo in [false, true] {
                    let result = s.apply_song_properties_metadata_history(redo).unwrap();
                    assert!(result.presentation_receipt().unwrap().preserves_audio());
                    focus = focus
                        .rebind_song_properties(&s, result.presentation_receipt().unwrap())
                        .unwrap();
                    assert_eq!(
                        (
                            focus.row().to_bits(),
                            focus.music_time().to_bits(),
                            focus.chart_time().to_bits()
                        ),
                        coordinate
                    );
                    assert!(focus.is_bound_to(&s, 0));
                    assert_eq!(
                        s.serialize().unwrap(),
                        if redo {
                            after.clone()
                        } else {
                            original.clone()
                        }
                    );
                }
                assert_eq!(s.document().text(None, "VENDOR").as_deref(), Some("opaque"));
            }
        }
    }
}
#[test]
fn auto_assets_real_owner_is_typed_and_empty_stale_or_foreign_history_is_atomic() {
    use deadsync_editor::song_assets::{Assets, ImageType};
    let mut s = session(Format::Ssc);
    let original = s.serialize().unwrap();
    let assets = Assets {
        images: vec![(ImageType::Banner, "banner.png".into())],
        lyrics: Some("song.lrc".into()),
    };
    assert_eq!(assets.apply(&mut s, false).unwrap(), 2);
    assert!(s.history_preserves_song_properties_metadata(false));
    let after = s.serialize().unwrap();
    s.apply_song_properties_metadata_history(false).unwrap();
    assert_eq!(s.serialize().unwrap(), original);
    let revision = s.revision();
    let redo = s.redo_label().map(str::to_owned);
    let draft = Draft::capture(&s).unwrap();
    assert!(!draft.apply(&mut s).unwrap());
    assert_eq!(s.revision(), revision);
    assert_eq!(s.redo_label(), redo.as_deref());
    s.apply_song_properties_metadata_history(true).unwrap();
    assert_eq!(s.serialize().unwrap(), after);
    let stale = Draft::capture(&s).unwrap();
    s.set_text(None, "TITLE", "Change song properties").unwrap();
    let bytes = s.serialize().unwrap();
    let revision = s.revision();
    assert!(!s.history_preserves_song_properties_metadata(false));
    assert!(s.apply_song_properties_metadata_history(false).is_err());
    assert!(stale.apply(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), bytes);
    assert_eq!(s.revision(), revision);
    let mut live = Draft::capture(&s).unwrap();
    live.set_music_offset(0.25).unwrap();
    live.apply(&mut s).unwrap();
    assert!(s.history_changes_song_properties_live(false));
    assert!(!s.history_preserves_song_properties_metadata(false));
    assert!(s.apply_song_properties_metadata_history(false).is_err());
}
#[test]
fn metadata_media_history_survives_save_as_and_recovery_including_authored_sm_preview() {
    use deadsync_editor::{song_text::resident, storage::FileSession};
    for format in [Format::Sm, Format::Ssc] {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().canonicalize().unwrap();
        let old = base.join("old");
        let new = base.join("new");
        std::fs::create_dir(&old).unwrap();
        std::fs::create_dir(&new).unwrap();
        for name in ["main.wav", "preview.wav", "lyrics.lrc"] {
            std::fs::write(old.join(name), b"asset").unwrap();
        }
        let ext = if format == Format::Sm { "sm" } else { "ssc" };
        let path = old.join(format!("song.{ext}"));
        std::fs::write(&path, session(format).serialize().unwrap()).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        let mut draft = Draft::capture(&file.session).unwrap();
        draft.set_text(Field::PreviewMusic, "preview.wav").unwrap();
        draft.set_text(Field::Lyrics, "lyrics.lrc").unwrap();
        draft
            .apply_with_presentation_receipt(&mut file.session)
            .unwrap();
        let destination = new.join(format!("copy.{ext}"));
        let review = file.prepare_save_as(&destination).unwrap();
        file.save_as(review).unwrap();
        for redo in [false, true] {
            file.session
                .apply_song_properties_metadata_history(redo)
                .unwrap();
            let draft = Draft::capture(&file.session).unwrap();
            if redo {
                assert_eq!(
                    new.join(draft.text(Field::PreviewMusic))
                        .canonicalize()
                        .unwrap(),
                    old.join("preview.wav").canonicalize().unwrap()
                );
                assert_eq!(
                    new.join(draft.text(Field::Lyrics)).canonicalize().unwrap(),
                    old.join("lyrics.lrc").canonicalize().unwrap()
                );
            } else {
                assert!(draft.text(Field::PreviewMusic).is_empty());
                assert!(draft.text(Field::Lyrics).is_empty());
            }
        }
        let bytes = file.session.serialize().unwrap();
        file.save_recovery().unwrap();
        drop(file);
        let mut recovered = FileSession::open(&destination).unwrap();
        recovered.recover().unwrap();
        assert_eq!(recovered.session.serialize().unwrap(), bytes);
        assert!(
            !resident::effective(recovered.session.document())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            recovered.session.document().text(None, "VENDOR").as_deref(),
            Some("opaque")
        );
    }
}
#[test]
fn pure_song_timing_choice_keeps_its_existing_typed_producer() {
    let mut s = session(Format::Ssc);
    let mut draft = Draft::capture(&s).unwrap();
    let target = if draft.timing_chart(&s).unwrap().is_some() {
        None
    } else {
        Some(0)
    };
    assert!(draft.set_timing_chart(&s, target).unwrap());
    assert!(!draft.requires_live_handoff());
    assert!(draft.apply(&mut s).unwrap());
    assert!(s.song_timing_history_preserves_presentation(false));
    assert!(!s.history_preserves_song_properties_metadata(false));
    let result = s.apply_song_timing_history(false).unwrap();
    assert!(result.presentation_receipt().unwrap().preserves_audio());
}
#[test]
fn song_widget_paste_keeps_composable_primitive_and_remains_one_atomic_history_entry() {
    use deadsync_editor::event_clipboard::{Event, Selection};
    for format in [Format::Sm, Format::Ssc] {
        let mut s = session(format);
        let mut draft = Draft::capture(&s).unwrap();
        draft.set_sample_start(0.25).unwrap();
        draft.set_sample_length(2.).unwrap();
        draft.apply(&mut s).unwrap();
        assert!(
            !s.history_preserves_song_properties_metadata(false),
            "raw draft primitive must remain composable inside clipboard transactions"
        );
        let mut selection = Selection::capture(&mut s, 0, 0, i32::MAX as u64).unwrap();
        selection.select_all(false);
        selection.include_song_widgets(&s).unwrap();
        for index in 0..selection.events().len() {
            if matches!(selection.events()[index], Event::SongWidget(_)) {
                selection.set_selected(index, true).unwrap();
            }
        }
        let clip = selection.copy(&mut s).unwrap().unwrap();
        let before = s.serialize().unwrap();
        assert!(
            clip.paste_at_fractional_row(&mut s, 0, 192., 0)
                .unwrap()
                .changed
        );
        let after = s.serialize().unwrap();
        assert_ne!(after, before);
        assert!(
            !s.history_preserves_song_properties_metadata(false),
            "compound paste is its own closed producer"
        );
        s.undo().unwrap();
        assert_eq!(s.serialize().unwrap(), before);
        s.redo().unwrap();
        assert_eq!(s.serialize().unwrap(), after);
    }
}
