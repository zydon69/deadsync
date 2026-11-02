use deadsync_editor::{
    document::{Document, Format},
    event_clipboard::{Event, region::Modifiers},
    history::Session,
    options_preferences::RenderPolicy,
    region_selection::{
        geometry::{self, Axis, Range},
        inventory::Inventory,
        misc_layout::Rect,
    },
    selection_preferences::{Overlap, RegionMode},
    spacing::Speeds,
    spacing_search::Settings,
    zoom::SpacingMode,
};
fn fixture() -> Session {
    let rows = (0..16)
        .map(|i| {
            let mut row = [b'0'; 4];
            if [0, 1, 2, 4, 8].contains(&i) {
                row[i % 4] = b'1';
            }
            String::from_utf8(row.to_vec()).unwrap()
        })
        .collect::<Vec<_>>()
        .join("\n");
    Session::new(Document::parse(format!("#VERSION:0.83;#BPMS:0=120;#SAMPLESTART:0.139;#SAMPLELENGTH:0;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:{rows};").into_bytes(),Format::Ssc).unwrap())
}
#[test]
fn actual_original_finish_region_modes_visibility_and_modifier_precedence_match() {
    let cases: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/key_commands/region-finish-source.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let mut session = fixture();
        let inventory = Inventory::capture(&mut session, 0).unwrap();
        let mut selection = inventory.choices();
        let initial = selection
            .events()
            .iter()
            .position(|e| matches!(e,Event::Note(p)if p.row==0))
            .unwrap();
        selection.set_selected(initial, true).unwrap();
        let mode = Overlap::from_value(case["mode"].as_i64().unwrap() as i32);
        let spacing = match case["spacing"].as_u64().unwrap() {
            0 => SpacingMode::ConstantTime,
            1 => SpacingMode::ConstantRow,
            _ => SpacingMode::Variable,
        };
        let modifier = case["modifier"].as_str().unwrap();
        let mods = Modifiers {
            alt: modifier.contains("alt"),
            control: modifier.contains("ctrl"),
            shift: modifier.contains("shift"),
        };
        let hidden = case["hidden"].as_str().unwrap();
        let x = if mods.alt {
            Range::normalized(-92.0, -2.0).unwrap()
        } else {
            Range::normalized(0.0, 256.0).unwrap()
        };
        let lanes = geometry::selected_lanes(x, 64.0, 64.0, mode).unwrap();
        let axis = if spacing == SpacingMode::ConstantTime {
            Axis::Time
        } else {
            Axis::Position
        };
        let bounds = Range::normalized(
            0.0,
            if spacing == SpacingMode::ConstantTime {
                0.5
            } else {
                48.0
            },
        )
        .unwrap();
        let bounds = geometry::adjusted_range(
            inventory.spacing_index(),
            Settings {
                mode: spacing,
                reverse: false,
                speeds: Speeds::default(),
                interpolated_scroll: 1.0,
                zoom: 1.0,
            },
            axis,
            bounds,
            32.0,
            mode,
        )
        .unwrap();
        let preview = selection
            .events()
            .iter()
            .find(|e| matches!(e, Event::SongWidget(_)))
            .copied()
            .unwrap();
        let misc = [(
            preview,
            Rect {
                x: -190.0,
                y: 30.0,
                width: 40.0,
                height: 20.0,
            },
        )];
        let screen = Rect {
            x: if mods.alt { -220.0 } else { -128.0 },
            y: 0.0,
            width: if mods.alt { 90.0 } else { 256.0 },
            height: if spacing == SpacingMode::ConstantTime {
                150.0
            } else {
                14400.0
            },
        };
        let policy = RenderPolicy {
            notes: hidden != "notes",
            misc_events: hidden != "misc",
            ..Default::default()
        };
        let candidates = inventory.collect(
            bounds,
            spacing == SpacingMode::ConstantTime,
            lanes,
            4,
            policy,
            mods.alt,
            &misc,
            screen,
            mode,
        );
        let before = session.serialize().unwrap();
        selection
            .finish_region(&session, &candidates, mods, RegionMode::TimeOrPosition)
            .unwrap();
        let id = |e: Event| match e {
            Event::Note(p) => p.row,
            Event::SongWidget(_) => 1000,
            _ => 2000,
        };
        let actual: Vec<_> = selection.selected_events().map(id).collect();
        let expected: Vec<_> = case["selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert_eq!(actual, expected, "{case}");
        assert_eq!(
            selection.last_selected_event().map(id),
            case["last"].as_u64(),
            "{case}"
        );
        assert_eq!(session.serialize().unwrap(), before);
    }
}
#[test]
fn interval_lane_policy_uses_real_anchor_and_visibility_does_not_refilter_shift_extension() {
    let mut session = fixture();
    let inventory = Inventory::capture(&mut session, 0).unwrap();
    let mut selection = inventory.choices();
    let find = |row| {
        inventory
            .entries()
            .iter()
            .find_map(|e| matches!(e.event,Event::Note(p)if p.row==row).then_some(e.event))
            .unwrap()
    };
    let anchor = find(12);
    let endpoint = find(96);
    let i = selection
        .events()
        .iter()
        .position(|e| *e == anchor)
        .unwrap();
    selection.set_selected(i, true).unwrap();
    selection
        .finish_region(
            &session,
            &[endpoint],
            Modifiers {
                shift: true,
                control: true,
                alt: false,
            },
            RegionMode::TimeOrPositionAndLane,
        )
        .unwrap();
    let rows: Vec<_> = selection
        .selected_events()
        .filter_map(|e| match e {
            Event::Note(p) => Some(p.row),
            _ => None,
        })
        .collect();
    assert!(rows.contains(&12));
    assert!(rows.contains(&96));
    assert!(
        !rows.contains(&24),
        "lane2 lies outside anchor lane1 to endpoint lane0 interval"
    );
    assert_eq!(selection.last_selected_event(), Some(endpoint));
    let mut other = inventory.choices();
    other.set_selected(i, true).unwrap();
    other
        .finish_region(
            &session,
            &[endpoint],
            Modifiers {
                shift: true,
                ..Default::default()
            },
            RegionMode::TimeOrPosition,
        )
        .unwrap();
    assert!(
        other.selected_events().any(|e| e == find(24)),
        "Shift interval does not recheck initial render visibility"
    );
    let before: Vec<_> = other.selected_events().collect();
    let foreign = {
        let mut s = fixture();
        let inv = Inventory::capture(&mut s, 0).unwrap();
        inv.entries()
            .iter()
            .find(|e| matches!(e.event, Event::SongWidget(_)))
            .unwrap()
            .event
    };
    let mut changed = deadsync_editor::song_properties::Draft::capture(&session).unwrap();
    changed.set_sample_start(3.0).unwrap();
    changed.apply(&mut session).unwrap();
    assert!(
        other
            .finish_region(
                &session,
                &[foreign],
                Modifiers::default(),
                RegionMode::default()
            )
            .is_err()
    );
    assert_eq!(other.selected_events().collect::<Vec<_>>(), before);
}
#[test]
fn inventory_times_keep_delay_stop_and_widget_phases_and_invalidate_only_on_content() {
    let mut session=Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#SAMPLESTART:0.609;#LASTSECONDHINT:0.709;#NOTEDATA:;#STEPSTYPE:dance-single;#DELAYS:1=0.25;#STOPS:1=0.5;#SPEEDS:1=2=0=0;#LABELS:1=before;#NOTES:0000\n1000\n0000\n0000;".as_slice(),Format::Ssc).unwrap());
    let inventory = Inventory::capture(&mut session, 0).unwrap();
    let entries = inventory.entries();
    let label = entries
        .iter()
        .find(|e| {
            matches!(
                e.event,
                Event::Timing {
                    row: 48,
                    kind: deadsync_editor::event_clipboard::TimingKind::Label
                }
            )
        })
        .unwrap();
    let note = entries
        .iter()
        .find(|e| matches!(e.event,Event::Note(p)if p.row==48))
        .unwrap();
    assert_eq!(label.chart_time, 0.5);
    assert_eq!(note.chart_time, 0.75);
    assert!(
        entries
            .iter()
            .filter_map(|e| match e.event {
                Event::SongWidget(w) => Some(w),
                _ => None,
            })
            .all(|w| w.position() == 48.0)
    );
    let widget_times: Vec<_> = entries
        .iter()
        .filter_map(|e| matches!(e.event, Event::SongWidget(_)).then_some(e.chart_time))
        .collect();
    assert_ne!(
        widget_times[0], widget_times[1],
        "distinct stop-interior widget times must not round-trip through a row"
    );
    assert!(inventory.is_current(&session, 0));
    let prefs = deadsync_editor::selection_preferences::Preferences::default();
    let mut other = prefs;
    other.overlap = Overlap::All;
    session
        .set_selection_preferences(prefs, other, None)
        .unwrap();
    assert!(inventory.is_current(&session, 0));
}
