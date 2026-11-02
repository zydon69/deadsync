use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{
        Event, Selection,
        presets::{Mode, Preset},
    },
    history::Session,
};

fn fixture() -> (Session, Selection) {
    let mut session = Session::new(
        Document::parse(
            b"#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1111;".as_slice(),
            Format::Ssc,
        )
        .unwrap(),
    );
    let mut selection = Selection::capture(&mut session, 0, 0, 48).unwrap();
    selection.select_all(false);
    (session, selection)
}
fn lane(event: Event) -> u64 {
    match event {
        Event::Note(position) => u64::from(position.lane),
        _ => panic!("unexpected selected timing"),
    }
}

#[test]
fn anchor_and_hashset_order_match_sixteen_original_selection_operations() {
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/selection-anchor.expected.json"
    ))
    .unwrap();
    let (mut session, mut selection) = fixture();
    let before = session.serialize().unwrap();
    let steps = oracle["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 16);
    for step in steps {
        let op = step["op"].as_str().unwrap();
        if op == "Clear" {
            selection.select_all(false);
        } else {
            let target: u64 = op[1..].parse().unwrap();
            let index = selection.events().iter().position(|event|
                matches!(event, Event::Note(position) if u64::from(position.lane) == target)
            ).unwrap();
            selection.set_selected(index, op.starts_with('S')).unwrap();
        }
        let selected: Vec<_> = selection.selected_events().map(lane).collect();
        let expected: Vec<_> = step["selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_u64().unwrap())
            .collect();
        assert_eq!(selected, expected, "{op}");
        assert_eq!(
            selection.last_selected_event().map(lane),
            step["last"].as_u64(),
            "{op}"
        );
        // Expanding available choices must not infer a new anchor from slot order.
        selection = selection.expand_to_chart(&mut session).unwrap();
        assert_eq!(
            selection.last_selected_event().map(lane),
            step["last"].as_u64(),
            "expanded {op}"
        );
    }
    assert_eq!(session.serialize().unwrap(), before);
    assert!(!session.is_dirty());
}

#[test]
fn additive_presets_keep_or_move_the_explicit_anchor_and_remove_clears_it() {
    let (_session, mut selection) = fixture();
    selection
        .apply_preset(Preset::Taps, None, Mode::Add)
        .unwrap();
    assert_eq!(selection.last_selected_event().map(lane), Some(3));
    selection
        .apply_preset(Preset::Holds, None, Mode::Add)
        .unwrap();
    assert_eq!(selection.last_selected_event().map(lane), Some(3));
    let index = selection
        .events()
        .iter()
        .position(|event| matches!(event,Event::Note(p) if p.lane==0))
        .unwrap();
    selection.set_selected(index, true).unwrap();
    assert_eq!(selection.last_selected_event().map(lane), Some(0));
    selection
        .apply_preset(Preset::Taps, None, Mode::Remove)
        .unwrap();
    assert_eq!(selection.last_selected_event(), None);
    assert_eq!(selection.selected_count(), 0);
}
