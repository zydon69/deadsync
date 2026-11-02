use deadsync_editor::{
    document::{Document, Format},
    song_assets::{self, Assets, ImageType as I},
    song_text::{Field, Text},
};
use std::path::{Path, PathBuf};
fn paths(names: &[&str]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|name| Path::new("/songs/example").join(name))
        .collect()
}
fn image(kind: I, entries: &[(&str, Option<(i32, i32)>)]) -> Option<String> {
    let files = paths(&entries.iter().map(|entry| entry.0).collect::<Vec<_>>());
    song_assets::find_image(kind, Path::new("/songs/example"), &files, |path| {
        entries
            .iter()
            .find(|entry| path.file_name().unwrap() == entry.0)
            .unwrap()
            .1
    })
}
#[test]
fn named_candidates_precede_every_dimension_and_preserve_extension_case() {
    assert_eq!(
        image(
            I::Background,
            &[("exact.png", Some((640, 480))), ("MY_BACKGROUND.png", None)]
        )
        .as_deref(),
        Some("MY_BACKGROUND.png")
    );
    for (kind, name) in [
        (I::Background, "testbg.png"),
        (I::Banner, "test bn.jpg"),
        (I::CdTitle, "CDTITLE-foo.png"),
        (I::Jacket, "jk_test.png"),
        (I::Jacket, "test ALBUMART.png"),
        (I::CdImage, "test-cd.png"),
        (I::DiscImage, "test title.png"),
        (I::DiscImage, "test disc.png"),
    ] {
        assert_eq!(image(kind, &[(name, None)]).as_deref(), Some(name));
    }
    assert_eq!(image(I::Banner, &[("banner.PNG", Some((418, 164)))]), None);
    assert_eq!(image(I::Banner, &[("banner.mp4", None)]), None);
    assert_eq!(
        image(I::Background, &[("background.mp4", None)]).as_deref(),
        Some("background.mp4")
    );
}
#[test]
fn dimensions_prioritize_exact_then_first_ratio_then_biggest_then_video() {
    assert_eq!(
        image(
            I::Background,
            &[
                ("ratio.png", Some((320, 240))),
                ("exact.png", Some((640, 480)))
            ]
        )
        .as_deref(),
        Some("exact.png")
    );
    assert_eq!(
        image(
            I::Background,
            &[
                ("small.png", Some((32, 24))),
                ("large.png", Some((1280, 960)))
            ]
        )
        .as_deref(),
        Some("small.png")
    );
    assert_eq!(
        image(
            I::Background,
            &[
                ("movie.mp4", None),
                ("wide.png", Some((1000, 600))),
                ("bigger.png", Some((2000, 1200)))
            ]
        )
        .as_deref(),
        Some("bigger.png")
    );
    assert_eq!(
        image(
            I::Background,
            &[("too-small.png", Some((639, 400))), ("movie.webm", None)]
        )
        .as_deref(),
        Some("movie.webm")
    );
    assert_eq!(
        image(I::Banner, &[("square.png", Some((2000, 2000)))]),
        None
    );
    assert_eq!(
        image(
            I::Banner,
            &[
                ("ratio.png", Some((209, 82))),
                ("exact.png", Some((418, 164)))
            ]
        )
        .as_deref(),
        Some("exact.png")
    );
    assert_eq!(
        image(
            I::Background,
            &[("a.png", Some((1000, 600))), ("b.png", Some((1200, 500)))]
        )
        .as_deref(),
        Some("a.png")
    );
    assert_eq!(
        image(
            I::Background,
            &[
                ("too-wide.png", Some((2_000_000_000, 2_000_000_000))),
                ("movie.mp4", None)
            ]
        )
        .as_deref(),
        Some("movie.mp4")
    );
}
#[test]
fn classification_checks_category_order_and_defaults_to_background() {
    assert_eq!(
        song_assets::best_image_type(Path::new("jacket.png"), |_| Some((640, 480))),
        I::Background
    );
    assert_eq!(
        song_assets::best_image_type(Path::new("jacket.png"), |_| Some((418, 164))),
        I::Banner
    );
    assert_eq!(
        song_assets::best_image_type(Path::new("jacket.png"), |_| Some((500, 500))),
        I::Jacket
    );
    assert_eq!(
        song_assets::best_image_type(Path::new("unknown.png"), |_| None),
        I::Background
    );
    assert_eq!(
        song_assets::best_image_type(Path::new("background-banner.png"), |_| None),
        I::Background
    );
}
#[test]
fn batch_reuses_candidates_and_lyrics_uses_first_exact_lowercase_extension() {
    let files = paths(&[
        "background-banner.png",
        "subtitles.LRC",
        "Été.lrc",
        "second.lrc",
    ]);
    let assets = Assets::discover(Path::new("/songs/example"), &files, |_| None);
    assert_eq!(
        assets.images,
        vec![
            (I::Background, "background-banner.png".into()),
            (I::Banner, "background-banner.png".into())
        ]
    );
    assert_eq!(assets.lyrics.as_deref(), Some("Été.lrc"));
}
#[test]
fn batch_stages_only_candidates_respects_unset_in_both_formats_and_validates_atomically() {
    let assets = Assets {
        images: vec![
            (I::Background, "new.png".into()),
            (I::Jacket, "jacket.png".into()),
        ],
        lyrics: Some("words.lrc".into()),
    };
    for format in [Format::Sm, Format::Ssc] {
        let document = Document::parse(
            b"#BACKGROUND:old.png;#JACKET:legacy.png;#TITLE:Keep;".to_vec(),
            format,
        )
        .unwrap();
        let mut text = Text::read(&document);
        assert_eq!(assets.stage(&mut text, true).unwrap(), 1);
        assert_eq!(text.get(Field::Background), "old.png");
        assert_eq!(text.get(Field::Jacket), "legacy.png");
        assert_eq!(assets.stage(&mut text, false).unwrap(), 2);
        assert_eq!(text.get(Field::Background), "new.png");
        // EditorSong stages all image paths before the selected writer decides
        // their output representation, including Jacket for an SM document.
        assert_eq!(text.get(Field::Jacket), "jacket.png");
        assert_eq!(text.get(Field::Lyrics), "words.lrc");
        assert_eq!(assets.stage(&mut text, false).unwrap(), 0);
        let invalid = Assets {
            images: vec![(I::Background, "other.png".into())],
            lyrics: Some("bad\n.lrc".into()),
        };
        assert!(invalid.stage(&mut text, false).is_err());
        assert_eq!(text.get(Field::Background), "new.png");
        assert_eq!(text.get(Field::Jacket), "jacket.png");
        assert_eq!(text.get(Field::Lyrics), "words.lrc");
        assert_eq!(text.get(Field::Title), "Keep");
    }
}
#[test]
fn filesystem_inventory_is_direct_and_does_not_modify_assets() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("background.png"), b"not decoded").unwrap();
    std::fs::create_dir(dir.path().join("nested")).unwrap();
    std::fs::write(dir.path().join("nested/banner.png"), b"child").unwrap();
    let files = song_assets::files(dir.path()).unwrap();
    assert_eq!(files, vec![dir.path().join("background.png")]);
    assert_eq!(
        Assets::discover(dir.path(), &files, |_| None).images,
        vec![(I::Background, "background.png".into())]
    );
    assert_eq!(std::fs::read(&files[0]).unwrap(), b"not decoded");
    assert!(song_assets::files(&dir.path().join("missing")).is_err());
}

