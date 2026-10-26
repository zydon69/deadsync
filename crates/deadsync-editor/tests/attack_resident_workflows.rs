use deadsync_editor::{
    attacks::{inspector::Draft, placement::Timeline},
    document::{Document, Format},
    history::Session,
    timing_edit::{
        Scope, TempoDraft,
        region::{Draft as Regions, Kind},
    },
};
fn source() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy:TIME=1.5:LEN=3:MODS=50% Drunk;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;".as_slice(),Format::Ssc).unwrap())
}
fn hide(s: &mut Session) {
    let mut draft = Regions::capture(s, 0, Scope::Song).unwrap();
    draft.set(Kind::Warp, 0, 192).unwrap();
    draft.apply(s, 0).unwrap();
}
fn rows(s: &Session, chart: usize) -> Vec<u64> {
    Timeline::capture(s, chart)
        .unwrap()
        .markers()
        .iter()
        .map(|m| m.row)
        .collect()
}
#[test]
fn coincident_export_times_keep_distinct_rows_modifiers_and_inspector_selection() {
    let mut s = source();
    hide(&mut s);
    assert_eq!(rows(&s, 0), [96, 144]);
    assert!(Timeline::capture(&s, 0).unwrap().has_resident_rows());
    let mut draft = Draft::capture(&s, 0, Scope::Chart).unwrap();
    assert_eq!(draft.event_for_source(1), Some(1));
    let event = &draft.attacks().events()[1];
    let mut value = event.modifiers[0].clone();
    value.name = "Écran clair".into();
    let time = event.time;
    draft.set(1, 0, time, value).unwrap();
    draft.apply(&mut s, 0).unwrap();
    let bytes = s.serialize().unwrap();
    let reopened = Session::new(Document::parse(bytes.clone(), Format::Ssc).unwrap());
    assert_eq!(rows(&reopened, 0), [96, 144]);
    let timeline = Timeline::capture(&reopened, 0).unwrap();
    assert_eq!(timeline.markers()[0].modifiers[0].length, 2.0);
    assert_eq!(timeline.markers()[1].modifiers[0].name, "Écran clair");
    assert_eq!(timeline.markers()[1].modifiers[0].length, 3.0);
    let mut draft = Draft::capture(&reopened, 0, Scope::Chart).unwrap();
    assert!(!draft.has_changes());
    let mut reopened = reopened;
    assert!(!draft.apply(&mut reopened, 0).unwrap());
    assert_eq!(reopened.serialize().unwrap(), bytes);
    draft.remove(0).unwrap();
    draft.apply(&mut reopened, 0).unwrap();
    assert_eq!(rows(&reopened, 0), [144]);
    reopened.undo().unwrap();
    assert_eq!(reopened.serialize().unwrap(), bytes);
}
#[test]
fn future_corrupt_or_mismatched_metadata_cannot_modify_timing_or_erase_redo() {
    for mutation in ["future", "invalid-row", "clock-field", "modifier"] {
        let mut s = source();
        hide(&mut s);
        let mut json: serde_json::Value =
            serde_json::from_str(&s.document().text(Some(0), "DSAttackRows").unwrap()).unwrap();
        match mutation {
            "future" => json["version"] = 2.into(),
            "invalid-row" => json["events"][0]["row"] = u64::MAX.into(),
            "clock-field" => json["timing"][0]["unknown"] = true.into(),
            _ => json["events"][0]["modifiers"][0]["name"] = "Tampered".into(),
        };
        s.set_text(Some(0), "DSAttackRows", &json.to_string())
            .unwrap();
        assert!(Timeline::capture(&s, 0).is_err());
        s.set_text(None, "TITLE", "Redo").unwrap();
        s.undo().unwrap();
        let before = s.serialize().unwrap();
        let redo = s.redo_label().unwrap().to_owned();
        let mut draft = TempoDraft::capture(&s, 0, Scope::Song).unwrap();
        draft.set(0, 240.0).unwrap();
        assert!(draft.apply(&mut s, 0).is_err());
        assert_eq!(s.serialize().unwrap(), before);
        assert_eq!(s.redo_label(), Some(redo.as_str()));
    }
}
#[test]
fn externally_changed_attack_property_invalidates_the_exact_source_binding() {
    let mut s = source();
    hide(&mut s);
    let bytes = String::from_utf8(s.serialize().unwrap()).unwrap();
    let document = Document::parse(bytes.as_bytes(), Format::Ssc).unwrap();
    let range = document.tag(Some(0), "ATTACKS").unwrap().value.clone();
    let attack = bytes[range.clone()].replacen("Dizzy", "Reverse", 1);
    let mut changed = bytes.clone();
    changed.replace_range(range, &attack);
    assert_ne!(changed, bytes);
    let reopened = Session::new(Document::parse(changed.into_bytes(), Format::Ssc).unwrap());
    let timeline = Timeline::capture(&reopened, 0).unwrap();
    assert!(!timeline.has_resident_rows());
    assert_eq!(timeline.markers()[0].modifiers[0].name, "Reverse");
}
#[test]
fn duplicate_and_delete_preserve_resident_chart_ownership_and_exact_undo() {
    let mut s = source();
    hide(&mut s);
    let original = s.serialize().unwrap();
    let duplicate = s.duplicate_chart(0, s.revision()).unwrap();
    assert_eq!(duplicate, 1);
    assert_eq!(rows(&s, 1), [96, 144]);
    let duplicated = s.serialize().unwrap();
    deadsync_editor::chart_deletion::Draft::capture(&s, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(rows(&s, 0), [96, 144]);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), duplicated);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), original);
}
#[test]
fn sm_negative_stop_rows_survive_inspection_save_duplicate_and_chart_reindex() {
    let mut s=Session::new(Document::parse(b"#BPMS:0=120;#STOPS:0=-2;#ATTACKS:TIME=1:LEN=1:MODS=100% Dizzy;#NOTES:dance-single::Hard:9::1000;".as_slice(),Format::Sm).unwrap());
    let mut draft = Draft::capture(&s, 0, Scope::Song).unwrap();
    let value = draft.attacks().events()[0].modifiers[0].clone();
    draft.set_at_row(0, 0, 96, value).unwrap();
    draft.apply(&mut s, 0).unwrap();
    assert_eq!(rows(&s, 0), [96]);
    assert!(Timeline::capture(&s, 0).unwrap().has_resident_rows());
    let bytes = s.serialize().unwrap();
    let mut s = Session::new(Document::parse(bytes, Format::Sm).unwrap());
    s.duplicate_chart(0, s.revision()).unwrap();
    assert_eq!(rows(&s, 1), [96]);
    deadsync_editor::chart_deletion::Draft::capture(&s, 0)
        .unwrap()
        .apply(&mut s)
        .unwrap();
    assert_eq!(rows(&s, 0), [96]);
    assert!(s.document().text(None, "DSAttackRowsCHART0000").is_some());
    assert!(s.document().text(None, "DSAttackRowsCHART0001").is_none());
}

