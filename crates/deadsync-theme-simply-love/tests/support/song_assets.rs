use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_editor::{
    document::Format,
    music_file::{Browse, Kind, Picker},
    song_text::{Field, Text},
    storage::FileSession,
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, path::PathBuf, time::Instant};
fn key(s: &mut State, code: KeyCode, pressed: bool) {
    s.raw_key(&RawKeyboardEvent {
        code,
        pressed,
        repeat: false,
        timestamp: Instant::now(),
        host_nanos: 0,
    });
}
fn press(s: &mut State, code: KeyCode) {
    key(s, code, true);
    key(s, code, false);
}
fn ctrl(s: &mut State, code: KeyCode) {
    key(s, KeyCode::SuperLeft, true);
    press(s, code);
    key(s, KeyCode::SuperLeft, false);
}
struct Choose {
    kind: Kind,
    path: Option<PathBuf>,
    calls: usize,
}
impl Picker for Choose {
    fn choose(&mut self, request: &Browse) -> Result<Option<PathBuf>, deadsync_editor::Error> {
        assert_eq!(request.kind, self.kind);
        assert!(request.directory.is_dir());
        self.calls += 1;
        Ok(self.path.clone())
    }
}
fn setup(format: Format) -> (tempfile::TempDir, State, Vec<u8>) {
    i18n::init(deadsync_assets::language::load_for_tests(LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
    let d = tempfile::tempdir().unwrap();
    let (name, chart) = if format == Format::Sm {
        (
            "song.sm",
            "#JACKET:legacy.png;#NOTES:dance-single::Hard:9::1000;",
        )
    } else {
        (
            "song.ssc",
            "#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#NOTES:1000;",
        )
    };
    let bytes =
        format!("#VERSION:0.83;#TITLE:Source;#BPMS:0=120;#BANNER:old.png;#VENDOR:keep;{chart}")
            .into_bytes();
    fs::write(d.path().join(name), &bytes).unwrap();
    let state = State::open(&d.path().join(name), 0).unwrap();
    (d, state, bytes)
}
fn open(s: &mut State) {
    for code in [
        KeyCode::F11,
        KeyCode::ArrowUp,
        KeyCode::ArrowUp,
        KeyCode::ArrowUp,
        KeyCode::Enter,
    ] {
        press(s, code);
    }
}
fn field(s: &mut State, current: &mut usize, target: Field) {
    let target = Field::ALL
        .iter()
        .position(|field| *field == target)
        .unwrap();
    while *current != target {
        press(s, KeyCode::Tab);
        *current = (*current + 1) % (Field::ALL.len() + 1);
    }
}
#[test]
fn all_media_paths_use_typed_picker_stage_apply_undo_save_and_reopen() {
    for format in [Format::Sm, Format::Ssc] {
        let (d, mut s, original) = setup(format);
        open(&mut s);
        let mut current = 0;
        let cases = [
            (Field::Banner, Kind::Image),
            (Field::Background, Kind::ImageOrVideo),
            (Field::CdTitle, Kind::Image),
            (Field::Jacket, Kind::Image),
            (Field::CdImage, Kind::Image),
            (Field::DiscImage, Kind::Image),
            (Field::PreviewVideo, Kind::Video),
            (Field::Lyrics, Kind::Lyrics),
        ];
        let mut selected = Vec::new();
        for (index, (target, kind)) in cases.into_iter().enumerate() {
            field(&mut s, &mut current, target);
            let extension = match kind {
                Kind::Video | Kind::ImageOrVideo => "mp4",
                Kind::Lyrics => "lrc",
                _ => "png",
            };
            let name = format!("Été; # {index}.{extension}");
            let path = d.path().join(&name);
            fs::write(&path, b"untouched media fixture").unwrap();
            // macOS exposes its temporary directory through /var -> /private/var.
            // Use the same root as the opened simfile for this same-folder case;
            // lexical relative-path conversion has separate domain tests.
            let path = path.canonicalize().unwrap();
            let mut picker = Choose {
                kind,
                path: Some(path.clone()),
                calls: 0,
            };
            press(&mut s, KeyCode::F2);
            let handled = s.process_music_picker(&mut picker);
            assert!(handled);
            assert_eq!(picker.calls, 1);
            selected.push((target, name, path));
            if format == Format::Sm && target == Field::CdTitle {
                let mut actors = Vec::new();
                s.actors(&mut actors);
                assert!(actors.iter().any(|actor| matches!(actor,Actor::Text{content,..} if content.contains("legacy.png"))));
            }
            assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
        }
        let mut actors = Vec::new();
        s.actors(&mut actors);
        let label = if LANGUAGE == "fr" {
            "Fichier de paroles"
        } else {
            "Lyrics file"
        };
        assert!(
            actors
                .iter()
                .any(|actor| matches!(actor,Actor::Text{content,..} if content.starts_with(label)))
        );
        press(&mut s, KeyCode::Escape);
        for _ in 0..4 {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::Enter);
        let changed = s.workspace().file().session.serialize().unwrap();
        assert_ne!(changed, original);
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
        ctrl(&mut s, KeyCode::KeyY);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), changed);
        ctrl(&mut s, KeyCode::KeyS);
        let reopened = FileSession::open(s.workspace().file().require_existing_path().unwrap()).unwrap();
        let text = Text::read(reopened.session.document());
        for (target, name, path) in selected {
            assert_eq!(text.get(target), name);
            assert_eq!(fs::read(path).unwrap(), b"untouched media fixture");
        }
    }
}
#[test]
fn media_picker_cancel_stale_requests_and_clear_stay_in_the_draft() {
    let (_d, mut s, original) = setup(Format::Ssc);
    open(&mut s);
    let mut current = 0;
    field(&mut s, &mut current, Field::Banner);
    let mut picker = Choose {
        kind: Kind::Image,
        path: None,
        calls: 0,
    };
    press(&mut s, KeyCode::F2);
    press(&mut s, KeyCode::Tab);
    assert!(!s.process_music_picker(&mut picker));
    assert_eq!(picker.calls, 0);
    key(&mut s, KeyCode::ShiftLeft, true);
    press(&mut s, KeyCode::Tab);
    key(&mut s, KeyCode::ShiftLeft, false);
    press(&mut s, KeyCode::F2);
    assert!(s.process_music_picker(&mut picker));
    assert_eq!(picker.calls, 1);
    press(&mut s, KeyCode::F3);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::Escape);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
    assert!(!s.workspace().file().session.is_dirty());
}

