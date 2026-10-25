use deadsync_editor::{
    document::{Document, Format},
    generation::{context::ContextEvent, pattern_region},
    history::Session,
    notes::{Grid, Kind, Note, Position},
};
use std::sync::{Arc, atomic::AtomicBool};
fn pos(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
fn sustain(kind: Kind, end: u64) -> Note {
    Note {
        kind,
        end: Some(end),
        attributes: "[7]".into(),
        tail_attributes: "[9]".into(),
    }
}
#[test]
fn region_deletion_shortens_crossing_holds_removes_full_heads_and_is_one_reversible_gesture() {
    let mut grid = Grid::new(8, 2).unwrap();
    for (position, note) in [
        (pos(0, 0, 0), sustain(Kind::Hold, 192)),  // trim to 84
        (pos(0, 1, 90), sustain(Kind::Roll, 192)), // spacing would precede head: trim to 95
        (pos(0, 2, 95), sustain(Kind::Hold, 192)), // no room for sustain: delete
        (pos(0, 3, 96), sustain(Kind::Hold, 240)), // head inside: remove entire hold
        (
            pos(0, 4, 120),
            Note {
                kind: Kind::Mine,
                ..Note::tap()
            },
        ),
        (pos(0, 5, 168), Note::tap()), // after selection: keep
        (pos(1, 0, 0), sustain(Kind::Roll, 192)), // other owner: keep
    ] {
        grid.set(position, Some(note)).unwrap();
    }
    let body = String::from_utf8(grid.serialize().unwrap()).unwrap();
    let source = format!(
        "#TITLE:Keep;#NOTEDATA:;#STEPSTYPE:dance-routine;#METER:9;#VENDOR:keep;#NOTES:{body};"
    )
    .into_bytes();
    let mut session =
        Session::new(Document::parse(Arc::<[u8]>::from(source.clone()), Format::Ssc).unwrap());
    let (mut edits, report) = pattern_region::deletion_edits(
        session.grid(0).unwrap(),
        0,
        96,
        144,
        12,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(report.shortened_holds, 2);
    assert_eq!(report.removed, 3);
    edits.push((pos(0, 3, 96), Some(Note::tap())));
    session.edit_notes(0, "Generate pattern", edits).unwrap();
    let after = session.grid(0).unwrap().clone();
    assert_eq!(after.get(pos(0, 0, 0)).unwrap().end, Some(84));
    assert_eq!(after.get(pos(0, 1, 90)).unwrap().end, Some(95));
    assert_eq!(
        after.get(pos(0, 1, 90)).unwrap().tail_attributes.as_ref(),
        "[9]"
    );
    assert!(after.get(pos(0, 2, 95)).is_none());
    assert_eq!(after.get(pos(0, 3, 96)).unwrap().kind, Kind::Tap);
    assert_eq!(after.get(pos(1, 0, 0)).unwrap().end, Some(192));
    let saved = session.serialize().unwrap();
    let mut reopened =
        Session::new(Document::parse(Arc::<[u8]>::from(saved), Format::Ssc).unwrap());
    assert_eq!(reopened.grid(0).unwrap(), &after);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), source);
    assert!(session.undo_label().is_none());
    session.redo().unwrap();
    assert_eq!(session.grid(0).unwrap(), &after);
}
#[test]
fn context_extraction_retains_whole_sustains_and_original_overlap_order() {
    let mut grid = Grid::new(4, 1).unwrap();
    grid.set(pos(0, 0, 24), Some(sustain(Kind::Hold, 180)))
        .unwrap();
    grid.set(pos(0, 3, 0), Some(sustain(Kind::Roll, 180)))
        .unwrap();
    grid.set(pos(0, 1, 96), Some(Note::tap())).unwrap();
    grid.set(
        pos(0, 2, 108),
        Some(Note {
            kind: Kind::Mine,
            ..Note::tap()
        }),
    )
    .unwrap();
    let (selected, events) =
        pattern_region::extract(&grid, 0, 48, 108, &AtomicBool::new(false)).unwrap();
    assert_eq!(selected.get(pos(0, 0, 24)).unwrap().end, Some(180));
    assert_eq!(
        events.iter().map(|event| event.row).collect::<Vec<_>>(),
        vec![24, 0, 96, 108]
    );
    assert!(events[0].is_step);
    assert!(!events[3].is_step);
    assert!(pattern_region::extract(&grid, 1, 0, 100, &AtomicBool::new(false)).is_err());
    assert!(pattern_region::extract(&grid, 0, 0, 100, &AtomicBool::new(true)).is_err());
}
#[test]
fn context_window_counts_step_heads_and_includes_entire_boundary_rows() {
    let mut events = Vec::new();
    for row in 0..120 {
        for lane in 0..2 {
            events.push(ContextEvent {
                row: row * 12,
                lane: Some(lane),
                is_step: true,
            });
        }
    }
    assert_eq!(
        pattern_region::bounds(&events, 600, 720, 0, &AtomicBool::new(false)).unwrap(),
        (408, 912)
    );
    assert_eq!(
        pattern_region::bounds(&events, 600, 720, 64, &AtomicBool::new(false)).unwrap(),
        (216, 912)
    );
    events.push(ContextEvent {
        row: 2400,
        lane: None,
        is_step: false,
    });
    assert_eq!(
        pattern_region::bounds(&events, 1416, 1440, 0, &AtomicBool::new(false))
            .unwrap()
            .1,
        2400
    );
    assert!(pattern_region::bounds(&events, 600, 720, 0, &AtomicBool::new(true)).is_err());
    events.reverse();
    assert!(pattern_region::bounds(&events, 600, 720, 0, &AtomicBool::new(false)).is_err());
}

