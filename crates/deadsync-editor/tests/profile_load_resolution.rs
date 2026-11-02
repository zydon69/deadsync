use deadsync_editor::{
    document::{Document, Format},
    generation::{
        job::{ChartOptions, ChartTask},
        metadata::{ChartData, DYNAMIC, Guid, PATTERN_BALANCED, Registry, SIXTEENTHS},
        pattern_job,
        profile_load::Plan,
        profiles::{Config, Kind},
    },
    history::Session,
    notes::{Note, Position},
    storage::FileSession,
};
use std::{fs, path::Path, sync::atomic::AtomicBool};
fn raw(format: Format, input: &serde_json::Value) -> Vec<u8> {
    let mut input = input.clone();
    input["FutureChart"] = serde_json::json!({"keep":[1,2,3]});
    input["Patterns"]["96"]["FutureMarker"] = serde_json::json!({"keep":"marker"});
    let encoded = input.to_string();
    let encoded = format!(
        "{},\"Opaque\":{{\"num\":1e2}},\"Opaque\":{{\"num\":2e2}}}}",
        encoded.strip_suffix('}').unwrap()
    );
    let custom = String::from_utf8(deadsync_editor::document::escape_text(&encoded)).unwrap();
    match format{Format::Sm=>format!("#TITLE:Original;#FUTURE:opaque;#BPMS:0=120;#GAChartDataVersionCHART0000:1;#GAChartDataCHART0000:{custom};#NOTES:dance-single:source:Hard:9:0,0,0,0,0:1000\n0100\n0010\n0001;"),Format::Ssc=>format!("#VERSION:0.83;#TITLE:Original;#FUTURE:opaque;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;#NOTES:1000\n0100\n0010\n0001;#GAChartDataVersion:1;#GAChartData:{custom};")}.into_bytes()
}
fn cases() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("fixtures/profile-load/source.expected.json")).unwrap()
}
fn open(
    root: &Path,
    format: Format,
    input: &serde_json::Value,
) -> (std::path::PathBuf, Vec<u8>, FileSession) {
    let path = root.join(if format == Format::Sm {
        "song.sm"
    } else {
        "song.ssc"
    });
    let bytes = raw(format, input);
    fs::write(&path, &bytes).unwrap();
    let file = FileSession::open(&path).unwrap();
    (path, bytes, file)
}
fn edit(session: &mut Session) {
    session
        .edit_notes(
            0,
            "User note",
            [(
                Position {
                    row: 24,
                    lane: 2,
                    player: 0,
                },
                Some(Note::tap()),
            )],
        )
        .unwrap();
}
#[test]
fn actual_original_load_matrix_is_one_effective_baseline_with_exact_raw_cas_and_no_hidden_undo() {
    for format in [Format::Sm, Format::Ssc] {
        for case in cases() {
            let root = tempfile::tempdir().unwrap();
            let (path, raw, mut file) = open(root.path(), format, &case["input"]);
            let ids = file.session.chart_ids().to_vec();
            file.resolve_generation_profiles(None, &AtomicBool::new(false))
                .unwrap();
            let data = ChartData::read(file.session.document(), 0)
                .unwrap()
                .unwrap();
            assert_eq!(
                data.expressed_chart_config.to_string(),
                case["Expression"].as_str().unwrap()
            );
            let definition = &data.patterns.as_ref().unwrap()[&96];
            let expected = &case["Patterns"][0]["Definition"];
            assert_eq!(
                definition.pattern_config_guid.to_string(),
                expected["PatternConfigGuid"].as_str().unwrap()
            );
            assert_eq!(
                definition.performed_chart_config_guid.to_string(),
                expected["PerformedChartConfigGuid"].as_str().unwrap()
            );
            assert_eq!(
                data.extra["FutureChart"]["keep"],
                serde_json::json!([1, 2, 3])
            );
            assert_eq!(definition.extra["FutureMarker"]["keep"], "marker");
            let owner = match format {
                Format::Sm => file.session.document().text(None, "GAChartDataCHART0000"),
                Format::Ssc => file.session.document().text(Some(0), "GAChartData"),
            }
            .unwrap();
            assert_eq!(owner.matches("\"Opaque\"").count(), 2);
            assert!(owner.contains("\"num\":1e2"));
            assert!(owner.contains("\"num\":2e2"));
            assert_eq!(file.session.is_dirty(), case["Dirty"].as_bool().unwrap());
            assert_eq!(
                file.generation_profile_repairs().len(),
                case["Warnings"].as_array().unwrap().len()
            );
            assert!(file.session.undo_label().is_none());
            assert!(file.session.redo_label().is_none());
            assert_eq!(file.session.chart_ids(), ids);
            assert!(!file.has_external_change().unwrap());
            assert_eq!(fs::read(&path).unwrap(), raw);
            let normalized = file.session.serialize().unwrap();
            let dirty = file.session.is_dirty();
            edit(&mut file.session);
            let edited = file.session.serialize().unwrap();
            assert_ne!(edited, normalized);
            assert!(file.session.undo().unwrap());
            assert_eq!(file.session.serialize().unwrap(), normalized);
            assert_eq!(file.session.is_dirty(), dirty);
            assert!(file.session.redo().unwrap());
            assert_eq!(file.session.serialize().unwrap(), edited);
            file.save().unwrap();
            let persisted = fs::read(&path).unwrap();
            assert_ne!(persisted, raw);
            assert!(!file.session.is_dirty());
            assert!(!file.has_external_change().unwrap());
            let mut reopened = FileSession::open(&path).unwrap();
            reopened
                .resolve_generation_profiles(None, &AtomicBool::new(false))
                .unwrap();
            assert!(!reopened.session.is_dirty());
            assert!(reopened.session.undo_label().is_none());
            assert!(reopened.generation_profile_repairs().is_empty());
            let again = ChartData::read(reopened.session.document(), 0)
                .unwrap()
                .unwrap();
            assert_eq!(again.extra["FutureChart"], data.extra["FutureChart"]);
        }
    }
}
#[test]
fn repaired_metadata_feeds_real_chart_and_saved_pattern_workers_then_actual_apply_save_reopen() {
    for format in [Format::Sm, Format::Ssc] {
        let root = tempfile::tempdir().unwrap();
        let (path, _, mut file) = open(root.path(), format, &cases()[2]["input"]);
        file.resolve_generation_profiles(None, &AtomicBool::new(false))
            .unwrap();
        let cancel = AtomicBool::new(false);
        let conversion =
            ChartTask::capture(&file.session, 0, ChartOptions::balanced("dance-double"))
                .unwrap()
                .with_chart_expression(None)
                .unwrap()
                .run(&cancel, |_| {})
                .unwrap();
        assert_eq!(conversion.target_layout(), "dance-double");
        let preview = pattern_job::Task::capture_saved(&file.session, 0, 96, &Registry::builtin())
            .unwrap()
            .run(&cancel, |_, _| {})
            .unwrap();
        assert_eq!(preview.generated().start, 96);
        let before = file.session.serialize().unwrap();
        preview.apply(&mut file.session, 0).unwrap();
        let after = file.session.serialize().unwrap();
        assert!(file.session.undo().unwrap());
        assert_eq!(file.session.serialize().unwrap(), before);
        assert!(file.session.is_dirty());
        assert!(file.session.redo().unwrap());
        assert_eq!(file.session.serialize().unwrap(), after);
        file.save().unwrap();
        let mut reopened = FileSession::open(&path).unwrap();
        reopened.resolve_generation_profiles(None, &cancel).unwrap();
        assert!(!reopened.session.is_dirty());
        assert_eq!(
            ChartData::read(reopened.session.document(), 0)
                .unwrap()
                .unwrap()
                .patterns
                .unwrap()[&96]
                .pattern_config_guid
                .to_string(),
            SIXTEENTHS
        );
    }
}
#[test]
fn configured_catalog_keeps_present_custom_profiles_and_guards_membership_bytes_context_and_cancel()
{
    let root = tempfile::tempdir().unwrap();
    let profiles = root.path().join("profiles");
    fs::create_dir(&profiles).unwrap();
    let unknown = "12345678-9abc-4def-8123-456789abcdef";
    let registry = Registry::builtin();
    for (kind, config) in [
        (
            Kind::Expressed,
            Config::Expressed(registry.expressed[&DYNAMIC.parse::<Guid>().unwrap()].clone()),
        ),
        (
            Kind::Pattern,
            Config::Pattern(registry.patterns[&SIXTEENTHS.parse::<Guid>().unwrap()].clone()),
        ),
        (
            Kind::Performed,
            Config::Performed(
                registry.performed[&PATTERN_BALANCED.parse::<Guid>().unwrap()].clone(),
            ),
        ),
    ] {
        fs::write(profiles.join(format!("{}custom.json",kind.prefix())),serde_json::to_vec(&serde_json::json!({"Guid":unknown,"Name":"Custom","Config":config.json().unwrap()})).unwrap()).unwrap();
    }
    let (_, _, mut file) = open(root.path(), Format::Ssc, &cases()[2]["input"]);
    file.resolve_generation_profiles(Some(&profiles), &AtomicBool::new(false))
        .unwrap();
    let data = ChartData::read(file.session.document(), 0)
        .unwrap()
        .unwrap();
    assert_eq!(data.expressed_chart_config.to_string(), unknown);
    assert_eq!(
        data.patterns.unwrap()[&96].pattern_config_guid.to_string(),
        unknown
    );
    assert!(!file.session.is_dirty());
    assert!(file.generation_profile_repairs().is_empty());
    file.validate_generation_profile_context(Some(&profiles), &AtomicBool::new(false))
        .unwrap();
    assert!(
        file.validate_generation_profile_context(None, &AtomicBool::new(false))
            .is_err()
    );
    fs::write(profiles.join("ecc-custom.json"), b"external writer").unwrap();
    assert!(
        file.validate_generation_profiles(&AtomicBool::new(false))
            .is_err()
    );
    let fresh = tempfile::tempdir().unwrap();
    let (_, _, mut pending) = open(fresh.path(), Format::Ssc, &cases()[2]["input"]);
    let directory = fresh.path().join("missing");
    let before = pending.session.serialize().unwrap();
    let plan = Plan::capture(&pending.session, Some(&directory), &AtomicBool::new(false)).unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("ecc-new.json"), b"{}").unwrap();
    assert!(
        plan.apply(&mut pending.session, &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(pending.session.serialize().unwrap(), before);
    let plan = Plan::capture(&pending.session, None, &AtomicBool::new(false)).unwrap();
    assert!(
        plan.apply(&mut pending.session, &AtomicBool::new(true))
            .is_err()
    );
    assert_eq!(pending.session.serialize().unwrap(), before);
    assert!(Plan::capture(&pending.session, None, &AtomicBool::new(true)).is_err());
}
#[test]
fn stale_session_user_edit_and_external_source_change_never_publish_partial_resolution() {
    let root = tempfile::tempdir().unwrap();
    let (path, _, mut file) = open(root.path(), Format::Ssc, &cases()[2]["input"]);
    let plan = Plan::capture(&file.session, None, &AtomicBool::new(false)).unwrap();
    edit(&mut file.session);
    let edited = file.session.serialize().unwrap();
    assert!(
        plan.apply(&mut file.session, &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(file.session.serialize().unwrap(), edited);
    let mut fresh = FileSession::open(&path).unwrap();
    let plan = Plan::capture(&fresh.session, None, &AtomicBool::new(false)).unwrap();
    let mut foreign =
        Session::new(Document::parse(fresh.session.serialize().unwrap(), Format::Ssc).unwrap());
    assert!(plan.apply(&mut foreign, &AtomicBool::new(false)).is_err());
    fs::write(&path, b"external source writer").unwrap();
    let before = fresh.session.serialize().unwrap();
    assert!(
        fresh
            .resolve_generation_profiles(None, &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(fresh.session.serialize().unwrap(), before);
}
#[test]
fn preexisting_recovery_survives_automatic_marker_repair_but_real_user_edit_still_refuses() {
    let root = tempfile::tempdir().unwrap();
    let (path, raw, mut previous) = open(root.path(), Format::Ssc, &cases()[2]["input"]);
    edit(&mut previous.session);
    previous.save_recovery().unwrap();
    assert_eq!(fs::read(&path).unwrap(), raw);
    let mut loaded = FileSession::open(&path).unwrap();
    loaded
        .resolve_generation_profiles(None, &AtomicBool::new(false))
        .unwrap();
    assert!(loaded.session.is_dirty());
    assert!(loaded.session.undo_label().is_none());
    loaded.recover().unwrap();
    assert!(
        loaded
            .session
            .grid(0)
            .unwrap()
            .get(Position {
                row: 24,
                lane: 2,
                player: 0
            })
            .is_some()
    );
    assert!(loaded.session.is_dirty());
    assert!(loaded.session.undo_label().is_none());
    assert_eq!(
        ChartData::read(loaded.session.document(), 0)
            .unwrap()
            .unwrap()
            .expressed_chart_config
            .to_string(),
        DYNAMIC
    );
    assert!(
        loaded.recover().is_err(),
        "recovered user content is not an initial loader repair baseline"
    );
    let mut other = FileSession::open(&path).unwrap();
    other
        .resolve_generation_profiles(None, &AtomicBool::new(false))
        .unwrap();
    edit(&mut other.session);
    let before = other.session.serialize().unwrap();
    assert!(other.recover().is_err());
    assert_eq!(other.session.serialize().unwrap(), before);
    assert_eq!(other.session.undo_label(), Some("User note"));
    assert_eq!(fs::read(path).unwrap(), raw);
}

#[test]
fn expression_clean_baseline_still_invalidates_an_earlier_resolution_plan() {
    let root = tempfile::tempdir().unwrap();
    let (_, _, mut file) = open(root.path(), Format::Ssc, &cases()[1]["input"]);
    let cancelled = AtomicBool::new(false);
    let first = Plan::capture(&file.session, None, &cancelled).unwrap();
    let stale = Plan::capture(&file.session, None, &cancelled).unwrap();
    first.apply(&mut file.session, &cancelled).unwrap();
    assert!(!file.session.is_dirty());
    assert_eq!(file.session.revision(), 0);
    assert!(stale.apply(&mut file.session, &cancelled).is_err());
    assert!(file.session.undo_label().is_none());
}
#[test]
fn saving_a_repaired_baseline_still_compares_against_the_exact_original_raw_file() {
    let root = tempfile::tempdir().unwrap();
    let (path, raw, mut file) = open(root.path(), Format::Ssc, &cases()[2]["input"]);
    file.resolve_generation_profiles(None, &AtomicBool::new(false))
        .unwrap();
    let normalized = file.session.serialize().unwrap();
    assert_ne!(normalized, raw);
    fs::write(&path, &normalized).unwrap();
    assert!(
        file.save().is_err(),
        "external publication of normalized bytes must not replace the original raw CAS witness"
    );
    assert_eq!(fs::read(&path).unwrap(), normalized);
    assert_eq!(file.session.serialize().unwrap(), normalized);
    assert!(file.session.is_dirty());
    assert!(file.session.undo_label().is_none());
    fs::write(&path, &raw).unwrap();
    file.save().unwrap();
    assert!(!file.session.is_dirty());
    assert!(!file.has_external_change().unwrap());
}