#[test]
fn resident_clock_and_standard_file_projection_match_original_grooveauthor_and_fumen() {
    let mut s = source();
    hide(&mut s);
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-resident-clock.expected.json"
    ))
    .unwrap();
    let timeline = Timeline::capture(&s, 0).unwrap();
    let draft = Draft::capture(&s, 0, Scope::Chart).unwrap();
    let mut comparisons = 0;
    for (index, (marker, reference)) in timeline
        .markers()
        .iter()
        .zip(expected.as_array().unwrap())
        .enumerate()
    {
        assert_eq!(marker.row, reference["Row"].as_u64().unwrap());
        assert_eq!(marker.chart_time, reference["Time"].as_f64().unwrap());
        for (modifier, transport) in reference["Transport"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert_eq!(
                draft.end_row(index, modifier).unwrap(),
                transport["EndRow"].as_f64().unwrap().max(0.0).round() as u64
            );
            comparisons += 1;
            for cursor in transport["Cursors"].as_array().unwrap() {
                assert_eq!(
                    draft
                        .length_to_cursor(index, cursor["Row"].as_u64().unwrap())
                        .unwrap(),
                    cursor["Length"].as_f64().unwrap()
                );
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 14);
    let bytes = s.serialize().unwrap();
    let doc = Document::parse(bytes.clone(), Format::Ssc).unwrap();
    let range = doc.tag(Some(0), "DSAttackRows").unwrap().value.clone();
    let mut projection = bytes;
    projection.drain(range);
    let game = Session::new(Document::parse(projection, Format::Ssc).unwrap());
    let actual = Timeline::capture(&game, 0).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-resident-file.expected.json"
    ))
    .unwrap();
    let annotated = Session::new(
        Document::parse(
            expected[0]["Content"].as_str().unwrap().as_bytes(),
            Format::Ssc,
        )
        .unwrap(),
    );
    assert_eq!(rows(&annotated, 0), [96, 144]);
    // Exercise the same RSSP dependency as DeadSync's song/analysis parser.
    // Supply ordinary chart descriptors in these otherwise minimal fixtures.
    let analyze = |session: &Session| {
        let bytes = String::from_utf8(session.serialize().unwrap())
            .unwrap()
            .replace(
                "#STEPSTYPE:dance-single;",
                "#STEPSTYPE:dance-single;#DIFFICULTY:Hard;#METER:9;",
            )
            // Keep a playable note beyond the warp in this RSSP comparison.
            .replace("#NOTES:1000;", "#NOTES:1000,\n0000,\n0100;");
        deadsync_editor::analysis::chart_bytes(
            bytes.as_bytes(),
            Format::Ssc,
            0,
            &rssp::AnalysisOptions::default(),
        )
        .unwrap()
    };
    let with_metadata = analyze(&annotated);
    let mut plain = annotated.serialize().unwrap();
    let plain_document = Document::parse(plain.clone(), Format::Ssc).unwrap();
    let metadata_range = plain_document
        .tag(Some(0), "DSAttackRows")
        .unwrap()
        .value
        .clone();
    plain.drain(metadata_range);
    let plain = Session::new(Document::parse(plain, Format::Ssc).unwrap());
    let without_metadata = analyze(&plain);
    assert!(with_metadata.stats.total_arrows > 0);
    assert_eq!(with_metadata.short_hash, without_metadata.short_hash);
    assert_eq!(with_metadata.stats, without_metadata.stats);
    assert_eq!(
        with_metadata.timing_segments.bpms,
        without_metadata.timing_segments.bpms
    );
    let events = expected[0]["Charts"][0]["Attacks"].as_array().unwrap();
    assert_eq!(actual.markers().len(), events.len());
    for (marker, event) in actual.markers().iter().zip(events) {
        assert_eq!(marker.row, event["Row"].as_u64().unwrap());
        assert_eq!(
            marker.modifiers.len(),
            event["Modifiers"].as_array().unwrap().len()
        );
        for (modifier, reference) in marker
            .modifiers
            .iter()
            .zip(event["Modifiers"].as_array().unwrap())
        {
            assert_eq!(modifier.name, reference["Name"].as_str().unwrap());
            assert_eq!(modifier.length, reference["Length"].as_f64().unwrap());
            assert_eq!(modifier.level, reference["Level"].as_f64().unwrap());
            assert_eq!(modifier.speed, reference["Speed"].as_f64().unwrap());
        }
    }
}
#[test]
fn recovery_and_atomic_disk_save_retain_hidden_rows_without_changing_original_before_save() {
    use deadsync_editor::storage::FileSession;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    let original = source().serialize().unwrap();
    std::fs::write(&path, &original).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    hide(&mut file.session);
    file.save_recovery().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let mut recovered = FileSession::open(&path).unwrap();
    recovered.recover().unwrap();
    assert!(recovered.session.is_dirty());
    assert_eq!(rows(&recovered.session, 0), [96, 144]);
    recovered.save().unwrap();
    let opened = FileSession::open(&path).unwrap();
    assert_eq!(rows(&opened.session, 0), [96, 144]);
}