#[test]
fn generated_pattern_replaces_crossing_holds_in_one_apply_undo_save_workflow() {
    use deadsync_editor::generation::{
        config::{PatternConfig, PerformedConfig},
        pattern::{self, PatternRequest},
        search::RootContext,
        timing::{Event, EventKind, GenerationTiming},
    };
    use deadsync_editor::storage::FileSession;
    let mut grid = Grid::new(4, 1).unwrap();
    grid.set(pos(0, 0, 0), Some(sustain(Kind::Hold, 192)))
        .unwrap();
    grid.set(pos(0, 1, 95), Some(sustain(Kind::Roll, 192)))
        .unwrap();
    let body = String::from_utf8(grid.serialize().unwrap()).unwrap();
    let source=format!("#VERSION:0.83;#TITLE:Keep;#BPMS:0=150;#NOTEDATA:;#STEPSTYPE:dance-single;#METER:9;#VENDOR:keep;#NOTES:{body};").into_bytes();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pattern.ssc");
    std::fs::write(&path, &source).unwrap();
    let mut file = FileSession::open(&path).unwrap();
    let preview = pattern::generate(
        &PatternRequest {
            layout: "dance-single".into(),
            start: 96,
            end: 144,
            seed: 42,
            previous_foot: 1,
            previous_footing: [0, 3],
            following_footing: [-1, -1],
            context: RootContext {
                lane_counts: vec![0; 4],
                previous_times: [0.0; 2],
                total_steps: 0,
                last_transition: None,
                previous_same_counts: [0; 2],
            },
            config: PerformedConfig::balanced(),
            pattern: PatternConfig::sixteenths(),
            average_nps: 8.0,
        },
        &GenerationTiming::new(vec![Event {
            row: 0,
            kind: EventKind::Tempo(150.0),
        }])
        .unwrap(),
        &AtomicBool::new(false),
        |_, _| {},
    )
    .unwrap();
    preview.apply(&mut file.session, 0, 0, 0).unwrap();
    let after = file.session.grid(0).unwrap().clone();
    assert_eq!(after.get(pos(0, 0, 0)).unwrap().end, Some(84));
    assert!(after.get(pos(0, 1, 95)).is_none());
    assert_eq!(
        after
            .iter()
            .filter(|(position, _)| position.row >= 96 && position.row <= 144)
            .count(),
        5
    );
    assert_eq!(file.session.undo_label(), Some("Generate pattern"));
    file.session.undo().unwrap();
    assert_eq!(file.session.serialize().unwrap(), source);
    assert!(file.session.undo_label().is_none());
    file.session.redo().unwrap();
    file.save().unwrap();
    let mut reopened = FileSession::open(&path).unwrap();
    assert_eq!(reopened.session.grid(0).unwrap(), &after);
}
