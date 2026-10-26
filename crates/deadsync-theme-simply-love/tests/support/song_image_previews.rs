#[test]
fn image_preview_requests_follow_draft_and_cancel_without_source_changes() {
    use deadsync_theme_simply_love::screens::edition::image_previews::{PreviewView, Status};
    use std::sync::Arc;
    let (dir, mut state, original) = setup(Format::Ssc);
    assert!(state.image_preview_request().is_none());
    open(&mut state);
    let request = state.image_preview_request().unwrap();
    assert!(request.paths[0].as_ref().unwrap().ends_with("old.png"));
    let mut current = 0;
    field(&mut state, &mut current, Field::Banner);
    let selected = dir.path().join("new-banner.png");
    fs::write(&selected, b"preview fixture").unwrap();
    let mut picker = Choose {
        kind: Kind::Image,
        path: Some(selected.canonicalize().unwrap()),
        calls: 0,
    };
    press(&mut state, KeyCode::F2);
    assert!(state.process_music_picker(&mut picker));
    let staged = state.image_preview_request().unwrap();
    assert_eq!(staged.document_identity, request.document_identity);
    assert!(
        staged.paths[0]
            .as_ref()
            .unwrap()
            .ends_with("new-banner.png")
    );
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original
    );
    state.set_image_preview_views(std::array::from_fn(|index| PreviewView {
        status: Status::Ready,
        key: Arc::from(if index == 0 {
            "__editor_song_preview::banner"
        } else {
            "__editor_song_preview::cdtitle"
        }),
        source_size: Some([200, 80]),
        has_resident_image: true,
    }));
    let mut actors = Vec::new();
    state.actors(&mut actors);
    assert!(!actors.is_empty());
    press(&mut state, KeyCode::Escape);
    assert!(state.image_preview_request().is_none());
    assert_eq!(
        state.workspace().file().session.serialize().unwrap(),
        original
    );
    // SongText returns to the parent properties draft. Discard that complete
    // draft before using F11 to open a fresh properties panel.
    press(&mut state, KeyCode::Escape);
    open(&mut state);
    assert!(
        state.image_preview_request().unwrap().paths[0]
            .as_ref()
            .unwrap()
            .ends_with("old.png")
    );
    assert_eq!(fs::read(&selected).unwrap(), b"preview fixture");
}

#[test]
fn image_preview_feedback_is_localized_and_header_does_not_overlap_fields() {
    use deadlib_present::actors::{SizeSpec, SpriteSource};
    use deadsync_theme_simply_love::screens::edition::image_previews::{PreviewView, Status};
    use std::sync::Arc;
    let (_dir, mut state, _) = setup(Format::Ssc);
    open(&mut state);
    for (status, key) in [
        (Status::Empty, "ImagePreviewEmpty"),
        (Status::Loading, "ImagePreviewLoading"),
        (Status::Missing, "ImagePreviewMissing"),
        (Status::UnsupportedVideo, "ImagePreviewVideo"),
        (
            Status::Failed("private decoder details".into()),
            "ImagePreviewFailed",
        ),
    ] {
        state.set_image_preview_views(std::array::from_fn(|index| PreviewView {
            status: status.clone(),
            key: Arc::from(if index == 0 {
                "__editor_song_preview::banner"
            } else {
                "__editor_song_preview::cdtitle"
            }),
            source_size: None,
            has_resident_image: false,
        }));
        let mut actors = Vec::new();
        state.actors(&mut actors);
        let expected = i18n::tr("Edition", key);
        assert_ne!(expected.as_ref(), key, "translation must exist");
        assert!(actors.iter().any(
            |actor| matches!(actor, Actor::Text {content,..} if content.contains(expected.as_ref()))
        ));
        assert!(!actors.iter().any(|actor| matches!(actor, Actor::Text {content,..} if content.contains("private decoder details"))));
    }
    state.set_image_preview_views(std::array::from_fn(|index| PreviewView {
        status: Status::Ready,
        key: Arc::from(if index == 0 {
            "__editor_song_preview::banner"
        } else {
            "__editor_song_preview::cdtitle"
        }),
        source_size: Some([200, 80]),
        has_resident_image: true,
    }));
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let first_field_y = actors
        .iter()
        .find_map(|actor| match actor {
            Actor::Text {
                content, offset, ..
            } if content.contains("Source") => Some(offset[1]),
            _ => None,
        })
        .expect("first title field remains visible");
    let mut previews = 0;
    for actor in &actors {
        if let Actor::Sprite {
            source: SpriteSource::Texture(key),
            size: [SizeSpec::Px(w), SizeSpec::Px(h)],
            offset,
            ..
        } = actor
            && key.starts_with("__editor_song_preview::")
        {
            previews += 1;
            assert!(
                (w / h - 2.5).abs() < 0.001,
                "image aspect ratio is preserved"
            );
            assert!(offset[1] - h * 0.5 >= 0.0);
            assert!(
                offset[1] + h * 0.5 + 20.0 < first_field_y,
                "preview title and first field do not overlap"
            );
        }
    }
    assert_eq!(previews, 2);
    // Both media fields remain reachable through the existing keyboard traversal.
    let mut current = 0;
    field(&mut state, &mut current, Field::Banner);
    field(&mut state, &mut current, Field::CdTitle);
    assert!(state.wants_text_input());
}