#[test]
fn external_timing_or_offset_changes_invalidate_resident_rows_but_typed_edits_refresh_binding() {
    let mut s = source();
    hide(&mut s);
    let saved = String::from_utf8(s.serialize().unwrap()).unwrap();
    let changed = saved.replacen("#BPMS:0=120;", "#BPMS:0=240;", 1);
    let raw = Session::new(Document::parse(changed.into_bytes(), Format::Ssc).unwrap());
    assert!(!Timeline::capture(&raw, 0).unwrap().has_resident_rows());
    assert_eq!(rows(&raw, 0), [0]);
    let changed = saved.replacen("#VERSION:0.83;", "#VERSION:0.83;#OFFSET:0.5;", 1);
    let raw = Session::new(Document::parse(changed.into_bytes(), Format::Ssc).unwrap());
    assert!(!Timeline::capture(&raw, 0).unwrap().has_resident_rows());
    let mut tempo = TempoDraft::capture(&s, 0, Scope::Song).unwrap();
    tempo.set(0, 240.0).unwrap();
    tempo.apply(&mut s, 0).unwrap();
    assert_eq!(rows(&s, 0), [96, 144]);
    assert!(Timeline::capture(&s, 0).unwrap().has_resident_rows());
    let mut scroll =
        deadsync_editor::timing_edit::scroll::Draft::capture(&s, 0, Scope::Song).unwrap();
    scroll.set(0, -1.0).unwrap();
    scroll.apply(&mut s, 0).unwrap();
    assert_eq!(rows(&s, 0), [96, 144]);
    assert!(Timeline::capture(&s, 0).unwrap().has_resident_rows());
    let reopened = Session::new(Document::parse(s.serialize().unwrap(), Format::Ssc).unwrap());
    assert_eq!(rows(&reopened, 0), [96, 144]);
}

