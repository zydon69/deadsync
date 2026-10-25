use deadsync_editor::{
    chart_properties::Draft,
    chart_text::{Field, Text},
    document::{Document, Format},
    history::Session,
    music_file,
    playback::Preview,
    storage::FileSession,
};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};
fn fixtures(prefix: &str) -> Vec<(Value, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    fs::read_dir(&root)
        .unwrap()
        .filter_map(|entry| {
            let p = entry.unwrap().path();
            let name = p.file_name()?.to_string_lossy();
            if !name.starts_with(prefix) || !name.ends_with(".request.json") {
                return None;
            }
            Some((
                serde_json::from_slice(&fs::read(&p).unwrap()).unwrap(),
                serde_json::from_slice(
                    &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
                )
                .unwrap(),
            ))
        })
        .collect()
}
#[test]
fn raw_chart_music_and_effective_inheritance_match_actual_fumen_readers_without_rewrites() {
    let cases = fixtures("chart-music-");
    assert_eq!(cases.len(), 4);
    for (request, expected) in cases {
        let dir = tempfile::tempdir().unwrap();
        let format = if request["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let source = request["Content"].as_str().unwrap();
        let mut s = Session::new(Document::parse(source.as_bytes(), format).unwrap());
        assert_eq!(
            Text::read(s.document(), 0).unwrap().get(Field::Music),
            expected[0]["Music"].as_str().unwrap()
        );
        let effective = expected[0]["EffectiveMusic"].as_str().unwrap();
        let asset = dir.path().join(effective);
        fs::create_dir_all(asset.parent().unwrap()).unwrap();
        fs::write(&asset, b"audio fixture: decoder tested separately").unwrap();
        let preview = Preview::capture(
            &s,
            0,
            &dir.path().join(format!("song.{}", format.extension())),
        )
        .unwrap();
        assert_eq!(preview.music(), asset);
        assert!(!Draft::capture(&s, 0).unwrap().apply(&mut s, 0).unwrap());
        assert_eq!(s.serialize().unwrap(), source.as_bytes());
    }
}
#[test]
fn music_rating_offset_apply_together_and_clear_restores_song_audio_after_save_and_undo() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(dir.path().join("song.ogg"), b"song").unwrap();
    fs::write(dir.path().join("chart.ogg"), b"chart").unwrap();
    let source=b"#VERSION:0.83;#MUSIC:song.ogg;#OFFSET:0;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:other.ogg;#NOTES:0001;";
    fs::write(&path, source).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let mut draft = Draft::capture(&file.session, 0).unwrap();
    draft.set_text(Field::Music, "chart.ogg").unwrap();
    draft.set_rating(13).unwrap();
    draft.set_music_offset(0.2).unwrap();
    draft.set_use_chart_offset(true);
    assert_eq!(
        Preview::capture(&file.session, 0, &path).unwrap().music(),
        dir.path().join("song.ogg")
    );
    draft.apply(&mut file.session, 0).unwrap();
    assert_eq!(file.session.undo_label(), Some("Change chart properties"));
    let after = file.session.serialize().unwrap();
    assert_eq!(
        Preview::capture(&file.session, 0, &path).unwrap().music(),
        dir.path().join("chart.ogg")
    );
    assert_eq!(
        Text::read(file.session.document(), 1)
            .unwrap()
            .get(Field::Music),
        "other.ogg"
    );
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), source);
    file.session.redo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), after);
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(
        Preview::capture(&reopened.session, 0, &path)
            .unwrap()
            .music(),
        dir.path().join("chart.ogg")
    );
    let mut clear = Draft::capture(&reopened.session, 0).unwrap();
    clear.set_text(Field::Music, "").unwrap();
    clear.apply(&mut reopened.session, 0).unwrap();
    assert_eq!(
        Preview::capture(&reopened.session, 0, &path)
            .unwrap()
            .music(),
        dir.path().join("song.ogg")
    );
    reopened.session.undo().unwrap();
    assert_eq!(reopened.session.serialize().unwrap(), after);
    reopened.session.redo().unwrap();
    reopened.save().unwrap();
    let final_file = FileSession::open(&path).unwrap();
    assert_eq!(
        Preview::capture(&final_file.session, 0, &path)
            .unwrap()
            .music(),
        dir.path().join("song.ogg")
    );
    assert_eq!(fs::read(dir.path().join("chart.ogg")).unwrap(), b"chart");
}
#[test]
fn lexical_paths_match_original_fumen_helper_including_outside_dots_unicode_and_literal_percent() {
    let cases = fixtures("music-relative-");
    assert_eq!(cases.len(), 6);
    for (request, expected) in cases {
        assert_eq!(
            music_file::relative_path(
                Path::new(request["Directory"].as_str().unwrap()),
                Path::new(request["Path"].as_str().unwrap())
            )
            .unwrap(),
            expected.as_str().unwrap()
        );
    }
}
#[test]
fn selected_files_are_validated_without_copying_or_changing_originals_and_sm_override_is_rejected()
{
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("Song");
    fs::create_dir(&folder).unwrap();
    let simfile = folder.join("song.ssc");
    let external = dir.path().join("Été #50%20.ogg");
    fs::write(&external, b"unchanged").unwrap();
    assert_eq!(
        music_file::selected_path(&simfile, &external).unwrap(),
        "../Été #50%20.ogg"
    );
    assert!(music_file::selected_path(&simfile, dir.path()).is_err());
    assert!(music_file::selected_path(&simfile, &folder.join("missing.ogg")).is_err());
    assert_eq!(fs::read(&external).unwrap(), b"unchanged");
    let bytes = b"#MUSIC:song.ogg;#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;";
    let mut s = Session::new(Document::parse(&bytes[..], Format::Sm).unwrap());
    let mut draft = Draft::capture(&s, 0).unwrap();
    assert!(!draft.supports_text(Field::Music));
    assert!(draft.set_text(Field::Music, "chart.ogg").is_err());
    assert!(!draft.apply(&mut s, 0).unwrap());
    assert_eq!(s.serialize().unwrap(), bytes);
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let invalid = folder.join(std::ffi::OsString::from_vec(vec![
            0xff, b'.', b'o', b'g', b'g',
        ]));
        // Some Unix filesystems reject these names before a dialog can return
        // them. Test the Unicode conversion independently of file creation.
        assert!(music_file::relative_path(&folder, &invalid).is_err());
        let backslash = folder.join(r"literal\name.ogg");
        fs::write(&backslash, b"legacy").unwrap();
        assert!(music_file::selected_path(&simfile, &backslash).is_err());
        let control = folder.join("line\nbreak.ogg");
        fs::write(&control, b"legacy").unwrap();
        assert!(music_file::selected_path(&simfile, &control).is_err());
    }
}
