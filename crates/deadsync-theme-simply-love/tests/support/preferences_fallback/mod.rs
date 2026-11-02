use deadsync_editor::{Error, chart_order, pack::song_list, runtime};
use deadsync_theme_simply_love::{
    i18n,
    screens::edition::{EditorEntryRequest, State},
};
use std::time::{Duration, Instant};
struct PackOrder;
impl song_list::Orderer for PackOrder {
    fn order(&mut self, keys: &[song_list::OrderKey]) -> Result<Vec<usize>, Error> {
        Ok((0..keys.len()).collect())
    }
}
struct ChartOrder;
impl chart_order::Orderer for ChartOrder {
    fn order(&mut self, snapshot: &chart_order::Snapshot) -> Result<Vec<usize>, Error> {
        snapshot.indices_with(str::cmp)
    }
}
#[test]
fn rejected_preferences_keep_explicit_and_empty_intents_without_overwriting_rejected_bytes() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    let root = tempfile::tempdir().unwrap();
    let prefs = root.path().join("prefs.json");
    let rejected = br#"{"Version":2,"Future":"do not replace"}"#;
    std::fs::write(&prefs, rejected).unwrap();
    runtime::init_paths(root.path().join("configs"), prefs.clone()).unwrap();
    let song = root.path().join("song.ssc");
    let original=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;";
    std::fs::write(&song, original).unwrap();
    assert!(deadsync_editor::audio_preferences::File::open(&prefs).is_err());
    let empty = State::prepare_entry(root.path(), EditorEntryRequest::Empty).unwrap();
    assert!(!empty.has_open_song());
    assert!(empty.preferences_load_diagnostic().is_some());
    let direct = State::open(&song, 0).unwrap();
    assert!(direct.has_open_song());
    assert!(direct.preferences_load_diagnostic().is_some());
    assert_eq!(
        deadsync_editor::song_properties::sync_offset(direct.workspace().file().session.document())
            .unwrap(),
        0.009
    );
    let mut explicit =
        State::prepare_entry(root.path(), EditorEntryRequest::Explicit(song.clone())).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !explicit.has_open_song() && Instant::now() < deadline {
        explicit.process_pack_song_list(&mut PackOrder, &mut ChartOrder);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        explicit.has_open_song(),
        "explicit file intent must survive rejected application preferences"
    );
    assert!(explicit.preferences_load_diagnostic().is_some());
    let canonical_song = song.canonicalize().unwrap();
    assert_eq!(
        explicit.workspace().file().existing_path(),
        Some(canonical_song.as_path())
    );
    assert_eq!(
        explicit.workspace().file().session.serialize().unwrap(),
        original
    );
    assert!(!explicit.workspace().file().session.is_dirty());
    assert_eq!(std::fs::read(&prefs).unwrap(), rejected);
    assert_eq!(std::fs::read(&song).unwrap(), original);
}
