// Included after song_assets.rs; reuses its public-input helpers and fixtures.
fn batch_media_fixtures(directory: &std::path::Path) -> Vec<(Field, &'static str)> {
    let candidates = vec![
        (Field::Background, "background.png"),
        (Field::Banner, "banner.png"),
        (Field::CdTitle, "cdtitle.png"),
        (Field::Jacket, "jk_album.png"),
        (Field::CdImage, "album-cd.png"),
        (Field::DiscImage, "album disc.png"),
        (Field::Lyrics, "words.lrc"),
    ];
    for &(target, name) in &candidates {
        if target == Field::Lyrics {
            fs::write(directory.join(name), b"[00:01.00]Words").unwrap();
        } else {
            image::RgbImage::new(2, 2)
                .save(directory.join(name))
                .unwrap();
        }
    }
    candidates
}
fn batch_apply_from_properties_text_return(state: &mut State) {
    // Escape from song text returns to the text-entry action (index 3).
    press(state, KeyCode::Escape);
    for _ in 0..4 {
        press(state, KeyCode::ArrowDown);
    }
    press(state, KeyCode::Enter);
}
#[test]
fn batch_search_respects_pending_paths_and_overwrite_is_one_reversible_saved_change() {
    for format in [Format::Sm, Format::Ssc] {
        for overwrite in [false, true] {
            let (dir, mut state, original) = setup(format);
            let candidates = batch_media_fixtures(dir.path());
            let media_bytes: Vec<_> = candidates
                .iter()
                .map(|(_, name)| (*name, fs::read(dir.path().join(name)).unwrap()))
                .collect();
            open(&mut state);
            // The batch must use the current draft, not reread existing tags.
            assert!(state.text_input("Pending title"));
            let mut current = 0;
            field(&mut state, &mut current, Field::Banner);
            assert!(state.text_input("pending-choice.png"));
            press(&mut state, KeyCode::Escape);
            if overwrite {
                key(&mut state, KeyCode::ShiftLeft, true);
            }
            press(&mut state, KeyCode::F4);
            if overwrite {
                key(&mut state, KeyCode::ShiftLeft, false);
            }
            assert_eq!(
                state.workspace().file().session.serialize().unwrap(),
                original
            );
            assert!(!state.workspace().file().session.is_dirty());
            let count = if format == Format::Sm { 3 } else { 6 } + usize::from(overwrite);
            let mut actors = Vec::new();
            state.actors(&mut actors);
            let staged = if LANGUAGE == "fr" {
                format!("{count} chemins de médias préparés")
            } else {
                format!("{count} media paths staged")
            };
            assert!(actors.iter().any(
                |actor| matches!(actor, Actor::Text {content,..} if content.contains(&staged))
            ));
            // Open text again to retain the shared apply-navigation helper.
            press(&mut state, KeyCode::Enter);
            batch_apply_from_properties_text_return(&mut state);
            let changed = state.workspace().file().session.serialize().unwrap();
            assert_ne!(changed, original);
            let text = Text::read(state.workspace().file().session.document());
            assert_eq!(text.get(Field::Title), "Pending title");
            for &(target, name) in &candidates {
                if text.supports(target) {
                    assert_eq!(
                        text.get(target),
                        if target == Field::Banner && !overwrite {
                            "pending-choice.png"
                        } else {
                            name
                        }
                    );
                }
            }
            if format == Format::Sm {
                assert_eq!(text.get(Field::Jacket), "legacy.png");
                assert_eq!(text.get(Field::CdImage), "");
                assert_eq!(text.get(Field::DiscImage), "");
            }
            ctrl(&mut state, KeyCode::KeyZ);
            assert_eq!(
                state.workspace().file().session.serialize().unwrap(),
                original
            );
            ctrl(&mut state, KeyCode::KeyY);
            assert_eq!(
                state.workspace().file().session.serialize().unwrap(),
                changed
            );
            ctrl(&mut state, KeyCode::KeyS);
            let reopened = FileSession::open(state.workspace().file().path()).unwrap();
            assert_eq!(reopened.session.serialize().unwrap(), changed);
            for (name, bytes) in media_bytes {
                assert_eq!(fs::read(dir.path().join(name)).unwrap(), bytes);
            }
        }
    }
}
#[test]
fn cancelling_batch_discovery_discards_all_paths_and_reopening_reads_original_values() {
    for format in [Format::Sm, Format::Ssc] {
        let (dir, mut state, original) = setup(format);
        batch_media_fixtures(dir.path());
        press(&mut state, KeyCode::F11);
        key(&mut state, KeyCode::ShiftLeft, true);
        press(&mut state, KeyCode::F4);
        key(&mut state, KeyCode::ShiftLeft, false);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original
        );
        // Select the explicit Cancel action from the initial index 0.
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::ArrowDown);
        press(&mut state, KeyCode::Enter);
        assert!(!state.workspace().file().session.is_dirty());
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original
        );
        open(&mut state);
        let mut current = 0;
        field(&mut state, &mut current, Field::Banner);
        let mut actors = Vec::new();
        state.actors(&mut actors);
        assert!(actors.iter().any(
            |actor| matches!(actor, Actor::Text {content,..} if content.contains("[old.png]|"))
        ));
        // Reapplying a fresh, untouched draft must remain a no-op.
        batch_apply_from_properties_text_return(&mut state);
        assert_eq!(
            state.workspace().file().session.serialize().unwrap(),
            original
        );
        assert!(!state.workspace().file().session.is_dirty());
    }
}