#[test]
fn shared_attack_edit_preserves_the_active_local_resident_rows_and_metadata_bytes() {
    let mut s = source();
    hide(&mut s);
    let before = s.serialize().unwrap();
    let metadata = s.document().text(Some(0), "DSAttackRows").unwrap();
    let mut shared = Draft::capture(&s, 0, Scope::Song).unwrap();
    let event = &shared.attacks().events()[0];
    let mut value = event.modifiers[0].clone();
    value.name = "Reverse".into();
    let time = event.time;
    shared.set(0, 0, time, value).unwrap();
    shared.apply(&mut s, 0).unwrap();
    assert_eq!(rows(&s, 0), [96, 144]);
    assert_eq!(
        s.document().text(Some(0), "DSAttackRows").unwrap(),
        metadata
    );
    assert_eq!(
        Timeline::capture(&s, 0).unwrap().markers()[0].modifiers[0].name,
        "Dizzy"
    );
    assert_eq!(
        Draft::capture(&s, 0, Scope::Song)
            .unwrap()
            .attacks()
            .events()[0]
            .modifiers[0]
            .name,
        "Reverse"
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn shared_and_local_hidden_positions_coexist_save_and_undo_independently() {
    let bytes=b"#VERSION:0.83;#BPMS:0=120;#WARPS:0=4;#ATTACKS:TIME=1:LEN=2:MODS=100% Dizzy:TIME=1.5:LEN=3:MODS=50% Drunk;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;";
    let mut s = Session::new(Document::parse(bytes.as_slice(), Format::Ssc).unwrap());
    let mut shared = Draft::capture(&s, 0, Scope::Song).unwrap();
    let value = shared.attacks().events()[0].modifiers[0].clone();
    shared.set_at_row(0, 0, 48, value).unwrap();
    shared.apply(&mut s, 0).unwrap();
    assert!(s.document().text(Some(0), "DSAttackRowsSong").is_some());
    assert_eq!(rows(&s, 0), [48, 336]);
    let mut local = Draft::capture(&s, 0, Scope::Chart).unwrap();
    let value = local.attacks().events()[0].modifiers[0].clone();
    local.set_at_row(0, 0, 96, value).unwrap();
    local.apply(&mut s, 0).unwrap();
    assert!(s.document().text(Some(0), "DSAttackRows").is_some());
    assert_eq!(rows(&s, 0), [96, 336]);
    assert_eq!(
        Draft::capture(&s, 0, Scope::Song)
            .unwrap()
            .start_row(0)
            .unwrap(),
        48
    );
    let saved = s.serialize().unwrap();
    let mut s = Session::new(Document::parse(saved.clone(), Format::Ssc).unwrap());
    let mut shared = Draft::capture(&s, 0, Scope::Song).unwrap();
    let event = &shared.attacks().events()[0];
    let mut value = event.modifiers[0].clone();
    value.speed = 2.0;
    let time = event.time;
    shared.set(0, 0, time, value).unwrap();
    shared.apply(&mut s, 0).unwrap();
    assert_eq!(
        Draft::capture(&s, 0, Scope::Song)
            .unwrap()
            .start_row(0)
            .unwrap(),
        48
    );
    assert_eq!(rows(&s, 0), [96, 336]);
    assert_eq!(
        Timeline::capture(&s, 0).unwrap().markers()[0].modifiers[0].speed,
        1.0
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), saved);
}