/// Selection/classification differential against the unchanged pinned C# class.
/// Codec dimensions are injected on both sides; image decoding is not covered.
#[test]
fn selection_and_classification_match_pinned_grooveauthor_source() {
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/grooveauthor/song-assets-discovery.json"),
        )
        .expect("generate source oracle fixture with scripts/editor-song-assets-oracle.py"),
    )
    .unwrap();
    assert_eq!(
        fixture["Reference"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    let cases = fixture["Cases"].as_array().unwrap();
    assert_eq!(cases.len(), 114);
    for (index, case) in cases.iter().enumerate() {
        let entries = case["Files"].as_array().unwrap();
        let files = paths(
            &entries
                .iter()
                .map(|entry| entry["Name"].as_str().unwrap())
                .collect::<Vec<_>>(),
        );
        let probe = |path: &Path| {
            let entry = entries
                .iter()
                .find(|entry| path.file_name().unwrap() == entry["Name"].as_str().unwrap())
                .unwrap();
            entry["Width"]
                .as_i64()
                .zip(entry["Height"].as_i64())
                .map(|(w, h)| (w as i32, h as i32))
        };
        let assets = Assets::discover(Path::new("/songs/example"), &files, probe);
        fn reference_name(kind: I) -> &'static str {
            match kind {
                I::Background => "Background",
                I::Banner => "Banner",
                I::CdTitle => "CDTitle",
                I::Jacket => "Jacket",
                I::CdImage => "CDImage",
                I::DiscImage => "DiscImage",
            }
        }
        let actual = serde_json::json!({
            "Images":assets.images.into_iter().map(|(kind,path)|(reference_name(kind).to_string(),serde_json::Value::String(path))).collect::<serde_json::Map<_,_>>(),
            "Lyrics":assets.lyrics,
            "Types":files.iter().map(|path|reference_name(song_assets::best_image_type(path,probe))).collect::<Vec<_>>()
        });
        assert_eq!(actual, case["Expected"], "source oracle case {index}");
    }
}
