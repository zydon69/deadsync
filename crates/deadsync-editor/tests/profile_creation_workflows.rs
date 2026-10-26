use deadsync_editor::{
    document::{Document, Format},
    generation::{
        metadata::ChartData,
        profiles::{Config, Kind, Profile, ProfileFile},
    },
    history::Session,
};
use std::{fs, sync::Arc};
fn session() -> Session {
    Session::new(
        Document::parse(
            Arc::<[u8]>::from(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
                    .as_slice(),
            ),
            Format::Ssc,
        )
        .unwrap(),
    )
}
fn profile(kind: Kind) -> Profile {
    let json = format!(
        r#"{{"Guid":"12345678-9abc-4def-8123-456789abcdef","Name":"Shared","Future":1e999,"Config":{}}}"#,
        Config::editor_defaults(kind).json().unwrap()
    );
    Profile::parse(kind, json.as_bytes()).unwrap()
}
#[test]
fn creation_for_every_family_orders_with_song_edits_and_retains_exact_bytes_on_redo() {
    for kind in [Kind::Pattern, Kind::Performed, Kind::Expressed] {
        let dir = tempfile::tempdir().unwrap();
        let library = dir.path().join("profiles");
        let profile = profile(kind);
        let mut s = session();
        let source = s.serialize().unwrap();
        let path = s.create_profile(&library, &profile).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert!(!s.is_dirty());
        assert_eq!(
            ProfileFile::open(&path).unwrap().profile().guid(),
            profile.guid()
        );
        s.edit_notes(
            0,
            "Insert note",
            [(
                deadsync_editor::notes::Position {
                    player: 0,
                    row: 48,
                    lane: 1,
                },
                Some(deadsync_editor::notes::Note::tap()),
            )],
        )
        .unwrap();
        let mut data = ChartData::default();
        data.music_offset = 1.0;
        data.save(&mut s, 0).unwrap();
        assert!(s.is_dirty());
        s.undo().unwrap();
        assert!(s.is_dirty());
        assert!(path.exists());
        s.undo().unwrap();
        assert!(path.exists());
        assert!(!s.is_dirty());
        s.undo().unwrap();
        assert!(!path.exists());
        assert_eq!(s.serialize().unwrap(), source);
        assert!(!s.is_dirty());
        s.redo().unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert!(!s.is_dirty());
        s.redo().unwrap();
        assert!(s.is_dirty());
        s.redo().unwrap();
        assert!(s.is_dirty());
    }
}
#[test]
fn external_edits_block_undo_and_existing_destinations_block_redo_without_consuming_history() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = session();
    let path = s
        .create_profile(dir.path(), &profile(Kind::Expressed))
        .unwrap();
    let bytes = fs::read(&path).unwrap();
    fs::write(&path, b"external edit").unwrap();
    let revision = s.revision();
    assert!(s.undo().is_err());
    assert_eq!(s.revision(), revision);
    assert_eq!(fs::read(&path).unwrap(), b"external edit");
    fs::write(&path, &bytes).unwrap();
    s.undo().unwrap();
    fs::write(&path, b"replacement").unwrap();
    assert!(s.redo().is_err());
    assert_eq!(fs::read(&path).unwrap(), b"replacement");
    fs::remove_file(&path).unwrap();
    s.redo().unwrap();
    assert_eq!(fs::read(path).unwrap(), bytes);
}
#[test]
fn identity_collision_under_another_filename_and_nested_staging_publish_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let profile = profile(Kind::Expressed);
    let other = dir.path().join("ecc-other-name.json");
    profile.export_to(&other).unwrap();
    let mut s = session();
    assert!(s.create_profile(dir.path(), &profile).is_err());
    assert!(s.undo_label().is_none());
    assert!(!dir.path().join(profile.filename()).exists());
    let fresh = profile.duplicate("Fresh").unwrap();
    assert!(
        s.edit_chart_atomic(0, "Nested creation", |temporary| {
            temporary.create_profile(dir.path(), &fresh).map(|_| ())
        })
        .is_err()
    );
    assert!(!dir.path().join(fresh.filename()).exists());
    assert!(s.undo_label().is_none());
}

#[test]
fn a_failed_creation_keeps_redo_and_discarding_the_creation_branch_keeps_the_file_absent() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = session();
    let path = s
        .create_profile(dir.path(), &profile(Kind::Expressed))
        .unwrap();
    let bytes = fs::read(&path).unwrap();
    s.undo().unwrap();
    let redo = s.redo_label().unwrap().to_owned();
    let builtin = Profile::parse(
        Kind::Expressed,
        br#"{"Guid":"a19d532e-b0ce-4759-ad1c-02ecbbdf2efd","Config":{}}"#,
    )
    .unwrap();
    assert!(s.create_profile(dir.path(), &builtin).is_err());
    assert_eq!(s.redo_label(), Some(redo.as_str()));
    s.redo().unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    s.undo().unwrap();
    let mut data = ChartData::default();
    data.music_offset = 2.0;
    data.save(&mut s, 0).unwrap();
    assert!(s.redo_label().is_none());
    assert!(!path.exists());
}

#[test]
fn chart_and_marker_drafts_keep_pending_values_only_across_the_next_copy_creation() {
    use deadsync_editor::{
        chart_properties,
        generation::{
            marker_edit,
            metadata::{Definition, PATTERN_BALANCED, SIXTEENTHS},
        },
    };
    use std::collections::BTreeMap;
    let dir = tempfile::tempdir().unwrap();
    let mut s = session();
    let data = ChartData {
        patterns: Some(BTreeMap::from([(
            0,
            Definition {
                pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                length: 48,
                start_position_inclusive: true,
                end_position_inclusive: true,
                ..Default::default()
            },
        )])),
        ..Default::default()
    };
    data.save(&mut s, 0).unwrap();
    let mut chart = chart_properties::Draft::capture(&s, 0).unwrap();
    chart.set_music_offset(1.0).unwrap();
    chart.set_use_chart_offset(true);
    chart.set_rating(13).unwrap();
    let mut marker = marker_edit::Draft::at_cursor(&s, 0, 0).unwrap();
    marker.definition.random_seed = 123;
    assert!(chart.after_profile_creation(&s).is_err());
    s.create_profile(dir.path(), &profile(Kind::Expressed))
        .unwrap();
    chart.after_profile_creation(&s).unwrap();
    marker.after_profile_creation(&s).unwrap();
    assert!(chart.has_changes());
    assert!(marker.has_changes());
    chart.apply(&mut s, 0).unwrap();
    assert_eq!(
        ChartData::read(s.document(), 0)
            .unwrap()
            .unwrap()
            .music_offset,
        1.0
    );
    assert_eq!(s.document().text(Some(0), "METER").as_deref(), Some("13"));
    s.undo().unwrap();
    marker.apply(&mut s, 0).unwrap();
    assert_eq!(
        ChartData::read(s.document(), 0)
            .unwrap()
            .unwrap()
            .patterns
            .unwrap()[&0]
            .random_seed,
        123
    );
    let mut stale = chart_properties::Draft::capture(&s, 0).unwrap();
    let mut changed = ChartData::read(s.document(), 0).unwrap().unwrap();
    changed.music_offset = 3.0;
    changed.save(&mut s, 0).unwrap();
    assert!(stale.after_profile_creation(&s).is_err());
    let other = session();
    assert!(stale.after_profile_creation(&other).is_err());
}