#[test]
fn automatic_media_search_stages_real_images_and_lyrics_and_preserves_missing_paths() {
    for format in [Format::Sm, Format::Ssc] {
        let (d, mut s, original) = setup(format);
        open(&mut s);
        let mut current = 0;
        field(&mut s, &mut current, Field::Banner);
        press(&mut s, KeyCode::F4);
        let mut actors = Vec::new();
        s.actors(&mut actors);
        let missing = if LANGUAGE == "fr" {
            "Aucun média correspondant trouvé"
        } else {
            "No matching media found"
        };
        assert!(actors.iter().any(|actor| matches!(actor, Actor::Text {content,..} if content.contains(missing))));
        assert!(actors.iter().any(|actor| matches!(actor, Actor::Text {content,..} if content.contains("old.png"))));
        let cases = [
            (Field::Banner, "Été banner.png"),
            (Field::Background, "background.png"),
            (Field::CdTitle, "cdtitle.png"),
            (Field::Jacket, "jk_album.png"),
            (Field::CdImage, "album-cd.png"),
            (Field::DiscImage, "album disc.png"),
            (Field::Lyrics, "paroles.lrc"),
        ];
        for (target, name) in cases {
            if target == Field::Lyrics {
                fs::write(d.path().join(name), b"[00:01.00]Hello").unwrap();
            } else {
                image::RgbImage::new(2, 2).save(d.path().join(name)).unwrap();
            }
            field(&mut s, &mut current, target);
            press(&mut s, KeyCode::F4);
            assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
        }
        press(&mut s, KeyCode::Escape);
        for _ in 0..4 {
            press(&mut s, KeyCode::ArrowDown);
        }
        press(&mut s, KeyCode::Enter);
        let changed = s.workspace().file().session.serialize().unwrap();
        let text = Text::read(s.workspace().file().session.document());
        for (target, name) in cases {
            if text.supports(target) {
                assert_eq!(text.get(target), name);
            }
        }
        ctrl(&mut s, KeyCode::KeyZ);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
        ctrl(&mut s, KeyCode::KeyY);
        assert_eq!(s.workspace().file().session.serialize().unwrap(), changed);
        ctrl(&mut s, KeyCode::KeyS);
        let reopened = FileSession::open(s.workspace().file().require_existing_path().unwrap()).unwrap();
        assert_eq!(reopened.session.serialize().unwrap(), changed);
    }
}

#[test]
fn automatic_search_uses_decoded_dimensions_then_background_video_fallback() {
    let (d, mut s, original) = setup(Format::Ssc);
    image::RgbImage::new(418, 164).save(d.path().join("wide.png")).unwrap();
    image::RgbImage::new(640, 480).save(d.path().join("scene.png")).unwrap();
    open(&mut s);
    let mut current = 0;
    field(&mut s, &mut current, Field::Banner);
    press(&mut s, KeyCode::F4);
    field(&mut s, &mut current, Field::Background);
    press(&mut s, KeyCode::F4);
    let mut actors = Vec::new();
    s.actors(&mut actors);
    assert!(actors.iter().any(|actor| matches!(actor, Actor::Text {content,..} if content.contains("scene.png"))));
    fs::remove_file(d.path().join("wide.png")).unwrap();
    fs::remove_file(d.path().join("scene.png")).unwrap();
    fs::write(d.path().join("animation.mp4"), b"video selection fixture").unwrap();
    press(&mut s, KeyCode::F4);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
    press(&mut s, KeyCode::Escape);
    for _ in 0..4 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    let text = Text::read(s.workspace().file().session.document());
    assert_eq!(text.get(Field::Banner), "wide.png");
    assert_eq!(text.get(Field::Background), "animation.mp4");
}

#[test]
fn automatic_media_search_ignores_shortcuts_during_ime_composition() {
    let (d, mut s, original) = setup(Format::Ssc);
    image::RgbImage::new(2, 2).save(d.path().join("banner.png")).unwrap();
    open(&mut s);
    let mut current = 0;
    field(&mut s, &mut current, Field::Banner);
    s.text_preedit("日本語");
    press(&mut s, KeyCode::F4);
    s.text_preedit("");
    press(&mut s, KeyCode::Escape);
    for _ in 0..4 {
        press(&mut s, KeyCode::ArrowDown);
    }
    press(&mut s, KeyCode::Enter);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
    assert!(!s.workspace().file().session.is_dirty());
}
