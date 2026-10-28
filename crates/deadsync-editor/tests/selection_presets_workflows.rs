use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{
        Event, Selection, TimingKind,
        presets::{Mode, Preset},
    },
    history::Session,
    notes::{Kind, Position},
};
use std::collections::BTreeSet;
fn preset(name: &str) -> Preset {
    match name {
        "Notes" => Preset::Notes,
        "Taps" => Preset::Taps,
        "Mines" => Preset::Mines,
        "Fakes" => Preset::Fakes,
        "Lifts" => Preset::Lifts,
        "Holds" => Preset::Holds,
        "Rolls" => Preset::Rolls,
        "HoldsAndRolls" => Preset::HoldsAndRolls,
        "Miscellaneous" => Preset::Miscellaneous,
        "NotesAndMiscellaneous" => Preset::NotesAndMiscellaneous,
        "Patterns" => Preset::Patterns,
        _ => panic!("preset"),
    }
}
fn event(kind: &str, index: usize, player: u32) -> (Event, Option<Kind>) {
    let row = index as u64 * 48;
    let note = match kind {
        "Tap" => Some(Kind::Tap),
        "Mine" => Some(Kind::Mine),
        "Fake" => Some(Kind::Fake),
        "Lift" => Some(Kind::Lift),
        "Hold" => Some(Kind::Hold),
        "Roll" => Some(Kind::Roll),
        _ => None,
    };
    (
        match kind {
            "Tempo" => Event::Timing {
                row,
                kind: TimingKind::Tempo,
            },
            "Stop" => Event::Timing {
                row,
                kind: TimingKind::Stop,
            },
            "Pattern" => Event::Pattern { row },
            "Attack" => Event::Attack { row },
            _ => Event::Note(Position {
                row,
                lane: 0,
                player,
            }),
        },
        note,
    )
}
#[test]
fn original_54_additive_presets_match_including_players_and_patterns_as_misc() {
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/selection-presets.request.json"
    ))
    .unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/selection-presets.expected.json"
    ))
    .unwrap();
    let mut count = 0;
    for (case, oracle) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        let preset = preset(case["Preset"].as_str().unwrap());
        let player = case["Player"].as_u64().map(|p| p as u32);
        let mut selected = BTreeSet::new();
        let mut last = None;
        for item in case["Events"].as_array().unwrap() {
            if item["Selected"] == true {
                let id = item["Id"].as_str().unwrap();
                selected.insert(id);
                last = Some(id);
            }
        }
        for (index, item) in case["Events"].as_array().unwrap().iter().enumerate() {
            let id = item["Id"].as_str().unwrap();
            let (e, kind) = event(
                item["Kind"].as_str().unwrap(),
                index,
                item["Player"].as_u64().unwrap() as u32,
            );
            if preset.matches(e, kind, player) {
                selected.insert(id);
                last = Some(id);
            }
        }
        let actual: Vec<_> = selected.into_iter().collect();
        let expected: Vec<_> = oracle["Selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(actual, expected, "{}", case["Name"]);
        assert_eq!(last, oracle["Last"].as_str(), "{}", case["Name"]);
        count += 1;
    }
    assert_eq!(count, 54);
}
fn session() -> Session {
    Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0M00\n00F0\n000L;".as_slice(),Format::Ssc).unwrap())
}
fn chosen(selection: &Selection) -> BTreeSet<Event> {
    selection
        .events()
        .iter()
        .enumerate()
        .filter(|(i, _)| selection.is_selected(*i))
        .map(|(_, e)| *e)
        .collect()
}
#[test]
fn expansion_keeps_exact_choices_and_add_replace_remove_do_not_edit_song() {
    let mut s = session();
    let before = s.serialize().unwrap();
    let mut range = Selection::capture(&mut s, 0, 0, 0).unwrap();
    range
        .apply_preset(Preset::Notes, None, Mode::Replace)
        .unwrap();
    let previous = chosen(&range);
    assert_eq!(previous.len(), 1);
    let mut all = range.expand_to_chart(&mut s).unwrap();
    assert_eq!(chosen(&all), previous);
    let report = all.apply_preset(Preset::Mines, None, Mode::Add).unwrap();
    assert_eq!((report.matched, report.selected), (1, 2));
    let report = all
        .apply_preset(Preset::Fakes, None, Mode::Replace)
        .unwrap();
    assert_eq!((report.matched, report.selected), (1, 1));
    let report = all.apply_preset(Preset::Fakes, None, Mode::Remove).unwrap();
    assert_eq!((report.matched, report.selected), (1, 0));
    all.apply_preset(Preset::Notes, None, Mode::Add).unwrap();
    let before_choices = chosen(&all);
    assert!(
        all.apply_preset(Preset::Miscellaneous, Some(0), Mode::Replace)
            .is_err()
    );
    assert_eq!(chosen(&all), before_choices);
    assert_eq!(s.serialize().unwrap(), before);
    assert!(s.undo_label().is_none());
    assert!(!s.is_dirty());
}
#[test]
fn exact_preset_cut_leaves_other_kinds_and_clock_then_undo_restores_bytes() {
    let mut s = session();
    let before = s.serialize().unwrap();
    let mut selection = Selection::capture(&mut s, 0, 0, i32::MAX as u64).unwrap();
    selection
        .apply_preset(Preset::Mines, None, Mode::Replace)
        .unwrap();
    assert_eq!(selection.selected_count(), 1);
    selection.cut(&mut s).unwrap();
    assert!(
        s.grid(0)
            .unwrap()
            .get(Position {
                row: 48,
                lane: 1,
                player: 0
            })
            .is_none()
    );
    assert_eq!(s.grid(0).unwrap().iter().count(), 3);
    assert_eq!(s.document().text(None, "BPMS").as_deref(), Some("0=120"));
    let after = s.serialize().unwrap();
    s.undo().unwrap();
    assert_eq!(s.serialize().unwrap(), before);
    s.redo().unwrap();
    assert_eq!(s.serialize().unwrap(), after);
}
#[test]
fn stale_and_foreign_expansion_refuses_without_modifying_either_song() {
    let mut s = session();
    let selection = Selection::capture(&mut s, 0, 0, 0).unwrap();
    let mut foreign = session();
    let foreign_bytes = foreign.serialize().unwrap();
    assert!(selection.expand_to_chart(&mut foreign).is_err());
    assert_eq!(foreign.serialize().unwrap(), foreign_bytes);
    s.set_text(None, "TITLE", "changed").unwrap();
    let before = s.serialize().unwrap();
    assert!(selection.expand_to_chart(&mut s).is_err());
    assert_eq!(s.serialize().unwrap(), before);
}
