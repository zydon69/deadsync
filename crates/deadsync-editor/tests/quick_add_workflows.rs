use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    quick_add::{Kind, Plan, Refusal, add_row},
    timing_edit::{self, Scope},
};
fn session() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#TITLE:Quick;#BPMS:0=120,2=180;#TIMESIGNATURES:0=4=4;#NOTEDATA:;#STEPSTYPE:dance-single;#DESCRIPTION:;#DIFFICULTY:Edit;#METER:1;#NOTES:\n0000\n0000\n0000\n0000\n;".as_slice(),Format::Ssc).unwrap())
}
#[test]
fn source_nearest_row_ties_round_down_then_clamp() {
    for (input, expected) in [
        (-50.5, 0),
        (-0.1, 0),
        (0.5, 0),
        (1.5, 1),
        (1.5000001, 2),
        (47.5, 47),
        (47.6, 48),
    ] {
        assert_eq!(add_row(input).unwrap(), expected);
    }
    for v in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 2147483648.0] {
        assert_eq!(add_row(v).unwrap_err().reason, Refusal::InvalidPosition);
    }
}
#[test]
fn every_quick_add_preserves_origin_and_refuses_same_type_collision() {
    for kind in [
        Kind::Tempo,
        Kind::Signature,
        Kind::Scroll,
        Kind::Speed,
        Kind::Stop,
        Kind::Delay,
        Kind::Warp,
        Kind::Fake,
        Kind::Ticks,
        Kind::Combo,
        Kind::Label,
        Kind::Attack,
        Kind::Pattern,
    ] {
        let mut s = session();
        let before = s.document().text(Some(0), "BPMS");
        let plan = Plan::prepare_with_seed(&s, 0, kind, 192.0, 42).unwrap();
        assert_eq!(s.document().text(Some(0), "BPMS"), before);
        assert!(plan.apply(&mut s, 0).unwrap(), "{kind:?}");
        assert_eq!(
            Plan::prepare(&s, 0, kind, 192.0).err().unwrap().reason,
            Refusal::Occupied,
            "{kind:?}"
        );
        s.undo().unwrap();
        assert!(Plan::prepare(&s, 0, kind, 192.0).is_ok(), "{kind:?}");
        s.redo().unwrap();
        assert_eq!(
            Plan::prepare(&s, 0, kind, 192.0).err().unwrap().reason,
            Refusal::Occupied,
            "{kind:?}"
        );
    }
}
#[test]
fn source_factory_defaults_are_distinct_from_inspector_defaults() {
    let mut s = session();
    Plan::prepare(&s, 0, Kind::Speed, 192.0)
        .unwrap()
        .apply(&mut s, 0)
        .unwrap();
    let d = timing_edit::speed::Draft::capture(&s, 0, Scope::Chart).unwrap();
    assert_eq!(
        d.speeds().values()[&192],
        timing_edit::speed::Speed {
            rate: 1.0,
            period: timing_edit::speed::Period::Rows(48)
        }
    );
    Plan::prepare(&s, 0, Kind::Stop, 192.0)
        .unwrap()
        .apply(&mut s, 0)
        .unwrap();
    let d = timing_edit::pause::Draft::capture(&s, 0, Scope::Chart).unwrap();
    assert!((d.pauses(timing_edit::pause::Kind::Stop).values()[&192] - 1.0 / 3.0).abs() < 1e-12);
    Plan::prepare_with_seed(&s, 0, Kind::Pattern, 192.0, 42)
        .unwrap()
        .apply(&mut s, 0)
        .unwrap();
    let data =
        deadsync_editor::generation::metadata::ChartData::existing_or_initial(s.document(), 0)
            .unwrap();
    let p = &data.patterns.unwrap()[&192];
    assert_eq!(p.length, 192);
    assert_eq!(p.random_seed, 42);
    assert!(p.start_position_inclusive);
    assert!(!p.end_position_inclusive);
    assert!(!p.ignore_preceding_distribution);
}
#[test]
fn stale_wrong_chart_foreign_and_new_edit_then_undo_are_refused() {
    let mut s = session();
    let p = Plan::prepare(&s, 0, Kind::Ticks, 192.0).unwrap();
    assert!(!p.is_current(&session(), 0));
    assert!(!p.is_current(&s, 1));
    s.set_text(None, "TITLE", "later").unwrap();
    s.undo().unwrap();
    assert_eq!(p.apply(&mut s, 0).err().unwrap().reason, Refusal::Stale);
    assert!(
        timing_edit::tick::Draft::capture(&s, 0, Scope::Chart)
            .unwrap()
            .counts()
            .values()
            .get(&192)
            .is_none()
    );
}
#[test]
fn actual_source_factories_match_all_fifty_two_native_outputs() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/quick-add.expected.json")).unwrap();
    for case in fixture["Cases"].as_array().unwrap() {
        let method = case["Method"].as_str().unwrap();
        let bpm = case["Bpm"].as_f64().unwrap();
        let e = &case["Event"];
        let f = &e["Fields"];
        let kind = match method {
            "CreateTempoEvent" => Kind::Tempo,
            "CreateTimeSignatureEvent" => Kind::Signature,
            "CreateScrollRateEvent" => Kind::Scroll,
            "CreateInterpolatedScrollRateEvent" => Kind::Speed,
            "CreateStopEvent" => Kind::Stop,
            "CreateDelayEvent" => Kind::Delay,
            "CreateWarpEvent" => Kind::Warp,
            "CreateFakeRegionEvent" => Kind::Fake,
            "CreateTicksEvent" => Kind::Ticks,
            "CreateComboMultipliersEvent" => Kind::Combo,
            "CreateLabelEvent" => Kind::Label,
            "CreateAttackEvent" => Kind::Attack,
            "CreatePatternEvent" => Kind::Pattern,
            _ => panic!("source method"),
        };
        let mut s = session();
        let mut tempo = timing_edit::TempoDraft::capture(&s, 0, Scope::Chart).unwrap();
        tempo.set(96, bpm).unwrap();
        tempo.apply(&mut s, 0).unwrap();
        let seed = e["Pattern"]["RandomSeed"].as_i64().unwrap_or(42) as i32;
        Plan::prepare_with_seed(&s, 0, kind, 192.0, seed)
            .unwrap()
            .apply(&mut s, 0)
            .unwrap();
        let near = |a: f64, b: f64| assert!((a - b).abs() < 1e-11, "{method}: {a} != {b}");
        match kind {
            Kind::Tempo => near(
                timing_edit::TempoDraft::capture(&s, 0, Scope::Chart)
                    .unwrap()
                    .tempos()
                    .values()[&192],
                f["TempoBPM"].as_f64().unwrap(),
            ),
            Kind::Signature => {
                let d = timing_edit::signature::Draft::capture(&s, 0, Scope::Chart).unwrap();
                let v = d.signatures().values()[&192];
                assert_eq!(
                    v.numerator,
                    f["Signature"]["Numerator"].as_u64().unwrap() as u32
                );
                assert_eq!(
                    v.denominator,
                    f["Signature"]["Denominator"].as_u64().unwrap() as u32
                );
            }
            Kind::Scroll => near(
                timing_edit::scroll::Draft::capture(&s, 0, Scope::Chart)
                    .unwrap()
                    .scrolls()
                    .values()[&192],
                f["Rate"].as_f64().unwrap(),
            ),
            Kind::Speed => {
                let d = timing_edit::speed::Draft::capture(&s, 0, Scope::Chart).unwrap();
                let v = d.speeds().values()[&192];
                near(v.rate, f["Rate"].as_f64().unwrap());
                assert_eq!(
                    v.period,
                    timing_edit::speed::Period::Rows(
                        f["PeriodLengthIntegerPosition"].as_i64().unwrap() as i32
                    )
                );
            }
            Kind::Stop | Kind::Delay => {
                let k = if kind == Kind::Stop {
                    timing_edit::pause::Kind::Stop
                } else {
                    timing_edit::pause::Kind::Delay
                };
                near(
                    timing_edit::pause::Draft::capture(&s, 0, Scope::Chart)
                        .unwrap()
                        .pauses(k)
                        .values()[&192],
                    f["LengthSeconds"].as_f64().unwrap(),
                );
            }
            Kind::Warp | Kind::Fake => {
                let k = if kind == Kind::Warp {
                    timing_edit::region::Kind::Warp
                } else {
                    timing_edit::region::Kind::Fake
                };
                assert_eq!(
                    timing_edit::region::Draft::capture(&s, 0, Scope::Chart)
                        .unwrap()
                        .regions(k)
                        .values()[&192],
                    f["LengthIntegerPosition"].as_u64().unwrap()
                );
            }
            Kind::Ticks => assert_eq!(
                timing_edit::tick::Draft::capture(&s, 0, Scope::Chart)
                    .unwrap()
                    .counts()
                    .values()[&192],
                f["Ticks"].as_i64().unwrap() as i32
            ),
            Kind::Combo => {
                let d = timing_edit::combo::Draft::capture(&s, 0, Scope::Chart).unwrap();
                let v = d.combos().values()[&192];
                assert_eq!(v.hit, f["HitMultiplier"].as_i64().unwrap() as i32);
                assert_eq!(v.miss, f["MissMultiplier"].as_i64().unwrap() as i32);
            }
            Kind::Label => assert_eq!(
                timing_edit::label::Draft::capture(&s, 0, Scope::Chart)
                    .unwrap()
                    .labels()
                    .values()[&192],
                f["Text"].as_str().unwrap()
            ),
            Kind::Attack => {
                let d = deadsync_editor::attacks::inspector::Draft::capture(&s, 0, Scope::Chart)
                    .unwrap();
                let v = &d
                    .attacks()
                    .events()
                    .iter()
                    .find(|v| v.row == 192)
                    .unwrap()
                    .modifiers[0];
                let m = &f["Modifiers"][0];
                assert_eq!(v.name, m["Name"].as_str().unwrap());
                near(v.level, m["Level"].as_f64().unwrap());
                near(v.speed, m["Speed"].as_f64().unwrap());
                near(v.length, m["LengthSeconds"].as_f64().unwrap());
            }
            Kind::Pattern => {
                let d = deadsync_editor::generation::metadata::ChartData::existing_or_initial(
                    s.document(),
                    0,
                )
                .unwrap();
                let v = serde_json::to_value(&d.patterns.as_ref().unwrap()[&192]).unwrap();
                assert_eq!(v, e["Pattern"]);
            }
        }
    }
    for p in fixture["Positions"].as_array().unwrap() {
        assert_eq!(
            add_row(p["Position"].as_f64().unwrap()).unwrap(),
            p["Row"].as_u64().unwrap()
        );
    }
    assert_eq!(fixture["Collision"]["Same"], false);
    assert_eq!(fixture["Collision"]["Different"], true);
    let seeds = fixture["Seeds"].as_array().unwrap();
    assert!(seeds.iter().all(|s| {
        s.as_i64()
            .is_some_and(|s| (0..i32::MAX as i64).contains(&s))
    }));
}
