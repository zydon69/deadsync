use deadsync_editor::Error;
#[path = "../src/minimap_palette.rs"]
mod minimap_palette;
use minimap_palette::{ColorMethod, Family, Palette, Preferences};
fn rgb(value: &serde_json::Value) -> [f32; 3] {
    std::array::from_fn(|i| value[i].as_f64().unwrap() as f32)
}
#[test]
fn original_cached_palette_matches_4224_csharp_lookups() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/minimap-palette.reference.json"
    ))
    .unwrap();
    assert_eq!(
        reference["reference_commit"],
        "48e18f71655d0f0d86144c63239331833501fda2"
    );
    assert_eq!(
        reference["fumen_commit"],
        "0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89"
    );
    let mut count = 0;
    for (index, config) in reference["configurations"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let input = &config["input"];
        let mut prefs = Preferences::default();
        let subdivisions = std::array::from_fn(|i| rgb(&input["subdivisions"][i]));
        if index == 0 {
            assert_eq!(prefs.subdivisions, subdivisions);
            assert_eq!(prefs.hold, rgb(&input["hold"]));
            assert_eq!(prefs.roll, rgb(&input["roll"]));
            assert_eq!(prefs.mine, rgb(&input["mine"]));
            assert_eq!(
                prefs.players,
                input["players"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(rgb)
                    .collect::<Vec<_>>()
            );
        }
        prefs.subdivisions = subdivisions;
        prefs.hold = rgb(&input["hold"]);
        prefs.roll = rgb(&input["roll"]);
        prefs.mine = rgb(&input["mine"]);
        prefs.piu_top = rgb(&input["piuTop"]);
        prefs.piu_middle = rgb(&input["piuMiddle"]);
        prefs.piu_bottom = rgb(&input["piuBottom"]);
        prefs.players = input["players"]
            .as_array()
            .unwrap()
            .iter()
            .map(rgb)
            .collect();
        prefs.normal_multiplier = input["normalMultiplier"].as_f64().unwrap() as f32;
        prefs.selected_multiplier = input["selectedMultiplier"].as_f64().unwrap() as f32;
        prefs.routine_alpha = input["routineAlpha"].as_f64().unwrap() as f32;
        prefs.piu_hold_saturation = input["piuSaturation"].as_f64().unwrap() as f32;
        let palette = Palette::new(&prefs).unwrap();
        for case in config["cases"].as_array().unwrap() {
            let family = match case["family"].as_str().unwrap() {
                "row" => Family::Row,
                "routine" => Family::RoutineRow,
                "pump" => Family::PumpLane {
                    start_index: 0,
                    routine: false,
                },
                "pumproutine" => Family::PumpLane {
                    start_index: 0,
                    routine: true,
                },
                _ => unreachable!(),
            };
            let colors = palette
                .colors(
                    family,
                    case["row"].as_u64().unwrap() as u8,
                    case["lane"].as_u64().unwrap() as u16,
                    case["player"].as_u64().unwrap() as u32,
                    case["selected"].as_bool().unwrap(),
                )
                .unwrap();
            let expected = &case["expected"];
            assert_eq!(
                [colors.head, colors.hold, colors.roll, colors.mine],
                std::array::from_fn::<_, 4, _>(|i| expected[["head", "hold", "roll", "mine"][i]]
                    .as_u64()
                    .unwrap() as u32),
                "configuration{index} case{case}"
            );
            count += 1;
        }
    }
    assert_eq!(count, 4224);
}
#[test]
fn chart_family_halfdouble_player_wrapping_and_multiplayer_hold_toggle_are_independent() {
    let mut prefs = Preferences::default();
    let defaults = Palette::new(&prefs).unwrap();
    assert_eq!(
        defaults.colors(Family::Row, 3, 0, 0, true).unwrap().head,
        0xffffffff
    );
    assert_eq!(
        defaults
            .colors(Family::RoutineRow, 3, 0, 0, true)
            .unwrap()
            .head,
        0xffffffff,
        "original selected multiplier intentionally saturates both palettes to white"
    );
    // Use a non-saturating selected multiplier to observe family dispatch.
    prefs.selected_multiplier = 2.0;
    prefs.color_multiplayer_holds = false;
    let palette = Palette::new(&prefs).unwrap();
    for selected in [false, true] {
        let ordinary = palette.colors(Family::Row, 3, 0, 0, selected).unwrap();
        let routine = palette
            .colors(Family::RoutineRow, 3, 0, 0, selected)
            .unwrap();
        assert_eq!(ordinary.hold, routine.hold);
        assert_eq!(ordinary.roll, routine.roll);
        assert_ne!(ordinary.head, routine.head);
        let half = palette
            .colors(
                Family::PumpLane {
                    start_index: 2,
                    routine: false,
                },
                0,
                0,
                0,
                selected,
            )
            .unwrap();
        let middle = palette
            .colors(
                Family::PumpLane {
                    start_index: 0,
                    routine: false,
                },
                0,
                2,
                0,
                selected,
            )
            .unwrap();
        assert_eq!(half, middle);
        assert_eq!(half.hold, half.roll);
        assert_eq!(
            palette
                .colors(Family::RoutineRow, 7, 0, 1, selected)
                .unwrap(),
            palette
                .colors(Family::RoutineRow, 7, 0, 11, selected)
                .unwrap()
        );
    }
    assert_eq!(ColorMethod::Stepmania.color_row(52, 4, 8).unwrap(), 4);
    assert_eq!(ColorMethod::Note.color_row(52, 4, 8).unwrap(), 4);
    assert_eq!(ColorMethod::Beat.color_row(52, 4, 8).unwrap(), 8);
    assert!(ColorMethod::Beat.color_row(0, u64::MAX, 8).is_err());
    assert!(
        palette
            .colors(
                Family::PumpLane {
                    start_index: 5,
                    routine: false
                },
                0,
                0,
                0,
                false
            )
            .is_err()
    );
    prefs.players.clear();
    assert!(Palette::new(&prefs).is_err());
}
