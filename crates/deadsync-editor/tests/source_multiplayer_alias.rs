use deadsync_editor::{
    audio_preferences::{
        File,
        source_import::{Disposition, Draft},
    },
    document::{Document, Format},
    history::Session,
    key_binding_preferences::source_profile::Platform,
    note_color_palette::Palette,
};
#[test]
fn source_json_color_holds_alias_changes_actual_multiplayer_hold_palette_and_native_name_stays_unknown()
 {
    let mut session =
        Session::new(Document::parse(b"#TITLE:Source;".as_slice(), Format::Sm).unwrap());
    let file = File::memory().unwrap();
    let make = |raw: &[u8], session: &mut Session| {
        Draft::prepare(session, None, &file, raw, Platform::Mac).unwrap()
    };
    let enabled = make(
        br#"{"PreferencesMultiplayer":{"ColorHoldsAndRolls":true}}"#,
        &mut session,
    );
    let disabled = make(
        br#"{"PreferencesMultiplayer":{"ColorHoldsAndRolls":false}}"#,
        &mut session,
    );
    assert!(
        !disabled
            .target()
            .note_colors_arc()
            .colors
            .color_multiplayer_holds
    );
    assert!(
        disabled
            .report()
            .items
            .iter()
            .any(|i| i.path == "PreferencesMultiplayer.ColorHoldsAndRolls"
                && i.disposition == Disposition::Applied)
    );
    assert!(
        !disabled
            .report()
            .items
            .iter()
            .any(|i| i.path == "PreferencesMultiplayer.ColorMultiplayerHoldsAndRolls")
    );
    let field = disabled
        .report()
        .items
        .iter()
        .find(|i| i.path == "PreferencesMultiplayer.ColorHoldsAndRolls")
        .unwrap();
    assert_eq!(field.raw, Some(serde_json::json!(false)));
    assert_eq!(field.effective, Some(serde_json::json!(false)));
    let enabled = Palette::capture(enabled.target().note_colors_arc()).unwrap();
    let disabled = Palette::capture(disabled.target().note_colors_arc()).unwrap();
    let a = enabled
        .fills("dance-routine", 0, 0, 0, false, false, false, false)
        .unwrap();
    let b = disabled
        .fills("dance-routine", 0, 0, 0, false, false, false, false)
        .unwrap();
    assert_eq!(a.head, b.head);
    assert_ne!(a.body, b.body);
    let unknown = make(
        br#"{"PreferencesMultiplayer":{"ColorMultiplayerHoldsAndRolls":1e999}}"#,
        &mut session,
    );
    assert!(
        unknown
            .target()
            .note_colors_arc()
            .colors
            .color_multiplayer_holds
    );
    assert!(unknown.report().items.iter().any(|i| i.path
        == "PreferencesMultiplayer.ColorMultiplayerHoldsAndRolls"
        && i.disposition == Disposition::Retained));
}
#[test]
fn every_applied_or_defaulted_group_report_path_is_an_actual_source_json_property() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/global_source_fidelity.expected.json"
    ))
    .unwrap();
    let constructor = &reference["cases"][0]["value"];
    let mut session =
        Session::new(Document::parse(b"#TITLE:Source;".as_slice(), Format::Sm).unwrap());
    let file = File::memory().unwrap();
    let draft = Draft::prepare(&mut session, None, &file, b"{}", Platform::Mac).unwrap();
    for row in &draft.report().items {
        if !matches!(
            row.disposition,
            Disposition::Applied | Disposition::Defaulted
        ) {
            continue;
        }
        let Some((group, field)) = row.path.split_once('.') else {
            continue;
        };
        if group == "PreferencesKeyBinds" {
            continue;
        }
        assert!(
            constructor.get(group).and_then(|g| g.get(field)).is_some(),
            "Fabricated source property {}",
            row.path
        );
    }
}
