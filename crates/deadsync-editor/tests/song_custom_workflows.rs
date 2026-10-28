use deadsync_editor::{
    document::{Document, Format, escape_text},
    history::Session,
    song_properties::{self, Draft},
    storage::FileSession,
};
use std::sync::Arc;
fn pair(suffix: &str, value: f64) -> String {
    let json = format!("{{\"SyncOffset\":{value},\"Future\":{{\"Keep\":true}}}}");
    format!(
        "#GASongDataVersion{suffix}:1;#GASongData{suffix}:{};",
        String::from_utf8(escape_text(&json)).unwrap()
    )
}
const CHARTS: &str =
    "#BPMS:0=120;#NOTES:dance-single:a:Hard:9::1000;#NOTES:dance-single:b:Easy:2::0001;";
#[test]
fn source_order_edits_effective_alias_only_and_survives_undo_redo_save_reopen() {
    for (first, last) in [("", "SONG"), ("SONG", "")] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("song.sm");
        let loser = pair(first, 0.1);
        let winner = pair(last, 0.2);
        let original = format!("{loser}{winner}#VENDOR:raw\\:keep;{CHARTS}");
        std::fs::write(&path, &original).unwrap();
        let mut file = FileSession::open(&path).unwrap();
        assert_eq!(
            song_properties::sync_offset(file.session.document()).unwrap(),
            0.2
        );
        let mut draft = Draft::capture(&file.session).unwrap();
        draft.set_sync_offset(0.375).unwrap();
        assert!(draft.apply(&mut file.session).unwrap());
        let changed = file.session.serialize().unwrap();
        assert!(String::from_utf8_lossy(&changed).contains(&loser));
        assert!(String::from_utf8_lossy(&changed).contains("#VENDOR:raw\\:keep;"));
        assert_eq!(
            song_properties::sync_offset(file.session.document()).unwrap(),
            0.375
        );
        assert!(file.session.undo().unwrap());
        assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
        assert!(file.session.redo().unwrap());
        file.save().unwrap();
        let reopened = FileSession::open(&path).unwrap();
        assert_eq!(
            song_properties::sync_offset(reopened.session.document()).unwrap(),
            0.375
        );
    }
}
#[test]
fn absent_sm_metadata_is_created_in_source_song_scope_and_old_bare_metadata_remains_compatible() {
    for old in [String::new(), pair("", 0.1)] {
        let original = format!("{old}{CHARTS}");
        let mut session = Session::new(
            Document::parse(Arc::<[u8]>::from(original.clone().into_bytes()), Format::Sm).unwrap(),
        );
        let mut draft = Draft::capture(&session).unwrap();
        draft.set_sync_offset(0.0).unwrap();
        draft.apply(&mut session).unwrap();
        let bytes = session.serialize().unwrap();
        let changed = String::from_utf8_lossy(&bytes);
        if old.is_empty() {
            assert!(changed.contains("#GASongDataVersionSONG:1;"));
            assert!(changed.contains("#GASongDataSONG:"));
        } else {
            assert!(!changed.contains("GASongDataSONG"));
        }
        assert_eq!(
            song_properties::sync_offset(session.document()).unwrap(),
            0.0
        );
        assert!(session.undo().unwrap());
        assert_eq!(session.serialize().unwrap(), original.as_bytes());
    }
}
#[test]
fn mixed_fields_are_independent_and_chart_deletion_updates_selected_data_owner() {
    let data =
        "{\"SyncOffset\":0.25,\"DefaultChartType\":0,\"DefaultChartIndex\":1,\"Future\":true}";
    let original = format!(
        "{}#GASongDataVersionSONG:1;#GASongDataSONG:{};{CHARTS}",
        pair("", 0.1),
        String::from_utf8(escape_text(data)).unwrap()
    );
    let mut session = Session::new(
        Document::parse(Arc::<[u8]>::from(original.clone().into_bytes()), Format::Sm).unwrap(),
    );
    deadsync_editor::chart_deletion::Draft::capture(&session, 0)
        .unwrap()
        .apply(&mut session)
        .unwrap();
    assert_eq!(
        song_properties::sync_offset(session.document()).unwrap(),
        0.25
    );
    let bytes = session.serialize().unwrap();
    let reparsed = Document::parse(bytes, Format::Sm).unwrap();
    let json: serde_json::Value =
        serde_json::from_str(&reparsed.text(None, "GASongDataSONG").unwrap()).unwrap();
    assert_eq!(json["DefaultChartIndex"], 0);
    assert_eq!(json["Future"], true);
    assert!(session.undo().unwrap());
    assert_eq!(session.serialize().unwrap(), original.as_bytes());
    let mixed = format!("{}#GASongDataVersion:99;{CHARTS}", pair("SONG", 0.2));
    let document = Document::parse(mixed.into_bytes(), Format::Sm).unwrap();
    assert!(song_properties::sync_offset(&document).is_err());
}
