use deadsync_editor::{
    audio_preferences::File,
    chart_identity::Difficulty,
    chart_order::Snapshot,
    document::{Document, Format},
    history::Session,
    open_preferences::Preferences,
};
use std::{fs, sync::Arc};
fn session(format: Format, charts: &[(&str, &str, i32)]) -> Session {
    let mut source = String::from("#VERSION:0.83;#TITLE:Preferred;#BPMS:0=120;");
    for &(kind, difficulty, meter) in charts {
        let columns = deadsync_editor::notes::layout_columns(kind).unwrap_or(4);
        let notes = format!("1{}", "0".repeat(usize::from(columns - 1)));
        source += &match format {
            Format::Sm => format!("#NOTES:{kind}::{difficulty}:{meter}::{notes};"),
            Format::Ssc => format!(
                "#NOTEDATA:;#STEPSTYPE:{kind};#DIFFICULTY:{difficulty};#METER:{meter};#NOTES:{notes};"
            ),
        };
    }
    Session::new(Document::parse(Arc::<[u8]>::from(source.into_bytes()), format).unwrap())
}
fn selected(session: &mut Session, preferences: Preferences) -> Option<usize> {
    let snapshot = Snapshot::resident(session).unwrap();
    let order = snapshot.indices_with(str::cmp).unwrap();
    preferences.select(&snapshot, &order).unwrap()
}
#[test]
fn defaults_choose_challenge_of_single_even_when_source_starts_with_double() {
    for format in [Format::Sm, Format::Ssc] {
        let mut session = session(
            format,
            &[
                ("dance-double", "Challenge", 10),
                ("dance-single", "Easy", 2),
                ("dance-single", "Challenge", 13),
                ("dance-single", "Challenge", 9),
            ],
        );
        let before = session.serialize().unwrap();
        let token = (
            session.revision(),
            session.content_revision(),
            session.chart_ids().to_vec(),
        );
        assert_eq!(selected(&mut session, Preferences::default()), Some(3));
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(
            (
                session.revision(),
                session.content_revision(),
                session.chart_ids().to_vec()
            ),
            token
        );
        assert!(!session.is_dirty());
    }
}
#[test]
fn preferred_exact_then_difficulty_priority_then_paired_type_match_source() {
    let mut preferences = Preferences::default();
    preferences.set_steps_type("pump-double").unwrap();
    preferences.set_difficulty(Difficulty::Hard);
    let mut exact = session(
        Format::Ssc,
        &[
            ("pump-double", "Challenge", 99),
            ("pump-double", "Hard", 7),
            ("pump-single", "Hard", 8),
        ],
    );
    assert_eq!(selected(&mut exact, preferences), Some(1));
    let mut fallback = session(
        Format::Ssc,
        &[
            ("pump-double", "Edit", 99),
            ("pump-double", "Easy", 1),
            ("pump-double", "Medium", 2),
        ],
    );
    assert_eq!(selected(&mut fallback, preferences), Some(2));
    let mut paired = session(
        Format::Ssc,
        &[
            ("dance-single", "Challenge", 13),
            ("pump-single", "Easy", 20),
            ("pump-single", "Hard", 1),
        ],
    );
    assert_eq!(selected(&mut paired, preferences), Some(2));
}
#[test]
fn unsupported_only_and_empty_song_have_no_default_editable_chart() {
    for mut song in [
        session(Format::Ssc, &[]),
        session(Format::Ssc, &[("kb7-single", "Hard", 7)]),
    ] {
        assert_eq!(selected(&mut song, Preferences::default()), None);
    }
}
#[test]
fn invalid_chart_permutation_is_rejected_without_mutating_the_song() {
    let mut song = session(
        Format::Ssc,
        &[
            ("dance-single", "Challenge", 9),
            ("dance-double", "Hard", 8),
        ],
    );
    let before = song.serialize().unwrap();
    let token = (
        song.revision(),
        song.content_revision(),
        song.chart_ids().to_vec(),
    );
    let snapshot = Snapshot::resident(&mut song).unwrap();
    for order in [vec![], vec![0, 0], vec![0, 7], vec![0]] {
        assert!(Preferences::default().select(&snapshot, &order).is_err());
    }
    assert_eq!(song.serialize().unwrap(), before);
    assert_eq!(
        (
            song.revision(),
            song.content_revision(),
            song.chart_ids().to_vec()
        ),
        token
    );
}
#[test]
fn common_options_are_read_and_survive_other_preference_saves() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("preferences.json");
    fs::write(&path,r#"{"Version":1,"Options":{"DefaultStepsType":"pump_double","DefaultDifficultyType":"Easy","UnknownVendor":{"keep":true}},"Vendor":"opaque"}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    let preferred = file.opening().unwrap();
    assert_eq!(preferred.steps_type(), Some("pump-double"));
    assert_eq!(preferred.difficulty_value(), 1);
    let mut audio = file.audio();
    audio.set_fade_in(0.25).unwrap();
    file.set_audio(audio);
    file.save().unwrap();
    let reopened = File::open(&path).unwrap();
    assert_eq!(reopened.opening().unwrap(), preferred);
    let json: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(json["Options"]["UnknownVendor"]["keep"], true);
    assert_eq!(json["Vendor"], "opaque");
}
#[test]
fn couple_and_routine_keep_the_explicit_native_multiplayer_extension() {
    let mut song = session(
        Format::Ssc,
        &[
            ("dance-routine", "Challenge", 10),
            ("dance-couple", "Challenge", 8),
        ],
    );
    let mut preference = Preferences::default();
    preference.set_steps_type("dance-couple").unwrap();
    assert_eq!(selected(&mut song, preference), Some(1));
    preference.set_steps_type("dance-routine").unwrap();
    assert_eq!(selected(&mut song, preference), Some(0));
    // Original excludes Couple from supported charts. Native intentionally keeps it.
    let mut couple = session(Format::Ssc, &[("dance-couple", "Challenge", 8)]);
    assert_eq!(selected(&mut couple, Preferences::default()), Some(0));
}
#[test]
fn stepf2_raw_double_uses_the_existing_effective_routine_comparison_key() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-double;#DIFFICULTY:Beginner;#METER:8;#NOTES:X000000000;#NOTEDATA:;#STEPSTYPE:pump-double;#DIFFICULTY:Hard;#METER:10;#NOTES:1000000000;";
    let mut song = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    let before = song.serialize().unwrap();
    let mut preferred = Preferences::default();
    preferred.set_steps_type("pump-routine").unwrap();
    assert_eq!(selected(&mut song, preferred), Some(0));
    assert_eq!(song.document().charts()[0].steps_type, "pump-double");
    assert_eq!(song.serialize().unwrap(), before);
    assert!(!song.is_dirty());
}
