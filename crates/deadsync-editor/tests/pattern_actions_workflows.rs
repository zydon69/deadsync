use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{Event, Selection},
    generation::metadata::{ChartData, Definition, PATTERN_BALANCED, SIXTEENTHS},
    history::Session,
    notes::{Grid, Kind, Note, Position},
    pattern_actions::{Action, Plan, Refusal, Worker},
};
use std::{
    collections::BTreeMap,
    sync::{Arc, atomic::AtomicBool},
};
fn pos(row: u64, lane: u16) -> Position {
    Position {
        row,
        lane,
        player: 0,
    }
}
fn source() -> Session {
    let mut grid = Grid::new(4, 1).unwrap();
    for row in [0, 192, 384] {
        grid.set(pos(row, 0), Some(Note::tap())).unwrap();
    }
    grid.set(
        pos(360, 2),
        Some(Note {
            kind: Kind::Hold,
            end: Some(480),
            ..Note::tap()
        }),
    )
    .unwrap();
    let mut bytes=b"#VERSION:0.83;#BPMS:0=120;#TITLE:Patterns;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:1;#NOTES:".to_vec();
    bytes.extend(grid.serialize().unwrap());
    bytes.push(b';');
    let mut s = Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
    let patterns = [(0, 10), (192, 20), (384, 30)]
        .into_iter()
        .map(|(row, seed)| {
            (
                row,
                Definition {
                    length: 96,
                    random_seed: seed,
                    start_position_inclusive: true,
                    pattern_config_guid: SIXTEENTHS.parse().unwrap(),
                    performed_chart_config_guid: PATTERN_BALANCED.parse().unwrap(),
                    extra: BTreeMap::from([("Future".into(), serde_json::json!("keep"))]),
                    ..Definition::default()
                },
            )
        })
        .collect();
    ChartData {
        patterns: Some(patterns),
        ..ChartData::default()
    }
    .save(&mut s, 0)
    .unwrap();
    s
}
fn selected(s: &mut Session) -> Selection {
    let mut selection = Selection::capture(s, 0, 0, i32::MAX as u64).unwrap();
    selection.select_all(false);
    for i in 0..selection.events().len() {
        let pick = matches!(
            selection.events()[i],
            Event::Pattern { row: 0 | 384 } | Event::Note(_)
        );
        selection.set_selected(i, pick).unwrap();
    }
    selection
}
fn seeds(s: &Session) -> BTreeMap<u64, i32> {
    ChartData::read(s.document(), 0)
        .unwrap()
        .unwrap()
        .patterns
        .unwrap()
        .into_iter()
        .map(|(row, p)| (row, p.random_seed))
        .collect()
}
#[test]
fn exact_checked_patterns_exclude_intermediate_unchecked_marker_and_notes() {
    let mut s = source();
    let selection = selected(&mut s);
    let before = s.serialize().unwrap();
    let p = Plan::checked(&s, 0, &selection, Action::GenerateFixed).unwrap();
    assert_eq!(p.review().rows, vec![0, 384]);
    assert!(p.review().new_seeds.is_empty());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(
        Plan::all(&s, 0, Action::Clear).unwrap().review().rows,
        vec![0, 192, 384]
    );
}
#[test]
fn source_new_seed_history_is_separate_and_cancelled_worker_keeps_seed_commit() {
    let mut s = source();
    let before = s.serialize().unwrap();
    let p = Plan::rows_with_seeds(&s, 0, [0, 384], vec![101, 303]).unwrap();
    let review = p.review().clone();
    let started = p.begin(&mut s, 0, &review, None).unwrap();
    assert!(started.seeds_committed);
    assert_eq!(seeds(&s), BTreeMap::from([(0, 101), (192, 20), (384, 303)]));
    let reseeded = s.serialize().unwrap();
    let Worker::Generate(task) = started.worker else {
        panic!("generate")
    };
    assert!(task.run(&AtomicBool::new(true), |_| {}).is_err());
    assert_eq!(s.serialize().unwrap(), reseeded);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), reseeded);
}
#[test]
fn generated_notes_undo_then_seed_undo_restore_two_distinct_history_actions() {
    let mut s = source();
    let before = s.serialize().unwrap();
    let p = Plan::rows_with_seeds(&s, 0, [0, 384], vec![101, 303]).unwrap();
    let review = p.review().clone();
    let started = p.begin(&mut s, 0, &review, None).unwrap();
    let reseeded = s.serialize().unwrap();
    let Worker::Generate(task) = started.worker else {
        panic!("generate")
    };
    let preview = task.run(&AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(preview.report().failed, 0);
    assert!(preview.apply(&mut s, 0).unwrap());
    let after = s.serialize().unwrap();
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), reseeded);
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn clear_exact_set_preserves_unchecked_notes_and_marker_metadata() {
    let mut s = source();
    let before = s.serialize().unwrap();
    let metadata = ChartData::read(s.document(), 0).unwrap().unwrap();
    let selection = selected(&mut s);
    let p = Plan::checked(&s, 0, &selection, Action::Clear).unwrap();
    let review = p.review().clone();
    let started = p.begin(&mut s, 0, &review, None).unwrap();
    assert!(!started.seeds_committed);
    let Worker::Clear(task) = started.worker else {
        panic!("clear")
    };
    let preview = task.run(&AtomicBool::new(false), |_, _| {}).unwrap();
    preview.apply(&mut s, 0).unwrap();
    assert!(s.grid(0).unwrap().get(pos(0, 0)).is_none());
    assert!(s.grid(0).unwrap().get(pos(384, 0)).is_none());
    assert!(s.grid(0).unwrap().get(pos(192, 0)).is_some());
    assert_eq!(s.grid(0).unwrap().get(pos(360, 2)).unwrap().end, Some(372));
    assert_eq!(
        serde_json::to_value(ChartData::read(s.document(), 0).unwrap()).unwrap(),
        serde_json::to_value(Some(metadata)).unwrap()
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn stale_foreign_empty_and_inexact_review_refuse_without_reseed() {
    let mut s = source();
    let mut selection = selected(&mut s);
    selection.select_all(false);
    assert_eq!(
        Plan::checked(&s, 0, &selection, Action::Clear)
            .err()
            .unwrap()
            .reason,
        Refusal::Empty
    );
    let p = Plan::all(&s, 0, Action::GenerateNewSeeds).unwrap();
    let mut review = p.review().clone();
    review.rows.pop();
    let before = s.serialize().unwrap();
    assert_eq!(
        p.begin(&mut s, 0, &review, None).err().unwrap().reason,
        Refusal::MissingReview
    );
    assert_eq!(s.serialize().unwrap(), before);
    let p = Plan::all(&s, 0, Action::GenerateNewSeeds).unwrap();
    assert!(!p.is_current(&source(), 0));
    let review = p.review().clone();
    s.set_text(None, "TITLE", "new").unwrap();
    s.undo().unwrap();
    assert_eq!(
        p.begin(&mut s, 0, &review, None).err().unwrap().reason,
        Refusal::Stale
    );
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn actual_source_random_witness_seeds_preserve_selected_and_all_mapping() {
    let cases: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/pattern-actions.expected.json")).unwrap();
    for case in cases.as_array().unwrap().iter().filter(|c| {
        c["Action"].as_str().unwrap().ends_with("New")
            && !c["Action"].as_str().unwrap().starts_with("Empty")
    }) {
        let mut s = source();
        let all = case["Action"] == "AllNew";
        let expected: BTreeMap<_, _> = case["After"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                (
                    v["Row"].as_u64().unwrap(),
                    v["RandomSeed"].as_i64().unwrap() as i32,
                )
            })
            .collect();
        let rows = if all { vec![0, 192, 384] } else { vec![0, 384] };
        let p = Plan::rows_with_seeds(
            &s,
            0,
            rows.iter().copied(),
            rows.iter().map(|r| expected[r]).collect(),
        )
        .unwrap();
        let review = p.review().clone();
        let started = p.begin(&mut s, 0, &review, None).unwrap();
        drop(started);
        assert_eq!(seeds(&s), expected);
        assert_eq!(case["Undo1"], "ActionAutoGeneratePatterns");
        assert_eq!(case["Undo2"], "ActionMultiple");
        s.undo().unwrap();
        assert_eq!(seeds(&s), BTreeMap::from([(0, 10), (192, 20), (384, 30)]));
    }
}
