use deadsync_editor::{
    chart_creation::{Draft, LAYOUTS},
    document::{Document, Format},
    generation::metadata::{ChartData, DYNAMIC},
    history::Session,
    workspace::{Command, Workspace},
};
use serde_json::Value;
use std::{fs, path::PathBuf};
const SONG: &str = "#VERSION:0.83;#TITLE:Keep;#OFFSET:0.25;#MUSIC:song.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:Existing;#DIFFICULTY:Hard;#METER:9;#CREDIT:Author;#MUSIC:chart.ogg;#OFFSET:0.75;#BPMS:0=150,4=200;#TIMESIGNATURES:0=3=4,8=7=8;#STOPS:1=0.5;#LABELS:0=Intro;#SCROLLS:0=-1;#GAChartDataVersion:1;#GAChartData:{\"Patterns\":{\"48\":{\"Length\":96}},\"Future\":1};#NOTES:1000;";
fn source() -> Session {
    Session::new(Document::parse(SONG.as_bytes(), Format::Ssc).unwrap())
}
#[test]
fn original_song_defaults_and_all_layout_properties_match_each_csharp_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grooveauthor");
    let mut count = 0;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.starts_with("new-chart-defaults-") || !name.ends_with(".request.json") {
            continue;
        }
        count += 1;
        let request: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let expected: Value = serde_json::from_slice(
            &fs::read(root.join(name.replace(".request.json", ".expected.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(expected["Layouts"].as_array().unwrap().len(), LAYOUTS.len());
        for layout in expected["Layouts"].as_array().unwrap() {
            let mut session = Session::new(
                Document::parse(request["Content"].as_str().unwrap().as_bytes(), Format::Ssc)
                    .unwrap(),
            );
            let draft = Draft::capture(&session).unwrap();
            let defaults = draft.defaults();
            assert_eq!(
                defaults.tempo,
                expected["Tempo"].as_f64().unwrap(),
                "{name}"
            );
            assert_eq!(
                u64::from(defaults.signature.numerator),
                expected["Numerator"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                u64::from(defaults.signature.denominator),
                expected["Denominator"].as_u64().unwrap(),
                "{name}"
            );
            let old_count = session.document().charts().len();
            let index = draft
                .apply(&mut session, layout["Name"].as_str().unwrap())
                .unwrap();
            assert_eq!(index, old_count);
            let grid = session.grid(index).unwrap();
            assert_eq!(
                u64::from(grid.columns()),
                layout["Columns"].as_u64().unwrap()
            );
            assert_eq!(
                u64::from(grid.players()),
                layout["Players"].as_u64().unwrap()
            );
            assert_eq!(grid.iter().count(), 0);
            assert_eq!(
                session
                    .document()
                    .text(Some(index), "DIFFICULTY")
                    .as_deref(),
                Some("Beginner")
            );
            assert_eq!(
                session.document().text(Some(index), "METER").as_deref(),
                Some("1")
            );
            let data = ChartData::read(session.document(), index).unwrap().unwrap();
            assert!(data.patterns.is_none());
            assert_eq!(data.expressed_chart_config.to_string(), DYNAMIC);
            assert!(!data.should_use_chart_music_offset);
        }
    }
    assert_eq!(count, 7);
}
#[test]
fn creation_uses_working_timing_without_cloning_notes_properties_or_patterns() {
    let mut session = source();
    session.set_text(Some(0), "BPMS", "0=180,4=200").unwrap();
    let before = session.serialize().unwrap();
    let draft = Draft::capture(&session).unwrap();
    assert_eq!(draft.defaults().tempo, 180.0);
    let index = draft.apply(&mut session, "dance-routine").unwrap();
    assert_eq!(index, 1);
    assert_eq!(
        session.document().text(Some(1), "BPMS").as_deref(),
        Some("0=180")
    );
    assert_eq!(
        session
            .document()
            .text(Some(1), "TIMESIGNATURES")
            .as_deref(),
        Some("0=3=4")
    );
    assert_eq!(
        session.document().text(Some(1), "SCROLLS").as_deref(),
        Some("0=1")
    );
    assert_eq!(
        session.document().text(Some(1), "TICKCOUNTS").as_deref(),
        Some("0=4")
    );
    assert_eq!(
        session.document().text(Some(1), "COMBOS").as_deref(),
        Some("0=1=1")
    );
    assert_eq!(
        session.document().text(Some(1), "SPEEDS").as_deref(),
        Some("0=1=0=0")
    );
    for tag in ["STOPS", "LABELS", "CREDIT", "MUSIC"] {
        assert!(session.document().text(Some(1), tag).is_none(), "{tag}");
    }
    assert_eq!(
        session.document().text(Some(1), "DESCRIPTION").as_deref(),
        Some("")
    );
    assert_eq!(session.grid(1).unwrap().iter().count(), 0);
    assert_eq!(session.grid(1).unwrap().players(), 2);
    let data = ChartData::read(session.document(), 1).unwrap().unwrap();
    assert!(data.patterns.is_none());
    assert!(data.extra.is_empty());
    assert_eq!(data.max_player_index, 1);
    assert_eq!(
        deadsync_editor::chart_properties::Draft::capture(&session, 1)
            .unwrap()
            .effective_music_offset(),
        0.25
    );
    let after = session.serialize().unwrap();
    // Adding a chart materializes the existing timing-chart identity in the
    // shared song GA owner. This can insert before the first chart, so the
    // previous full byte stream is not necessarily one contiguous prefix.
    // Every existing tag payload (including opaque chart metadata) must remain
    // byte-identical, and the chosen timing chart must remain the original.
    let original = Document::parse(before.as_slice(), Format::Ssc).unwrap();
    assert_eq!(original.charts().len(), 1);
    assert_eq!(session.document().charts().len(), 2);
    for tag in original.tags() {
        let actual = session.document().tag(tag.chart, &tag.name).unwrap();
        assert_eq!(
            session.document().value(actual.value.clone()),
            original.value(tag.value.clone()),
            "existing {:?}/{}",
            tag.chart,
            tag.name
        );
    }
    let song: Value =
        serde_json::from_str(&session.document().text(None, "GASongData").unwrap()).unwrap();
    assert_eq!(
        session
            .document()
            .text(None, "GASongDataVersion")
            .as_deref(),
        Some("1")
    );
    assert_eq!(song["DefaultChartType"], "dance_single");
    assert_eq!(song["DefaultChartIndex"], 0);
    assert_eq!(song["SyncOffset"], 0.009);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), after);
}
#[test]
fn empty_song_bootstrap_and_sm_independent_initial_timing_have_valid_reopenable_metadata() {
    for text in [
        "#TITLE:New;#OFFSET:0.125;",
        "#VERSION:0.60;#TITLE:New;#BPMS:0=142;",
    ] {
        let mut session = Session::new(Document::parse(text.as_bytes(), Format::Ssc).unwrap());
        let before = session.serialize().unwrap();
        let draft = Draft::capture(&session).unwrap();
        assert_eq!(draft.apply(&mut session, "dance-single").unwrap(), 0);
        assert_eq!(
            session.document().text(None, "VERSION").as_deref(),
            Some("0.83")
        );
        let reopened =
            Session::new(Document::parse(session.serialize().unwrap(), Format::Ssc).unwrap());
        assert!(ChartData::read(reopened.document(), 0).unwrap().is_some());
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        assert!(session.document().charts().is_empty());
        session.redo().unwrap();
        assert_eq!(session.document().charts().len(), 1);
    }
    let raw = b"#OFFSET:0.125;#BPMS:0=120,4=180;#STOPS:2=0.5;#NOTES:dance-single:Old:Hard:9::1000;";
    let mut session = Session::new(Document::parse(raw.as_slice(), Format::Sm).unwrap());
    let draft = Draft::capture(&session).unwrap();
    let index = draft.apply(&mut session, "pump-routine").unwrap();
    assert_eq!(index, 1);
    assert_eq!(
        session.document().text(None, "BPMS").as_deref(),
        Some("0=120,4=180")
    );
    assert_eq!(session.grid(1).unwrap().players(), 2);
    let bytes = session.serialize().unwrap();
    let mut reopened = Session::new(Document::parse(bytes, Format::Sm).unwrap());
    assert_eq!(reopened.grid(1).unwrap().iter().count(), 0);
    // EditorChart's SM constructor creates only its initial tempo/signature,
    // not the old chart's later BPM nodes, pauses or inherited attacks.
    assert_eq!(
        deadsync_editor::timing_edit::TempoDraft::capture(
            &reopened,
            1,
            deadsync_editor::timing_edit::Scope::Chart
        )
        .unwrap()
        .tempos()
        .serialize(),
        "0=120"
    );
    assert!(
        deadsync_editor::timing_edit::pause::Draft::capture(
            &reopened,
            1,
            deadsync_editor::timing_edit::Scope::Chart
        )
        .unwrap()
        .pauses(deadsync_editor::timing_edit::pause::Kind::Stop)
        .values()
        .is_empty()
    );
    assert_eq!(
        deadsync_editor::playback::Clock::capture(&reopened, 1)
            .unwrap()
            .chart_time(240)
            .unwrap(),
        2.5
    );
    let original = Session::new(Document::parse(raw.as_slice(), Format::Sm).unwrap());
    assert_eq!(
        deadsync_editor::playback::Clock::capture(&reopened, 0)
            .unwrap()
            .chart_time(240)
            .unwrap(),
        deadsync_editor::playback::Clock::capture(&original, 0)
            .unwrap()
            .chart_time(240)
            .unwrap()
    );
    let data = ChartData::read(reopened.document(), 1).unwrap().unwrap();
    assert_eq!(data.max_player_index, 1);
    assert!(!data.should_use_chart_music_offset);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), raw);
}
#[test]
fn wrong_layout_stale_foreign_legacy_and_orphan_metadata_fail_atomically() {
    let mut session = source();
    let draft = Draft::capture(&session).unwrap();
    let before = session.serialize().unwrap();
    assert!(draft.apply(&mut session, "bad;#NOTES:").is_err());
    assert_eq!(session.serialize().unwrap(), before);
    let mut foreign = source();
    assert!(draft.apply(&mut foreign, "dance-single").is_err());
    session.set_text(None, "TITLE", "Changed").unwrap();
    let changed = session.serialize().unwrap();
    assert!(draft.apply(&mut session, "dance-single").is_err());
    assert_eq!(session.serialize().unwrap(), changed);
    session.undo().unwrap();
    assert!(
        Draft::capture(&session)
            .unwrap()
            .apply(&mut session, "bad")
            .is_err()
    );
    assert!(session.redo().unwrap());
    let legacy = Session::new(
        Document::parse(SONG.replace("0.83", "0.60").as_bytes(), Format::Ssc).unwrap(),
    );
    assert!(Draft::capture(&legacy).is_err());
    assert!(!legacy.is_dirty());
    let raw =
        b"#GAChartDataCHART0001:{\"Future\":1};#BPMS:0=120;#NOTES:dance-single::Hard:9::1000;";
    let mut sm = Session::new(Document::parse(raw.as_slice(), Format::Sm).unwrap());
    let draft = Draft::capture(&sm).unwrap();
    assert!(draft.apply(&mut sm, "dance-double").is_err());
    assert_eq!(sm.serialize().unwrap(), raw);
    assert!(!sm.is_dirty());
}
#[test]
fn workspace_creation_focus_history_notes_and_save_reopen_are_independent() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    fs::write(&path, SONG).unwrap();
    let mut workspace = Workspace::open(&path, 0).unwrap();
    workspace.execute(Command::Seek(96)).unwrap();
    let draft = workspace.new_chart_draft().unwrap();
    let index = workspace.create_chart(&draft, "dance-routine").unwrap();
    assert_eq!(index, 1);
    assert_eq!(workspace.chart(), 1);
    assert_eq!(workspace.row(), 0);
    assert_eq!(workspace.grid().players(), 2);
    workspace.execute(Command::SelectPlayer(1)).unwrap();
    workspace.execute(Command::Place(4)).unwrap();
    workspace.execute(Command::Undo).unwrap();
    workspace.execute(Command::Undo).unwrap();
    assert_eq!(workspace.chart(), 0);
    assert_eq!(
        workspace.file().session.serialize().unwrap(),
        SONG.as_bytes()
    );
    workspace.execute(Command::Redo).unwrap();
    workspace.execute(Command::Redo).unwrap();
    assert_eq!(workspace.chart(), 1);
    workspace.execute(Command::Save).unwrap();
    let reopened = Workspace::open(&path, 1).unwrap();
    assert_eq!(reopened.grid(), workspace.grid());
    assert!(!reopened.file().session.is_dirty());
}
