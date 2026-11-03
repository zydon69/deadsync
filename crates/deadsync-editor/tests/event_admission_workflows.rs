use deadsync_editor::region_selection::admission::{Admission, Kind};
use serde_json::Value;
#[test]
fn original_forward_loop_admits_combined_notes_misc_rates_and_crossing_holds() {
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/event-admission-source.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let max = case["max"].as_i64().unwrap() as i32;
        let pre = case["pre"].as_u64().unwrap() as usize;
        let mut admission = Admission::new(max, pre, case["draw"].as_bool().unwrap());
        let mut admitted: Vec<i64> = (1..=pre).map(|i| -(i as i64)).collect();
        let mut layout = vec![];
        let mut rates = 1;
        let mut edge = false;
        for (id, kind) in case["kinds"].as_array().unwrap().iter().enumerate() {
            let kind = kind.as_u64().unwrap();
            // The oracle host makes kind4 exceed the far edge. Source still
            // traverses it under negative scroll, before testing its quota.
            if kind != 2
                && (kind == 4
                    || case["ys"]
                        .as_array()
                        .is_some_and(|ys| ys[id].as_f64().unwrap() - 32.0 > 200.0))
                && !case["negative"].as_bool().unwrap()
            {
                edge = true;
                break;
            }
            let decision = admission.offer(match kind {
                0 | 3 => Kind::Lane,
                1 => Kind::Misc,
                2 => Kind::NextRate,
                _ => Kind::Other,
            });
            if kind == 2 {
                rates += 1;
            }
            if decision.admit {
                admitted.push(id as i64);
                if kind == 1 || kind == 2 {
                    layout.push(id as i64);
                }
            }
            if decision.stop {
                break;
            }
        }
        assert_eq!(
            serde_json::json!(admitted),
            case["admitted"],
            "{}",
            case["name"]
        );
        assert_eq!(
            serde_json::json!(layout),
            case["layout"],
            "{}",
            case["name"]
        );
        assert_eq!(rates, case["rates"].as_i64().unwrap(), "{}", case["name"]);
        assert_eq!(
            edge,
            case["reachedEndOfScreen"].as_bool().unwrap(),
            "{}",
            case["name"]
        );
    }
}
#[test]
fn source_loop_fixtures_drive_real_lazy_grid_and_viewport_plan() {
    use deadsync_editor::{
        document::{Document, Format},
        event_clipboard::{Event, TimingKind},
        generation::editor_timing::SpacingSection,
        history::Session,
        notes::{Grid, Note, Position},
        playback::Clock,
        rate_mapping::RateMapping,
        region_selection::{
            inventory::Entry,
            visible_events::{self, Viewport},
        },
        spacing::{Anchor, Speeds},
        spacing_index::Index,
        spacing_search::Settings,
        zoom::SpacingMode,
    };
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/event-admission-source.json"
    ))
    .unwrap();
    for case in cases
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["ys"].is_array())
    {
        let session = Session::new(
            Document::parse(
                b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;"
                    .to_vec(),
                Format::Ssc,
            )
            .unwrap(),
        );
        let clock = Clock::capture(&session, 0).unwrap();
        let mut grid = Grid::new(4, 1).unwrap();
        for row in [12, 24, 36, 120] {
            grid.set(
                Position {
                    player: 0,
                    lane: 0,
                    row,
                },
                Some(Note::tap()),
            )
            .unwrap();
        }
        let index = Index::new(
            [(0, 0), (0, 1), (24, 13)]
                .into_iter()
                .map(|(row, order)| SpacingSection {
                    order,
                    mapping: RateMapping::new(row as f64, row as f64 / 96.0, 0.0, 0.0, 1.0 / 96.0)
                        .unwrap(),
                    tempo: 120.0,
                    scroll: 1.0,
                })
                .collect(),
        )
        .unwrap();
        let misc = [
            (0, 0, TimingKind::Signature),
            (0, 1, TimingKind::Tempo),
            (18, 5, TimingKind::Label),
            (24, 13, TimingKind::Scroll),
        ]
        .map(|(row, phase, kind)| Entry {
            event: Event::Timing { row, kind },
            phase,
            chart_time: row as f64 / 96.0,
            widget: None,
        });
        let plan = visible_events::prepare(
            &grid,
            &clock,
            &index,
            &misc,
            Anchor {
                y: 200.0,
                time: 100.0 / 96.0,
                row: 100.0,
            },
            Settings {
                mode: SpacingMode::ConstantRow,
                reverse: false,
                speeds: Speeds::default(),
                interpolated_scroll: 1.0,
                zoom: 1.0,
            },
            Viewport {
                top: 0.0,
                bottom: 200.0,
                half_arrow: 32.0,
                hold_cap: 32.0,
            },
            case["max"].as_i64().unwrap() as i32,
            case["draw"].as_bool().unwrap(),
        )
        .unwrap();
        let got = plan
            .events()
            .iter()
            .map(|e| match e {
                Event::Timing {
                    kind: TimingKind::Signature,
                    ..
                } => 0,
                Event::Timing {
                    kind: TimingKind::Tempo,
                    ..
                } => 1,
                Event::Timing {
                    kind: TimingKind::Label,
                    ..
                } => 3,
                Event::Timing {
                    kind: TimingKind::Scroll,
                    ..
                } => 5,
                Event::Note(p) => match p.row {
                    12 => 2,
                    24 => 4,
                    36 => 6,
                    120 => 7,
                    _ => panic!(),
                },
                _ => panic!(),
            })
            .collect::<Vec<_>>();
        assert_eq!(serde_json::json!(got), case["admitted"], "{case}");
    }
}
#[test]
fn row_merge_is_lazy_phase_ordered_and_crossing_hold_precedes_initial_rate_quota_check() {
    use deadsync_editor::{
        document::{Document, Format},
        event_clipboard::{Event, TimingKind},
        generation::editor_timing::SpacingSection,
        history::Session,
        notes::{Grid, Kind as NoteKind, Note, Position},
        playback::Clock,
        rate_mapping::RateMapping,
        region_selection::{
            inventory::Entry,
            visible_events::{self, Viewport},
        },
        spacing::{Anchor, Speeds},
        spacing_index::Index,
        spacing_search::Settings,
        zoom::SpacingMode,
    };
    let mut grid = Grid::new(4, 1).unwrap();
    for row in [12, 24, 36, 120] {
        grid.set(
            Position {
                player: 0,
                lane: 0,
                row,
            },
            Some(Note::tap()),
        )
        .unwrap();
    }
    grid.set(
        Position {
            player: 0,
            lane: 1,
            row: 0,
        },
        Some(Note {
            kind: NoteKind::Hold,
            end: Some(96),
            attributes: "".into(),
            tail_attributes: "".into(),
        }),
    )
    .unwrap();
    assert_eq!(
        grid.row_heads_from(24)
            .take(2)
            .map(|(p, _)| p.row)
            .collect::<Vec<_>>(),
        vec![24, 36]
    );
    let session = Session::new(
        Document::parse(
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".to_vec(),
            Format::Ssc,
        )
        .unwrap(),
    );
    let clock = Clock::capture(&session, 0).unwrap();
    let index = Index::new(
        [(0, 0), (0, 1), (24, 13)]
            .into_iter()
            .map(|(row, order)| SpacingSection {
                order,
                mapping: RateMapping::new(row as f64, row as f64 / 96.0, 0.0, 0.0, 1.0 / 96.0)
                    .unwrap(),
                tempo: 120.0,
                scroll: 1.0,
            })
            .collect(),
    )
    .unwrap();
    let misc = [
        (0, 0, TimingKind::Signature),
        (0, 1, TimingKind::Tempo),
        (24, 13, TimingKind::Scroll),
    ]
    .map(|(row, phase, kind)| Entry {
        event: Event::Timing { row, kind },
        phase,
        chart_time: row as f64 / 96.0,
        widget: None,
    });
    let plan = visible_events::prepare(
        &grid,
        &clock,
        &index,
        &misc,
        Anchor {
            row: 150.0,
            time: 150.0 / 96.0,
            y: 200.0,
        },
        Settings {
            mode: SpacingMode::ConstantRow,
            reverse: false,
            speeds: Speeds::default(),
            interpolated_scroll: 1.0,
            zoom: 1.0,
        },
        Viewport {
            top: 0.0,
            bottom: 200.0,
            half_arrow: 32.0,
            hold_cap: 32.0,
        },
        0,
        true,
    )
    .unwrap();
    assert_eq!(
        plan.events(),
        &[
            Event::Note(Position {
                player: 0,
                lane: 1,
                row: 0
            }),
            Event::Timing {
                row: 24,
                kind: TimingKind::Scroll
            }
        ]
    );
    assert!(plan.stopped_by_quota);
}
#[test]
fn original_fumen_hold_discovery_and_forward_scan_count_same_predecessor_twice(){
    use deadsync_editor::{document::{Document,Format},history::Session,notes::{Grid,Note,Position,Kind as NoteKind},playback::Clock,region_selection::{visible_events::{self,Viewport},inventory::Entry},event_clipboard::{Event,TimingKind},spacing::{Anchor,Speeds},spacing_index::Index,spacing_search::Settings,generation::editor_timing::SpacingSection,rate_mapping::RateMapping,zoom::SpacingMode};
    let cases:Value=serde_json::from_str(include_str!("fixtures/key_commands/hold-admission-source.json")).unwrap();
    for case in cases.as_array().unwrap(){
        assert_eq!(case["same"],true);assert_eq!(case["pre"],serde_json::json!([0]));
        let mut grid=Grid::new(4,1).unwrap();let hold=Position{player:0,lane:0,row:0};let tap=Position{player:0,lane:1,row:96};
        grid.set(hold,Some(Note{kind:NoteKind::Hold,end:Some(192),attributes:"".into(),tail_attributes:"".into()})).unwrap();grid.set(tap,Some(Note::tap())).unwrap();
        let session=Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".to_vec(),Format::Ssc).unwrap());let clock=Clock::capture(&session,0).unwrap();
        let index=Index::new([0,1].into_iter().map(|order|SpacingSection{order,mapping:RateMapping::new(0.0,0.0,0.0,0.0,1.0/96.0).unwrap(),tempo:120.0,scroll:1.0}).collect()).unwrap();
        let misc=[(0,TimingKind::Signature),(1,TimingKind::Tempo)].map(|(phase,kind)|Entry{event:Event::Timing{row:0,kind},phase,chart_time:0.0,widget:None});
        let plan=visible_events::prepare(&grid,&clock,&index,&misc,Anchor{row:166.0,time:166.0/96.0,y:200.0},Settings{mode:SpacingMode::ConstantRow,reverse:false,speeds:Speeds::default(),interpolated_scroll:1.0,zoom:1.0},Viewport{top:0.0,bottom:200.0,half_arrow:32.0,hold_cap:32.0},case["max"].as_i64().unwrap()as i32,true).unwrap();
        let actual=plan.events().iter().map(|e|match e{Event::Note(p)if *p==hold=>0,Event::Note(p)if *p==tap=>1,_=>panic!("unexpected {e:?}")}).collect::<Vec<_>>();
        assert_eq!(serde_json::json!(actual),case["admitted"],"{case}");assert_eq!(plan.events().len(),case["count"].as_u64().unwrap()as usize);
        assert_eq!(plan.notes().filter(|p|*p==hold).count(),2);
    }
}
