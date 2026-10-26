// Isolated model compilation before declaring a production module.
pub use deadsync_editor::{Error, chart_order, history, notes};
#[path = "../src/lane_entry.rs"]
mod lane_entry;
use deadsync_editor::{
    document::{Document, Format},
    history::Session,
    notes::{Kind, Note, Position},
};
use lane_entry::{Inputs, Press};
fn session(routine: bool) -> Session {
    let text = if routine {
        "#VERSION:0.83;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000000\n&\n00000000;"
    } else {
        "#VERSION:0.83;#BPMS:0=120;#VENDOR:keep;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;"
    };
    Session::new(Document::parse(text.as_bytes().to_vec(), Format::Ssc).unwrap())
}
fn pos(player: u32, lane: u16, row: u64) -> Position {
    Position { player, lane, row }
}
fn down(lane: u16, player: u32, row: f64) -> Press {
    Press {
        lane,
        player,
        row,
        snap_rows: 0,
        modified: false,
        playing: false,
        advance_by_snap: false,
    }
}
fn hold(end: u64) -> Note {
    Note {
        kind: Kind::Hold,
        end: Some(end),
        attributes: "".into(),
        tail_attributes: "".into(),
    }
}
#[test]
fn tap_is_a_detached_preview_then_one_atomic_undoable_release() {
    let mut s = session(false);
    let before = s.serialize().unwrap();
    let revision = s.revision();
    let mut input = Inputs::capture(&mut s, 0).unwrap();
    input.press(&mut s, down(0, 0, 0.0)).unwrap();
    assert!(input.is_held(0));
    assert_eq!(input.drafts().next().unwrap().kind, Kind::Tap);
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.revision(), revision);
    assert!(input.release(&mut s, 0).unwrap());
    assert_eq!(
        s.grid(0).unwrap().get(pos(0, 0, 0)).unwrap().kind,
        Kind::Tap
    );
    assert!(!input.is_held(0));
    assert!(!input.release(&mut s, 0).unwrap());
    assert!(s.undo().unwrap());
    assert_eq!(s.serialize().unwrap(), before);
    assert!(s.redo().unwrap());
}
#[test]
fn held_movement_creates_forward_or_backward_hold_and_can_return_to_tap() {
    for end in [0.0, 96.0] {
        let mut s = session(false);
        let mut i = Inputs::capture(&mut s, 0).unwrap();
        i.press(&mut s, down(1, 0, 48.0)).unwrap();
        i.move_to(&s, end, 0, false).unwrap();
        let d = i.drafts().next().unwrap();
        assert_eq!(d.position.row, 48.min(end as u64));
        assert_eq!(d.end, Some(48.max(end as u64)));
        assert_eq!(d.kind, Kind::Hold);
        i.move_to(&s, 48.0, 0, false).unwrap();
        assert_eq!(i.drafts().next().unwrap().kind, Kind::Tap);
        i.move_to(&s, end, 0, false).unwrap();
        i.release(&mut s, 1).unwrap();
        assert_eq!(
            s.grid(0)
                .unwrap()
                .get(pos(0, 1, d.position.row))
                .unwrap()
                .end,
            d.end
        );
    }
}
#[test]
fn modification_changes_pending_tap_mine_hold_roll_without_history_noise() {
    let mut s = session(false);
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    i.press(&mut s, down(0, 0, 0.0)).unwrap();
    i.modifier(&s, true).unwrap();
    assert_eq!(i.drafts().next().unwrap().kind, Kind::Mine);
    i.move_to(&s, 48.0, 0, true).unwrap();
    assert_eq!(i.drafts().next().unwrap().kind, Kind::Roll);
    i.modifier(&s, false).unwrap();
    assert_eq!(i.drafts().next().unwrap().kind, Kind::Hold);
    i.release(&mut s, 0).unwrap();
    assert_eq!(s.revision(), 1);
    assert!(s.undo().unwrap());
    assert_eq!(s.grid(0).unwrap().iter().count(), 0);
}
#[test]
fn cancellation_and_focus_loss_leave_original_note_and_history_untouched() {
    let mut s = session(false);
    s.edit_notes(0, "seed", [(pos(0, 0, 48), Some(Note::tap()))])
        .unwrap();
    let before = s.serialize().unwrap();
    let revision = s.revision();
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    i.press(&mut s, down(0, 0, 48.0)).unwrap();
    assert_eq!(i.drafts().count(), 0);
    assert_eq!(i.deleted_heads().collect::<Vec<_>>(), vec![pos(0, 0, 48)]);
    assert_eq!(i.chart(), 0);
    assert!(i.is_current(&s));
    i.move_to(&s, 96.0, 0, false).unwrap();
    assert_eq!(i.cancel(), 1);
    assert_eq!(i.cancel(), 0);
    assert!(!i.release(&mut s, 0).unwrap());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(s.revision(), revision);
}
#[test]
fn negative_leadin_is_ignored_and_source_rounding_and_snap_ties_are_distinct() {
    assert_eq!(lane_entry::nearest(12.5).unwrap(), 12);
    assert_eq!(lane_entry::nearest(12.50001).unwrap(), 13);
    assert_eq!(lane_entry::snap(12, 24).unwrap(), 24);
    let mut s = session(false);
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    i.press(&mut s, down(0, 0, -0.1)).unwrap();
    assert!(!i.is_held(0));
    i.press(
        &mut s,
        Press {
            snap_rows: 24,
            ..down(0, 0, 12.5)
        },
    )
    .unwrap();
    assert_eq!(i.drafts().next().unwrap().position.row, 24);
    i.move_to(&s, 25.5, 0, false).unwrap();
    assert_eq!(i.drafts().next().unwrap().end, Some(25));
    assert!(i.move_to(&s, f64::NAN, 0, false).is_err());
    assert!(lane_entry::snap(0, u64::MAX).is_err());
}
#[test]
fn playing_and_advance_modes_commit_taps_immediately_but_deletion_never_advances() {
    let mut s = session(false);
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    let r = i
        .press(
            &mut s,
            Press {
                playing: true,
                snap_rows: 48,
                advance_by_snap: true,
                ..down(0, 0, 24.0)
            },
        )
        .unwrap();
    assert!(r.changed);
    assert_eq!(r.advance_to, None);
    assert!(!i.is_held(0));
    assert_eq!(s.grid(0).unwrap().get(pos(0, 0, 48)).unwrap().end, None);
    let r = i
        .press(
            &mut s,
            Press {
                snap_rows: 48,
                advance_by_snap: true,
                ..down(1, 0, 48.0)
            },
        )
        .unwrap();
    assert_eq!(r.advance_to, Some(96));
    assert!(r.changed);
    let r = i
        .press(
            &mut s,
            Press {
                snap_rows: 48,
                advance_by_snap: true,
                ..down(1, 0, 48.0)
            },
        )
        .unwrap();
    assert_eq!(r.advance_to, None);
    assert!(i.is_held(1));
    assert!(i.release(&mut s, 1).unwrap());
    assert!(s.grid(0).unwrap().get(pos(0, 1, 48)).is_none());
}
#[test]
fn participants_are_captured_per_press_and_chords_have_one_history_entry_per_lane() {
    let mut s = session(true);
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    i.press(&mut s, down(0, 0, 0.0)).unwrap();
    i.press(&mut s, down(4, 1, 0.0)).unwrap();
    i.move_to(&s, 48.0, 1, false).unwrap();
    i.release(&mut s, 0).unwrap();
    i.release(&mut s, 4).unwrap();
    assert!(s.grid(0).unwrap().get(pos(0, 0, 0)).is_some());
    assert!(s.grid(0).unwrap().get(pos(1, 4, 0)).is_some());
    assert!(s.undo().unwrap());
    assert!(s.grid(0).unwrap().get(pos(0, 0, 0)).is_some());
    assert!(s.grid(0).unwrap().get(pos(1, 4, 0)).is_none());
}
#[test]
fn unrelated_content_mutation_invalidates_pending_release_without_partial_commit() {
    let mut s = session(false);
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    i.press(&mut s, down(0, 0, 0.0)).unwrap();
    s.edit_notes(0, "external", [(pos(0, 2, 48), Some(Note::tap()))])
        .unwrap();
    let before = s.serialize().unwrap();
    assert!(!i.is_current(&s));
    assert!(i.release(&mut s, 0).is_err());
    assert_eq!(s.serialize().unwrap(), before);
    assert_eq!(i.cancel(), 1);
}
#[test]
fn tap_inside_hold_shortens_by_source_sixteenth_and_undo_restores_whole_operation() {
    let mut s = session(false);
    s.edit_notes(0, "seed", [(pos(0, 0, 0), Some(hold(96)))])
        .unwrap();
    let before = s.serialize().unwrap();
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    i.press(&mut s, down(0, 0, 48.0)).unwrap();
    i.release(&mut s, 0).unwrap();
    assert_eq!(s.grid(0).unwrap().get(pos(0, 0, 0)).unwrap().end, Some(36));
    assert_eq!(
        s.grid(0).unwrap().get(pos(0, 0, 48)).unwrap().kind,
        Kind::Tap
    );
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn new_hold_merges_edges_and_removes_inner_notes_across_participants_atomically() {
    let mut s = session(true);
    s.edit_notes(
        0,
        "seed",
        [
            (pos(0, 0, 0), Some(hold(48))),
            (pos(1, 0, 40), Some(Note::tap())),
            (pos(1, 0, 60), Some(hold(96))),
        ],
    )
    .unwrap();
    let before = s.serialize().unwrap();
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    i.press(&mut s, down(0, 0, 24.0)).unwrap();
    i.move_to(&s, 72.0, 0, false).unwrap();
    i.release(&mut s, 0).unwrap();
    assert_eq!(s.grid(0).unwrap().iter().count(), 1);
    assert_eq!(s.grid(0).unwrap().get(pos(0, 0, 0)).unwrap().end, Some(96));
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
}
#[test]
fn contained_hold_is_noop_unless_type_changes_then_owner_follows_source() {
    let mut s = session(true);
    s.edit_notes(0, "seed", [(pos(0, 0, 0), Some(hold(96)))])
        .unwrap();
    let mut i = Inputs::capture(&mut s, 0).unwrap();
    i.press(&mut s, down(0, 1, 24.0)).unwrap();
    i.move_to(&s, 48.0, 1, false).unwrap();
    assert!(!i.release(&mut s, 0).unwrap());
    i.press(&mut s, down(0, 1, 24.0)).unwrap();
    i.move_to(&s, 48.0, 1, true).unwrap();
    assert!(i.release(&mut s, 0).unwrap());
    assert!(s.grid(0).unwrap().get(pos(0, 0, 0)).is_none());
    assert_eq!(
        s.grid(0).unwrap().get(pos(1, 0, 0)).unwrap().kind,
        Kind::Roll
    );
}
#[test]
fn original_lane_down_and_movement_oracle_matches_detached_control_flow() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/lane-entry.reference.json"
    ))
    .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let mut s = session(true);
        if let Some(row) = case["initial"].as_u64() {
            s.edit_notes(0, "seed", [(pos(0, 0, row), Some(Note::tap()))])
                .unwrap();
        }
        let mut input = Inputs::capture(&mut s, 0).unwrap();
        let mut position = 0.0;
        let mut player = 0;
        let mut commits = 0;
        for (step, expected) in case["steps"]
            .as_array()
            .unwrap()
            .iter()
            .zip(case["expected"].as_array().unwrap())
        {
            if let Some(row) = step["row"].as_f64() {
                position = row;
            }
            if let Some(p) = step["player"].as_u64() {
                player = p as u32;
            }
            match step["kind"].as_str().unwrap() {
                "down" => {
                    let result = input
                        .press(
                            &mut s,
                            Press {
                                snap_rows: step["snap"].as_u64().unwrap(),
                                playing: step["playing"].as_bool().unwrap(),
                                advance_by_snap: step["advance"].as_bool().unwrap(),
                                ..down(0, player, position)
                            },
                        )
                        .unwrap();
                    if result.changed {
                        commits += 1;
                    }
                    if let Some(row) = result.advance_to {
                        position = row as f64;
                    }
                }
                "move" => input.move_to(&s, position, player, false).unwrap(),
                "up" => {
                    let was_held = input.is_held(0);
                    input.release(&mut s, 0).unwrap();
                    commits += usize::from(was_held);
                }
                "cancel" => {
                    input.cancel();
                }
                _ => unreachable!(),
            }
            assert_eq!(
                input.is_held(0),
                expected["active"].as_bool().unwrap(),
                "{} {step}",
                case["name"]
            );
            assert_eq!(commits, expected["commits"].as_u64().unwrap() as usize);
            assert_eq!(position, expected["position"].as_f64().unwrap());
            let actual = input.drafts().next();
            assert_eq!(actual.is_some(), !expected["draft"].is_null());
            if let Some(actual) = actual {
                let e = &expected["draft"];
                assert_eq!(actual.position.row, e["row"].as_u64().unwrap());
                assert_eq!(actual.position.player, e["player"].as_u64().unwrap() as u32);
                assert_eq!(actual.end, e["end"].as_u64());
                assert_eq!(
                    match actual.kind {
                        Kind::Hold => "Hold",
                        Kind::Mine => "Mine",
                        Kind::Tap => "Tap",
                        _ => panic!("oracle draft kind"),
                    },
                    e["kind"].as_str().unwrap()
                );
            }
        }
    }
}
#[test]
fn source_generic_lane_note_sorts_after_hold_and_tap_before_participant_tiebreak() {
    let mut s = session(true);
    let mut mine = Note::tap();
    mine.kind = Kind::Mine;
    s.edit_notes(
        0,
        "seed",
        [(pos(1, 0, 0), Some(hold(96))), (pos(0, 0, 0), Some(mine))],
    )
    .unwrap();
    let mut input = Inputs::capture(&mut s, 0).unwrap();
    input.press(&mut s, down(0, 1, 0.0)).unwrap();
    assert_eq!(
        input.deleted_heads().collect::<Vec<_>>(),
        vec![pos(0, 0, 0)]
    );
    input.release(&mut s, 0).unwrap();
    assert!(s.grid(0).unwrap().get(pos(0, 0, 0)).is_none());
    assert_eq!(s.grid(0).unwrap().get(pos(1, 0, 0)).unwrap().end, Some(96));
}
