use deadsync_editor::{
    chart_order::Snapshot,
    document::{Document, Format},
    history::Session,
    open_preferences::Preferences,
    storage::FileSession,
    workspace::Workspace,
};
use std::{fs, sync::Arc};
fn source(bad_preferred: bool) -> Vec<u8> {
    let single = if bad_preferred { "oops" } else { "1000" };
    let double = if bad_preferred { "10000000" } else { "oops" };
    format!("#VERSION:0.83;#TITLE:Peers retained;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:9;#VENDOR:single;#NOTES:{single};#NOTEDATA:;#STEPSTYPE:dance-double;#DIFFICULTY:Hard;#METER:8;#VENDOR:double;#NOTES:{double};").into_bytes()
}
#[test]
fn bad_nonpreferred_peer_does_not_prevent_valid_preferred_chart_or_rewrite_source() {
    for bad_preferred in [false, true] {
        let bytes = source(bad_preferred);
        let mut session =
            Session::new(Document::parse(Arc::<[u8]>::from(bytes.clone()), Format::Ssc).unwrap());
        let identities = session.chart_ids().to_vec();
        let token = (session.revision(), session.content_revision(), session.chart_ids().to_vec());
        // Existing strict snapshot callers keep their current error contract.
        assert!(Snapshot::resident(&mut session).is_err());
        let report = Snapshot::loadable(&mut session).unwrap();
        let good = if bad_preferred { 1 } else { 0 };
        let bad = 1 - good;
        assert_eq!(report.snapshot.source_indices(), [good]);
        assert_eq!(report.failures.len(), 1);
        assert_eq!(report.failures[0].source_index, bad);
        assert_eq!(report.failures[0].identity, identities[bad]);
        assert!(!report.failures[0].message.is_empty());
        let order = report.snapshot.indices_with(str::cmp).unwrap();
        assert_eq!(
            Preferences::default()
                .select(&report.snapshot, &order)
                .unwrap(),
            Some(good)
        );
        assert_eq!(session.chart_ids(), identities);
        assert_eq!((session.revision(), session.content_revision(), session.chart_ids().to_vec()), token);
        assert_eq!(session.serialize().unwrap(), bytes);
        assert!(!session.is_dirty());
    }
}
#[test]
fn invalid_chart_clock_is_reported_without_losing_its_valid_peer() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Challenge;#METER:9;#BPMS:0=inf;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-double;#DIFFICULTY:Hard;#METER:8;#NOTES:10000000;";
    let mut session = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    let report = Snapshot::loadable(&mut session).unwrap();
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].source_index, 0);
    assert_eq!(report.snapshot.source_indices(), [1]);
    assert_eq!(session.serialize().unwrap(), bytes);
    assert!(!session.is_dirty());
}
#[test]
fn all_bad_supported_charts_can_remain_unfocused_with_every_source_block_intact() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    let bytes = source(false);
    let bytes = String::from_utf8(bytes)
        .unwrap()
        .replace("#NOTES:1000;", "#NOTES:oops;");
    fs::write(&path, &bytes).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let identities = file.session.chart_ids().to_vec();
    let report = Snapshot::loadable(&mut file.session).unwrap();
    assert_eq!(report.failures.len(), 2);
    assert!(report.snapshot.source_indices().is_empty());
    let workspace = Workspace::from_file_unfocused(file);
    assert!(!workspace.has_chart());
    assert_eq!(workspace.file().session.chart_ids(), identities);
    assert_eq!(
        workspace.file().session.serialize().unwrap(),
        bytes.as_bytes()
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
}
#[test]
fn missing_notes_is_still_a_global_strict_document_error_before_chart_preparation() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTEDATA:;#STEPSTYPE:dance-double;#NOTES:10000000;";
    assert!(Document::parse(bytes.as_slice(), Format::Ssc).is_err());
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    fs::write(&path, bytes).unwrap();
    assert!(FileSession::open(&path).is_err());
    assert_eq!(fs::read(path).unwrap(), bytes);
}
#[test]
fn editing_valid_chart_undo_save_reopen_keeps_unavailable_peer_exactly() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("song.ssc");
    let bytes = source(false);
    fs::write(&path, &bytes).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let report = Snapshot::loadable(&mut file.session).unwrap();
    assert_eq!(report.failures.len(), 1);
    let doc = file.session.document();
    let peer = doc.value(doc.charts()[1].notes.clone()).to_vec();
    let position = deadsync_editor::notes::Position {
        row: 48,
        lane: 1,
        player: 0,
    };
    file.session
        .edit_notes(
            0,
            "Edit available chart",
            [(position, Some(deadsync_editor::notes::Note::tap()))],
        )
        .unwrap();
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), bytes);
    file.session.redo().unwrap();
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert!(reopened.session.grid(0).unwrap().get(position).is_some());
    let report = Snapshot::loadable(&mut reopened.session).unwrap();
    assert_eq!(report.failures.len(), 1);
    assert_eq!(report.failures[0].source_index, 1);
    let doc = reopened.session.document();
    assert_eq!(doc.charts().len(), 2);
    assert_eq!(doc.value(doc.charts()[1].notes.clone()), peer);
    assert_eq!(doc.text(Some(1), "VENDOR").as_deref(), Some("double"));
    assert!(!reopened.session.is_dirty());
}
#[test]
fn checked_preparation_cancellation_never_authorizes_an_empty_fallback() {
    let bytes = source(false);
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(bytes.clone()), Format::Ssc).unwrap());
    let result =
        Snapshot::loadable_checked(&mut session, &std::sync::atomic::AtomicBool::new(true));
    assert!(
        matches!(result,Err(deadsync_editor::Error::InvalidEdit(message)) if message=="Song opening cancelled")
    );
    assert_eq!(session.serialize().unwrap(), bytes);
    assert!(!session.is_dirty());
}
