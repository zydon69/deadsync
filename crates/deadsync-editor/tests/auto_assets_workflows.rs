use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    song_assets::{self, Assets},
    song_properties::Draft,
    song_text::Field,
};
use std::path::Path;
#[cfg(unix)]
use std::collections::BTreeSet;
const FIELDS: [(Field, &str); 7] = [
    (Field::Background, "BackgroundPath"),
    (Field::Banner, "BannerPath"),
    (Field::CdTitle, "CDTitlePath"),
    (Field::Jacket, "JacketPath"),
    (Field::CdImage, "CDImagePath"),
    (Field::DiscImage, "DiscImagePath"),
    (Field::Lyrics, "LyricsPath"),
];
const FILES: [&str; 7] = [
    "background.png",
    "banner.png",
    "cdtitle.png",
    "jacket.png",
    "cover-cd.png",
    "cover disc.png",
    "words.lrc",
];
fn assets(directory: &Path, mode: &str) -> Assets {
    if !["empty", "missing", "links"].contains(&mode) {
        std::fs::create_dir_all(directory).unwrap();
        for name in FILES {
            std::fs::write(directory.join(name), []).unwrap();
        }
    } else if mode != "missing" {
        std::fs::create_dir_all(directory).unwrap();
    }
    #[cfg(unix)]
    if mode == "links" {
        let root = directory.parent().unwrap();
        let target_dir = root.join("target-dir");
        std::fs::create_dir(&target_dir).unwrap();
        let target_file = root.join("target.dat");
        std::fs::write(&target_file, []).unwrap();
        std::os::unix::fs::symlink(root.join("missing.dat"), directory.join("background.png"))
            .unwrap();
        std::os::unix::fs::symlink(&target_file, directory.join("cdtitle.png")).unwrap();
        std::os::unix::fs::symlink(&target_dir, directory.join("banner.png")).unwrap();
    }
    let files = song_assets::files(directory).unwrap_or_default();
    Assets::discover(directory, &files, |_| Some((1, 1)))
}
#[test]
fn original_auto_assets_actions_match_exact_updates_noop_and_one_undo_in_sm_and_ssc() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/auto-assets.expected.json")).unwrap();
    assert_eq!(cases.as_array().unwrap().len(), 8);
    for case in cases.as_array().unwrap() {
        let name = case["Name"].as_str().unwrap();
        if name == "links" && !cfg!(unix) {
            continue;
        }
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("song");
        let discovered = assets(&directory, name);
        for format in [Format::Sm, Format::Ssc] {
            let mut source = String::from("#VERSION:0.83;#TITLE:Keep;#Opaque:unrelated;");
            for (field, property) in FIELDS {
                source.push_str(&format!(
                    "#{}:{};",
                    field.tag(),
                    case["Before"][property].as_str().unwrap()
                ));
            }
            let mut session = Session::new(Document::parse(source.as_bytes(), format).unwrap());
            let before = session.serialize().unwrap();
            let changed = if case["CanEdit"].as_bool().unwrap() {
                discovered
                    .apply(&mut session, case["OnlyUnset"].as_bool().unwrap())
                    .unwrap()
            } else {
                0
            };
            let draft = Draft::capture(&session).unwrap();
            let expected = FIELDS
                .iter()
                .filter(|(_, key)| case["After"][*key] != case["Before"][*key])
                .count();
            assert_eq!(changed, expected, "{name} {format:?}");
            for (field, key) in FIELDS {
                assert_eq!(
                    draft.text(field),
                    case["After"][key].as_str().unwrap(),
                    "{name} {format:?} {key}"
                );
            }
            assert_eq!(
                session.document().text(None, "Opaque").as_deref(),
                Some("unrelated")
            );
            let after = session.serialize().unwrap();
            assert_eq!(
                session.undo_label().is_some(),
                case["Undo"].is_string(),
                "{name} {format:?}"
            );
            if changed != 0 {
                session.undo().unwrap();
                assert_eq!(session.serialize().unwrap(), before);
                assert!(session.undo_label().is_none());
                session.redo().unwrap();
                assert_eq!(session.serialize().unwrap(), after);
            } else {
                assert_eq!(after, before);
                assert!(!session.is_dirty());
            }
        }
    }
}
#[test]
fn discovery_never_clears_absent_paths_and_invalid_candidate_set_is_atomic() {
    let source = b"#VERSION:0.83;#BACKGROUND:old.png;#LYRICSPATH:old.lrc;#Opaque:raw;";
    let mut session = Session::new(Document::parse(source.as_slice(), Format::Ssc).unwrap());
    let before = session.serialize().unwrap();
    assert_eq!(Assets::default().apply(&mut session, false).unwrap(), 0);
    let invalid = Assets {
        images: vec![(song_assets::ImageType::Background, "new.png".into())],
        lyrics: Some("bad\npath.lrc".into()),
    };
    assert!(invalid.apply(&mut session, false).is_err());
    assert_eq!(session.serialize().unwrap(), before);
    assert!(session.undo_label().is_none());
}
#[test]
#[cfg(unix)]
fn filesystem_file_inventory_matches_real_dotnet_broken_and_directory_link_case() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("song");
    let _ = assets(&directory, "links");
    let names: BTreeSet<_> = song_assets::files(&directory)
        .unwrap()
        .into_iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/auto-assets.expected.json")).unwrap();
    let case = cases
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["Name"] == "links")
        .unwrap();
    let expected: BTreeSet<_> = case["Inventory"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(names, expected);
}
