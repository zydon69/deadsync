use deadsync_editor::{
    advanced_export::{Limits, Options, Severity},
    document::{Document, Format},
    history::Session,
    output_policy::OutputPolicy,
    playback::Clock,
    storage::FileSession,
    timing_edit::{self, Scope, combo, label, region, scroll, speed},
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};
fn reference() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "fixtures/grooveauthor/resident-timing-sm.expected.json"
    ))
    .unwrap()
}
fn session() -> Session {
    Session::new(
        Document::parse(
            reference()["input"].as_str().unwrap().as_bytes(),
            Format::Sm,
        )
        .unwrap(),
    )
}
fn author(session: &mut Session) {
    assert!(
        session
            .edit_chart_atomic(0, "Authored timing events", |staged| {
                let mut regions = region::Draft::capture(staged, 0, Scope::Chart)?;
                regions.set(region::Kind::Warp, 48, 48)?;
                regions.set(region::Kind::Fake, 96, 48)?;
                regions.apply(staged, 0)?;
                let mut scroll = scroll::Draft::capture(staged, 0, Scope::Chart)?;
                scroll.set(144, 2.)?;
                scroll.apply(staged, 0)?;
                let mut speed = speed::Draft::capture(staged, 0, Scope::Chart)?;
                speed.set(
                    192,
                    speed::Speed {
                        rate: 3.,
                        period: speed::Period::Rows(96),
                    },
                )?;
                speed.apply(staged, 0)?;
                let mut combo = combo::Draft::capture(staged, 0, Scope::Chart)?;
                combo.set(240, combo::Multipliers { hit: 2, miss: 3 })?;
                combo.apply(staged, 0)?;
                let mut label = label::Draft::capture(staged, 0, Scope::Chart)?;
                label.set(288, "authored")?;
                label.apply(staged, 0)?;
                Ok(())
            })
            .unwrap()
    );
}
fn assert_authored(session: &Session, chart: usize) {
    let scope = Scope::Chart;
    assert_eq!(
        region::Draft::capture(session, chart, scope)
            .unwrap()
            .regions(region::Kind::Warp)
            .values()
            .get(&48),
        Some(&48)
    );
    assert_eq!(
        region::Draft::capture(session, chart, scope)
            .unwrap()
            .regions(region::Kind::Fake)
            .values()
            .get(&96),
        Some(&48)
    );
    assert_eq!(
        scroll::Draft::capture(session, chart, scope)
            .unwrap()
            .scrolls()
            .values()
            .get(&144),
        Some(&2.)
    );
    assert_eq!(
        speed::Draft::capture(session, chart, scope)
            .unwrap()
            .speeds()
            .values()[&192]
            .rate,
        3.
    );
    assert_eq!(
        combo::Draft::capture(session, chart, scope)
            .unwrap()
            .combos()
            .values()[&240],
        combo::Multipliers { hit: 2, miss: 3 }
    );
    assert_eq!(
        label::Labels::from_document(session.document(), chart)
            .unwrap()
            .values()
            .get(&288)
            .map(String::as_str),
        Some("authored")
    );
    let clock = Clock::capture(session, chart).unwrap();
    for event in reference()["authored"][0]["events"].as_array().unwrap() {
        assert_eq!(
            clock.chart_time(event["row"].as_u64().unwrap()).unwrap(),
            event["time"].as_f64().unwrap(),
            "actual source event {event}"
        );
    }
    let speeds = deadsync_editor::scroll_interpolation::Timeline::capture(session, chart).unwrap();
    assert_eq!(
        speeds
            .rate_at(240., clock.chart_time(240).unwrap())
            .unwrap(),
        3.
    );
    assert!(
        clock
            .spacing_sections()
            .any(|section| section.mapping.row() == 144. && section.scroll == 2.)
    );
}
fn output(session: &mut Session, format: Format) -> Result<Vec<u8>, deadsync_editor::Error> {
    let policy = OutputPolicy::Advanced {
        format,
        options: Options {
            require_identical_timing_in_sm: false,
            ..Default::default()
        },
        timing_chart: session.chart_ids()[0].clone(),
    };
    let prepared = policy.prepare_output(session, Limits::default(), &AtomicBool::new(false))?;
    if prepared
        .issues()
        .iter()
        .any(|issue| issue.severity == Severity::Error)
    {
        return Err(deadsync_editor::Error::InvalidEdit(format!(
            "{:?}",
            prepared.issues()
        )));
    }
    let acknowledged = prepared
        .issues()
        .iter()
        .enumerate()
        .filter_map(|(i, issue)| (issue.severity == Severity::Warning).then_some(i))
        .collect::<BTreeSet<_>>();
    Ok(prepared
        .generate(
            session,
            &acknowledged,
            16 * 1024 * 1024,
            &AtomicBool::new(false),
        )?
        .bytes()
        .to_vec())
}
#[test]
fn actual_sm_reader_ignored_tags_stay_opaque_and_authored_timing_matches_fumen_oracle() {
    let mut session = session();
    let original = session.serialize().unwrap();
    assert!(
        region::Draft::capture(&session, 0, Scope::Chart)
            .unwrap()
            .regions(region::Kind::Warp)
            .values()
            .is_empty()
    );
    assert!(
        label::Labels::from_document(session.document(), 0)
            .unwrap()
            .values()
            .is_empty()
    );
    assert_eq!(
        Clock::capture(&session, 0).unwrap().chart_time(96).unwrap(),
        1.
    );
    assert_eq!(session.serialize().unwrap(), original);
    author(&mut session);
    assert_authored(&session, 0);
    assert_eq!(
        Clock::capture(&session, 1).unwrap().chart_time(96).unwrap(),
        1.
    );
    assert_eq!(session.document().format, Format::Sm);
    assert_eq!(
        session.document().raw_property(None, "WARPS"),
        Some(b"1=100".as_slice())
    );
    assert!(!timing_edit::supports_chart_timing(session.document()));
    assert!(timing_edit::uses_chart_timing(session.document(), 0));
    assert!(!timing_edit::uses_chart_timing(session.document(), 1));
    let authored = session.serialize().unwrap();
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original);
    assert_eq!(
        Clock::capture(&session, 0).unwrap().chart_time(96).unwrap(),
        1.
    );
    session.redo().unwrap();
    assert_eq!(session.serialize().unwrap(), authored);
    assert_authored(&session, 0);
    let ssc = output(&mut session, Format::Ssc).unwrap();
    let reopened = Session::new(Document::parse(ssc, Format::Ssc).unwrap());
    assert_authored(&reopened, 0);
    assert_eq!(
        Clock::capture(&reopened, 1)
            .unwrap()
            .chart_time(96)
            .unwrap(),
        1.
    );
    assert!(
        output(&mut session, Format::Sm).is_err(),
        "warp omission still requires an explicit conversion/review"
    );
}
#[test]
fn authored_timing_survives_native_save_recovery_reopen_and_cross_chart_clipboard() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("native.sm");
    std::fs::write(&path, reference()["input"].as_str().unwrap()).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    author(&mut file.session);
    file.save_recovery().unwrap();
    let mut recovery = FileSession::open(&path).unwrap();
    recovery.recover().unwrap();
    assert_authored(&recovery.session, 0);
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(reopened.session.document().format, Format::Sm);
    assert_authored(&reopened.session, 0);
    let before = reopened.session.serialize().unwrap();
    let copy = deadsync_editor::event_clipboard::Clipboard::copy(&mut reopened.session, 0, 48, 288)
        .unwrap()
        .unwrap();
    assert!(copy.paste(&mut reopened.session, 1, 48, 0).unwrap());
    assert_authored(&reopened.session, 1);
    reopened.session.undo().unwrap();
    assert_eq!(reopened.session.serialize().unwrap(), before);
    reopened.session.redo().unwrap();
    assert_authored(&reopened.session, 1);
}
#[test]
fn resident_chart_identity_survives_duplicate_generated_append_deletion_and_empty_creation() {
    let mut session = session();
    author(&mut session);
    let duplicate = session.duplicate_chart(0, session.revision()).unwrap();
    assert_authored(&session, duplicate);
    let grid = session.grid(0).unwrap().clone();
    let appended = session
        .append_chart(
            0,
            "dance-single",
            &grid,
            session.revision(),
            "Generate clone",
        )
        .unwrap();
    assert_authored(&session, appended);
    let empty = deadsync_editor::chart_creation::Draft::capture(&session)
        .unwrap()
        .apply(&mut session, "dance-single")
        .unwrap();
    assert!(
        region::Draft::capture(&session, empty, Scope::Chart)
            .unwrap()
            .regions(region::Kind::Warp)
            .values()
            .is_empty()
    );
    assert_eq!(
        Clock::capture(&session, empty)
            .unwrap()
            .chart_time(96)
            .unwrap(),
        1.
    );
    let before = session.serialize().unwrap();
    let mut deletion = deadsync_editor::chart_deletion::Draft::capture(&session, 0).unwrap();
    let order = (0..session.document().charts().len()).collect::<Vec<_>>();
    deletion
        .prepare_song_timing(
            &session,
            &deadsync_editor::open_preferences::Preferences::default(),
            &order,
        )
        .unwrap();
    deletion.apply(&mut session).unwrap();
    assert_authored(&session, duplicate - 1);
    assert_authored(&session, appended - 1);
    assert_eq!(
        Clock::capture(&session, 0).unwrap().chart_time(96).unwrap(),
        1.
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    assert_authored(&session, 0);
}
#[test]
fn chart_copy_and_negative_stop_conversion_use_owner_and_preserve_atomic_guards() {
    let mut session = session();
    author(&mut session);
    use deadsync_editor::chart_event_copy::{EventType, Plan};
    let before = session.serialize().unwrap();
    let plan = Plan::capture(
        &mut session,
        0,
        [1],
        [
            EventType::Warp,
            EventType::Fake,
            EventType::Scroll,
            EventType::Speed,
            EventType::Combo,
            EventType::Label,
        ],
    )
    .unwrap();
    assert!(plan.apply(&mut session).unwrap().changed);
    assert_authored(&session, 1);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    let mut stale = region::Draft::capture(&session, 1, Scope::Chart).unwrap();
    stale.set(region::Kind::Warp, 384, 48).unwrap();
    session.set_text(None, "TITLE", "interleave").unwrap();
    let current = session.serialize().unwrap();
    assert!(stale.apply(&mut session, 1).is_err());
    assert_eq!(session.serialize().unwrap(), current);
    let mut stops = timing_edit::pause::Draft::capture(&session, 1, Scope::Chart).unwrap();
    stops
        .set(timing_edit::pause::Kind::Stop, 384, -0.5)
        .unwrap();
    stops.apply(&mut session, 1).unwrap();
    let original = session.serialize().unwrap();
    let conversion = timing_edit::conversion::Plan::prepare(
        &session,
        1,
        timing_edit::conversion::Direction::NegativeStopsToWarps,
        None,
    )
    .unwrap();
    conversion.apply(&mut session, &BTreeSet::new()).unwrap();
    assert!(
        region::Draft::capture(&session, 1, Scope::Chart)
            .unwrap()
            .regions(region::Kind::Warp)
            .values()
            .contains_key(&384)
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), original);
}

#[test]
fn sm_output_keeps_its_explicit_conversion_and_loss_review_boundary() {
    let mut session = session();
    author(&mut session);
    assert!(output(&mut session, Format::Sm).is_err());
    let plan = timing_edit::conversion::Plan::prepare(
        &session,
        0,
        timing_edit::conversion::Direction::WarpsToNegativeStops,
        None,
    )
    .unwrap();
    plan.apply(&mut session, &BTreeSet::new()).unwrap();
    let bytes = output(&mut session, Format::Sm).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    assert!(!text.to_ascii_uppercase().contains("#DSTIMING"));
    assert!(!text.to_ascii_uppercase().contains("#WARPS:"));
    let reopened = Session::new(Document::parse(bytes, Format::Sm).unwrap());
    assert_eq!(
        Clock::capture(&reopened, 0)
            .unwrap()
            .chart_time(384)
            .unwrap(),
        3.5
    );
    assert_eq!(
        session.document().raw_property(None, "WARPS"),
        Some(b"1=100".as_slice())
    );
}
