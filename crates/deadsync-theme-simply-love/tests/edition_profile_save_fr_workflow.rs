use deadlib_platform::input::{KeyCode, RawKeyboardEvent};
use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
use std::{fs, time::Instant};
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
fn texts(s: &State) -> Vec<String> {
    let mut a = Vec::new();
    s.actors(&mut a);
    a.into_iter()
        .filter_map(|a| {
            if let Actor::Text { content, .. } = a {
                Some(content.to_string())
            } else {
                None
            }
        })
        .collect()
}
fn has(s: &State, value: &str) -> bool {
    texts(s).iter().any(|text| text.contains(value))
}
fn init() {
    i18n::init(deadsync_assets::language::load_for_tests("fr"));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854.0, 480.0,
    ));
}
fn finish(s: &mut State) {
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    while s.is_generating() {
        s.update();
        assert!(Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}
fn open(s: &mut State) {
    press(s, KeyCode::F7);
    press(s, KeyCode::Enter);
    finish(s);
    press(s, KeyCode::KeyE);
}
fn custom(
    path: &std::path::Path,
    profiles: &std::path::Path,
) -> (State, std::path::PathBuf, Vec<u8>) {
    use deadsync_editor::{
        document::{Document, Format},
        generation::{
            metadata::ChartData,
            profiles::{Kind, Profile},
        },
        history::Session,
    };
    let profile=Profile::parse(Kind::Expressed,br#"{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Original","Future":"keep","Config":{"MinLevelForBrackets":4}}"#).unwrap();
    let file = profile.export_new(profiles).unwrap();
    let mut session = Session::new(
        Document::parse(
            std::sync::Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    );
    ChartData {
        expressed_chart_config: profile.guid().clone(),
        ..Default::default()
    }
    .save(&mut session, 0)
    .unwrap();
    let source = session.serialize().unwrap();
    fs::write(path, &source).unwrap();
    (
        State::open_with_profiles(path, 0, profiles).unwrap(),
        file.path().to_owned(),
        source,
    )
}
fn original_save(s: &mut State) {
    press(s, KeyCode::End);
    for _ in 0..3 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    finish(s);
}
#[test]
fn french_profile_name_staging_original_save_and_conflict_messages_are_resolved() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    open(&mut s);
    press(&mut s, KeyCode::F2);
    assert!(s.wants_text_input());
    assert!(has(&s, "Entrée : préparer le nom"));
    assert!(has(&s, "Échap : abandonner le nouveau nom"));
    ctrl(&mut s, KeyCode::KeyA);
    assert!(s.text_input("Profil français"));
    press(&mut s, KeyCode::Enter);
    assert!(has(&s, "Nom préparé."));
    original_save(&mut s);
    assert_eq!(
        deadsync_editor::generation::profiles::ProfileFile::open(&file)
            .unwrap()
            .profile()
            .name(),
        Some("Profil français")
    );
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Autre nom"));
    press(&mut s, KeyCode::Enter);
    fs::write(&file, b"Externe").unwrap();
    original_save(&mut s);
    assert!(has(&s, "Le fichier du profil a changé sur le disque"));
    assert!(
        !texts(&s)
            .iter()
            .any(|v| v.starts_with("ProfileName") || v.starts_with("ProfileEdit"))
    );
    assert_eq!(fs::read(&file).unwrap(), b"Externe");
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
}

#[test]
fn french_saved_history_menu_restores_the_profile_without_touching_song() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let original = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Saved name"));
    press(&mut s, KeyCode::Enter);
    original_save(&mut s);
    let saved = fs::read(&file).unwrap();
    assert_ne!(saved, original);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F2);
    assert!(s.text_input("Pending name"));
    press(&mut s, KeyCode::Enter);
    press(&mut s, KeyCode::F7);
    assert!(has(&s, "Enregistrez ou abandonnez"));
    assert_eq!(fs::read(&file).unwrap(), saved);
    press(&mut s, KeyCode::Escape);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::End);
    for _ in 0..6 {
        press(&mut s, KeyCode::ArrowDown);
    }
    assert!(has(&s, "Annuler la sauvegarde"));
    press(&mut s, KeyCode::Enter);
    finish(&mut s);
    assert_eq!(fs::read(&file).unwrap(), original);
    press(&mut s, KeyCode::KeyE);
    press(&mut s, KeyCode::F8);
    finish(&mut s);
    assert_eq!(fs::read(&file).unwrap(), saved);
    assert!(has(&s, "Saved name"));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    assert_eq!(fs::read(path).unwrap(), source);
}

