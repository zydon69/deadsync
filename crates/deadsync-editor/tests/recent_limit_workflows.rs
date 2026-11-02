use deadsync_editor::{
    audio_preferences::File,
    document::{Document, Format},
    history::Session,
    recent_songs::Song,
};
use serde_json::Value;
fn names(file: &File) -> Vec<String> {
    file.recent_songs()
        .entries
        .iter()
        .map(|r| r.song().unwrap().file.to_str().unwrap().to_owned())
        .collect()
}
fn compare(file: &File, case: &Value) {
    assert_eq!(
        file.recent_songs().maximum as u64,
        case["maximum"].as_u64().unwrap()
    );
    assert_eq!(
        names(file),
        case["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    );
}
fn change(session: &mut Session, file: &mut File, value: usize) {
    session
        .set_recent_files_history_size(file.recent_songs().maximum, value, file)
        .unwrap();
    *file = session.take_restore_options_update().unwrap().file.unwrap();
}
fn history(session: &mut Session, file: &mut File, redo: bool) {
    session.capture_source_preferences_context(file);
    if redo {
        session.redo().unwrap();
    } else {
        session.undo().unwrap();
    }
    *file = session.take_restore_options_update().unwrap().file.unwrap();
}
fn record(file: &mut File, name: &str) {
    file.record_recent_song(&Song {
        file: name.into(),
        title: None,
        pack: None,
        spacing_zoom: 1.,
        chart_position: 0.,
        active: vec![],
        focused: None,
    })
    .unwrap();
    file.save().unwrap();
}
#[test]
fn original_limit_action_keeps_records_until_real_record_and_undo_never_resurrects_them() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/recent-limit-source.json")).unwrap();
    let c = fixture["cases"].as_array().unwrap();
    assert_eq!(c.len(), 11);
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(&path,r#"{"Version":1,"Options":{"RecentFilesHistorySize":4,"Future":9},"RecentFiles":[{"FileName":"A","ActiveCharts":[]},{"FileName":"B","ActiveCharts":[]},{"FileName":"C","ActiveCharts":[]},{"FileName":"D","ActiveCharts":[]}],"Vendor":{"keep":true}}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    let mut session =
        Session::new(Document::parse(b"#TITLE:Unchanged;".as_slice(), Format::Sm).unwrap());
    compare(&file, &c[0]);
    change(&mut session, &mut file, 2);
    compare(&file, &c[1]);
    history(&mut session, &mut file, false);
    compare(&file, &c[2]);
    history(&mut session, &mut file, true);
    compare(&file, &c[3]);
    compare(&file, &c[4]);
    record(&mut file, "C");
    compare(&file, &c[5]);
    history(&mut session, &mut file, false);
    compare(&file, &c[6]);
    history(&mut session, &mut file, true);
    compare(&file, &c[7]);
    change(&mut session, &mut file, 0);
    compare(&file, &c[8]);
    record(&mut file, "E");
    compare(&file, &c[9]);
    history(&mut session, &mut file, false);
    compare(&file, &c[10]);
    assert!(!file.has_changes());
    assert_eq!(
        File::open(&path).unwrap().recent_songs(),
        file.recent_songs()
    );
    assert_eq!(session.serialize().unwrap(), b"#TITLE:Unchanged;");
    let out: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(out["Options"]["Future"], 9);
    assert_eq!(out["Vendor"]["keep"], true);
}
#[test]
fn competing_limit_blocks_undo_atomically_but_disjoint_record_updates_are_allowed() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("prefs.json");
    std::fs::write(&path, r#"{"Version":1}"#).unwrap();
    let mut file = File::open(&path).unwrap();
    let mut session =
        Session::new(Document::parse(b"#TITLE:Keep;".as_slice(), Format::Sm).unwrap());
    change(&mut session, &mut file, 2);
    let mut competing = File::open(&path).unwrap();
    let mut recent = competing.recent_songs().clone();
    recent.maximum = 3;
    competing.set_recent_songs(recent).unwrap();
    competing.save().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let revision = session.revision();
    session.capture_source_preferences_context(&file);
    assert!(session.undo().is_err());
    assert_eq!(session.revision(), revision);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(file.recent_songs().maximum, 2);
}