#[test]
fn profile_deletion_confirmation_uses_global_history_and_preserves_song_file() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    let original = fs::read(&file).unwrap();
    open(&mut s);
    press(&mut s, KeyCode::F9);
    assert!(has(&s, "Supprimer ce profil ?"));
    press(&mut s, KeyCode::Enter);
    assert_eq!(fs::read(&file).unwrap(), original);
    press(&mut s, KeyCode::F9);
    press(&mut s, KeyCode::ArrowDown);
    press(&mut s, KeyCode::Enter);
    assert!(!file.exists());
    assert!(has(&s, "Profil supprimé."));
    let repaired = s.workspace().file().session.serialize().unwrap();
    assert_ne!(repaired, source);
    press(&mut s, KeyCode::Digit2);
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(!file.exists());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), repaired);
    fs::write(&file, b"external replacement").unwrap();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(fs::read(&file).unwrap(), b"external replacement");
    assert_eq!(s.workspace().file().session.serialize().unwrap(), repaired);
    fs::remove_file(&file).unwrap();
    ctrl(&mut s, KeyCode::KeyZ);
    assert_eq!(fs::read(&file).unwrap(), original);
    assert_eq!(s.workspace().file().session.serialize().unwrap(), source);
    ctrl(&mut s, KeyCode::KeyY);
    assert!(!file.exists());
    assert_eq!(s.workspace().file().session.serialize().unwrap(), repaired);
    assert_eq!(fs::read(path).unwrap(), source);
}

fn conversion_selection(s: &mut State, custom: bool) {
    press(s, KeyCode::F3);
    for _ in 0..5 {
        press(s, KeyCode::ArrowDown);
    }
    press(s, KeyCode::Enter);
    finish(s);
    press(s, KeyCode::ArrowDown);
    if custom {
        for _ in 0..20 {
            if has(s, "Original") {
                break;
            }
            press(s, KeyCode::ArrowRight);
        }
        assert!(has(s, "Original"));
    } else {
        press(s, KeyCode::ArrowRight);
    }
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
    press(s, KeyCode::Escape);
}
fn expression_summary(s: &mut State) -> String {
    press(s, KeyCode::F3);
    let template = i18n::tr("Edition", "GenerationExpressionSummary");
    let prefix = template.split('{').next().unwrap();
    let summary = texts(s)
        .into_iter()
        .find(|text| text.starts_with(prefix))
        .unwrap();
    press(s, KeyCode::Escape);
    summary
}
fn delete_original(s: &mut State) {
    open(s);
    press(s, KeyCode::F9);
    press(s, KeyCode::ArrowDown);
    press(s, KeyCode::Enter);
}
#[test]
fn deleting_selected_conversion_profile_restores_its_selection_on_global_undo_and_redo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, source) = custom(&path, &profiles);
    conversion_selection(&mut s, true);
    assert!(expression_summary(&mut s).contains("Original"));
    delete_original(&mut s);
    assert!(!file.exists());
    let fallback = expression_summary(&mut s);
    assert!(!fallback.contains("Original"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(file.exists());
    assert!(expression_summary(&mut s).contains("Original"));
    ctrl(&mut s, KeyCode::KeyY);
    assert!(!file.exists());
    assert_eq!(expression_summary(&mut s), fallback);
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(expression_summary(&mut s).contains("Original"));
    assert_eq!(fs::read(path).unwrap(), source);
}
#[test]
fn a_later_explicit_conversion_choice_is_not_overwritten_by_deletion_undo() {
    init();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let profiles = dir.path().join("profiles");
    let (mut s, file, _) = custom(&path, &profiles);
    conversion_selection(&mut s, true);
    delete_original(&mut s);
    conversion_selection(&mut s, false);
    let chosen = expression_summary(&mut s);
    assert!(!chosen.contains("Original"));
    ctrl(&mut s, KeyCode::KeyZ);
    assert!(file.exists());
    assert_eq!(expression_summary(&mut s), chosen);
    ctrl(&mut s, KeyCode::KeyY);
    assert!(!file.exists());
    assert_eq!(expression_summary(&mut s), chosen);
}
