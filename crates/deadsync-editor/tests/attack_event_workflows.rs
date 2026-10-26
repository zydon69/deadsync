use deadsync_editor::{
    attacks::{Attacks, Draft, Modifier},
    document::{Document, Format},
    history::Session,
    timing_edit::Scope,
};
use serde_json::Value;
use std::sync::Arc;
fn document(format: Format) -> Document {
    let bytes:&[u8]=match format {
        Format::Sm=>b"// keep\n#TITLE:Source;#ATTACKS:TIME=2:LEN=3:MODS=50% Drunk;#NOTES:dance-single::Hard:9:0,0,0,0,0:1000;",
        Format::Ssc=>b"// keep\n#VERSION:0.83;#TITLE:Source;#BPMS:0=120;#ATTACKS:TIME=2:LEN=3:MODS=50% Drunk;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;#NOTEDATA:;#STEPSTYPE:dance-single;#ATTACKS:TIME=4:LEN=1:MODS=100% Dizzy;#NOTES:0100;",
    };
    Document::parse(Arc::<[u8]>::from(bytes), format).unwrap()
}
#[test]
fn actual_pinned_fumen_attack_parser_matches_times_defaults_groups_and_all_modifier_values() {
    let cases: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/attack-events.expected.json"
    ))
    .unwrap();
    for case in cases.as_array().unwrap() {
        let attacks = Attacks::parse(case["Source"].as_str().unwrap().as_bytes()).unwrap();
        let expected = case["Events"].as_array().unwrap();
        assert_eq!(attacks.events().len(), expected.len());
        for (event, reference) in attacks.events().iter().zip(expected) {
            assert_eq!(event.time, reference["Time"].as_f64().unwrap());
            let mods = reference["Modifiers"].as_array().unwrap();
            assert_eq!(event.modifiers.len(), mods.len());
            for (modifier, reference) in event.modifiers.iter().zip(mods) {
                assert_eq!(modifier.name, reference["Name"].as_str().unwrap());
                assert_eq!(modifier.level, reference["Level"].as_f64().unwrap());
                assert_eq!(modifier.speed, reference["Speed"].as_f64().unwrap());
                assert_eq!(modifier.length, reference["Length"].as_f64().unwrap());
            }
        }
        assert_eq!(
            Attacks::parse(&attacks.serialize().unwrap()).unwrap(),
            attacks
        );
    }
}
#[test]
fn sm_and_ssc_shared_attack_edit_is_one_undo_and_preserves_notes_unknown_tags_and_local_overrides()
{
    for format in [Format::Sm, Format::Ssc] {
        let mut session = Session::new(document(format));
        let before = session.serialize().unwrap();
        let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
        assert!(!draft.apply(&mut session, 0).unwrap());
        let mut modifier = draft.attacks().events()[0].modifiers[0].clone();
        modifier.level = -75.0;
        modifier.speed = 2.5;
        modifier.length = 4.0;
        draft.attacks_mut().set(0, 0, 1.25, modifier).unwrap();
        draft
            .attacks_mut()
            .add(
                1.25,
                Modifier {
                    name: "Reverse".into(),
                    length: 8.0,
                    ..Modifier::default()
                },
            )
            .unwrap();
        assert!(draft.apply(&mut session, 0).unwrap());
        let after = session.serialize().unwrap();
        assert!(String::from_utf8_lossy(&after).contains("#TITLE:Source;"));
        let reopened =
            Session::new(Document::parse(Arc::<[u8]>::from(after.clone()), format).unwrap());
        assert_eq!(
            Draft::capture(&reopened, 0, Scope::Song).unwrap().attacks(),
            draft.attacks()
        );
        if format == Format::Ssc {
            assert_eq!(
                Draft::capture(&reopened, 1, Scope::Chart)
                    .unwrap()
                    .attacks()
                    .events()[0]
                    .time,
                4.0
            );
        }
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
        let unchanged = Draft::capture(&session, 0, Scope::Song).unwrap();
        assert!(!unchanged.apply(&mut session, 0).unwrap());
        assert!(session.redo_label().is_some());
        session.redo().unwrap();
        assert_eq!(session.serialize().unwrap(), after);
    }
}
#[test]
fn chart_attack_inheritance_is_independent_of_split_timing_and_sibling_charts() {
    let mut session = Session::new(document(Format::Ssc));
    let before = session.serialize().unwrap();
    let mut local = Draft::capture(&session, 0, Scope::Chart).unwrap();
    assert!(local.inherited());
    local.attacks_mut().add(5.0, Modifier::default()).unwrap();
    local.apply(&mut session, 0).unwrap();
    assert!(session.document().raw_property(Some(0), "BPMS").is_none());
    assert_eq!(
        Draft::capture(&session, 0, Scope::Song)
            .unwrap()
            .attacks()
            .events()
            .len(),
        1
    );
    assert_eq!(
        Draft::capture(&session, 1, Scope::Chart)
            .unwrap()
            .attacks()
            .events()
            .len(),
        1
    );
    assert!(
        !Draft::capture(&session, 0, Scope::Chart)
            .unwrap()
            .inherited()
    );
    let mut deleting = Draft::capture(&session, 0, Scope::Chart).unwrap();
    while !deleting.attacks().events().is_empty() {
        deleting.attacks_mut().remove(0).unwrap();
    }
    let current = session.serialize().unwrap();
    assert!(deleting.apply(&mut session, 0).unwrap());
    assert!(
        Draft::capture(&session, 0, Scope::Chart)
            .unwrap()
            .attacks()
            .events()
            .is_empty()
    );
    assert_eq!(
        Draft::capture(&session, 1, Scope::Chart)
            .unwrap()
            .attacks()
            .events()
            .len(),
        1
    );
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), current);
    session.undo().unwrap();
    assert_eq!(session.serialize().unwrap(), before);
    assert!(Draft::capture(&Session::new(document(Format::Sm)), 0, Scope::Chart).is_err());
}
#[test]
fn names_are_escaped_once_without_tag_comment_or_parameter_injection() {
    let mut session = Session::new(document(Format::Ssc));
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    let name = "Écran ;#TITLE:injected, slash//name = end\\tail";
    draft
        .attacks_mut()
        .add(
            8.0,
            Modifier {
                name: name.into(),
                ..Modifier::default()
            },
        )
        .unwrap();
    draft.apply(&mut session, 0).unwrap();
    let bytes = session.serialize().unwrap();
    let reopened = Session::new(Document::parse(Arc::<[u8]>::from(bytes), Format::Ssc).unwrap());
    assert_eq!(
        reopened.document().text(None, "TITLE").as_deref(),
        Some("Source")
    );
    assert_eq!(
        Draft::capture(&reopened, 0, Scope::Song)
            .unwrap()
            .attacks()
            .events()[1]
            .modifiers[0]
            .name,
        name
    );
}
#[test]
fn invalid_mutations_collisions_and_stale_drafts_are_atomic() {
    let mut attacks =
        Attacks::parse(b"TIME=0:LEN=1:MODS=100% Dizzy:TIME=2:LEN=3:MODS=50% Drunk").unwrap();
    let before = attacks.clone();
    assert!(attacks.set(0, 0, 2.0, Modifier::default()).is_err());
    assert_eq!(attacks, before);
    for invalid in [
        Modifier {
            length: -1.0,
            ..Modifier::default()
        },
        Modifier {
            level: f64::NAN,
            ..Modifier::default()
        },
        Modifier {
            speed: f64::INFINITY,
            ..Modifier::default()
        },
        Modifier {
            name: "no".into(),
            ..Modifier::default()
        },
    ] {
        assert!(attacks.set(0, 0, 1.0, invalid.clone()).is_err());
        assert!(attacks.add(1.0, invalid).is_err());
        assert_eq!(attacks, before);
    }
    assert!(attacks.add(f64::NAN, Modifier::default()).is_err());
    assert!(attacks.remove(99).is_err());
    assert!(attacks.remove_modifier(0, 99).is_err());
    let mut session = Session::new(document(Format::Ssc));
    let mut draft = Draft::capture(&session, 0, Scope::Song).unwrap();
    draft.attacks_mut().add(9.0, Modifier::default()).unwrap();
    assert!(draft.apply(&mut session, 1).is_err());
    assert!(
        draft
            .apply(&mut Session::new(document(Format::Ssc)), 0)
            .is_err()
    );
    session.set_text(None, "ARTIST", "Other").unwrap();
    let bytes = session.serialize().unwrap();
    assert!(draft.apply(&mut session, 0).is_err());
    assert_eq!(session.serialize().unwrap(), bytes);
    for malformed in [
        "TIME=NaN:MODS=Dizzy",
        "LEN=1",
        "UNKNOWN=4:MODS=Dizzy",
        "TIME=1:LEN=2:MODS=*NaN Dizzy",
    ] {
        assert!(Attacks::parse(malformed.as_bytes()).is_err());
    }
}
#[test]
fn modifier_deletion_and_event_movement_preserve_other_modifiers_and_reorder_by_time() {
    let mut attacks = Attacks::default();
    attacks.add(2.0, Modifier::default()).unwrap();
    attacks
        .add(
            2.0,
            Modifier {
                name: "Reverse".into(),
                length: 9.0,
                ..Modifier::default()
            },
        )
        .unwrap();
    attacks.add(-1.0, Modifier::default()).unwrap();
    let index = attacks
        .set(
            1,
            0,
            -3.0,
            Modifier {
                level: 25.0,
                ..Modifier::default()
            },
        )
        .unwrap();
    assert_eq!(index, 0);
    assert_eq!(attacks.events()[0].modifiers[1].length, 9.0);
    attacks.remove_modifier(0, 0).unwrap();
    assert_eq!(attacks.events()[0].modifiers[0].name, "Reverse");
    attacks.remove_modifier(0, 0).unwrap();
    assert_eq!(attacks.events().len(), 2);
    assert_eq!(attacks.events()[0].time, -3.0);
    assert!(attacks.events()[0].modifiers.is_empty());
    assert_eq!(
        Attacks::parse(&attacks.serialize().unwrap()).unwrap(),
        attacks
    );
    attacks.remove(0).unwrap();
    assert_eq!(attacks.events().len(), 1);
    assert_eq!(attacks.events()[0].time, -1.0);
}

#[test]
fn attack_cursor_length_and_endpoint_use_chart_offset_but_ignore_audio_sync_offset() {
    use deadsync_editor::attacks::{cursor_time, endpoint_row, length_to_cursor};
    let mut session = Session::new(document(Format::Ssc));
    session.set_text(None, "OFFSET", "0.25").unwrap();
    let mut song = deadsync_editor::song_properties::Draft::capture(&session).unwrap();
    song.set_sync_offset(0.75).unwrap();
    song.apply(&mut session).unwrap();
    assert_eq!(cursor_time(&session, 0, 192).unwrap(), 1.75);
    assert_eq!(length_to_cursor(&session, 0, 192, 0.75).unwrap(), 1.0);
    assert_eq!(length_to_cursor(&session, 0, 0, 2.0).unwrap(), 0.0);
    assert_eq!(length_to_cursor(&session, 0, 192, -10.0).unwrap(), 2.0);
    assert_eq!(endpoint_row(&session, 0, 0.75, 1.0).unwrap(), 192);
    assert_eq!(endpoint_row(&session, 0, -10.0, 0.0).unwrap(), 0);
    assert!(endpoint_row(&session, 0, f64::MAX, f64::MAX).is_err());
    assert!(endpoint_row(&session, 0, 0.0, -1.0).is_err());
    // A chart override is independent of song/global audio playback alignment.
    session.set_text(Some(0), "OFFSET", "0.5").unwrap();
    assert_eq!(cursor_time(&session, 0, 192).unwrap(), 1.5);
    assert_eq!(endpoint_row(&session, 0, 0.5, 1.0).unwrap(), 192);
    let mut rates = Session::new(document(Format::Ssc));
    rates.set_text(None, "BPMS", "0=120,4=240").unwrap();
    assert_eq!(
        deadsync_editor::attacks::default_length(&rates, 0, 191).unwrap(),
        0.5
    );
    assert_eq!(
        deadsync_editor::attacks::default_length(&rates, 0, 192).unwrap(),
        0.25
    );
}

#[test]
fn clearing_all_shared_and_local_attacks_is_reversible_when_no_inherited_effects_remain() {
    for format in [Format::Sm, Format::Ssc] {
        let mut session = Session::new(document(format));
        let before = session.serialize().unwrap();
        let mut shared = Draft::capture(&session, 0, Scope::Song).unwrap();
        shared.attacks_mut().remove(0).unwrap();
        assert!(shared.apply(&mut session, 0).unwrap());
        let cleared = session.serialize().unwrap();
        let reopened = Session::new(Document::parse(cleared.clone(), format).unwrap());
        assert!(
            Draft::capture(&reopened, 0, Scope::Song)
                .unwrap()
                .attacks()
                .events()
                .is_empty()
        );
        if format == Format::Ssc {
            assert!(
                Draft::capture(&reopened, 0, Scope::Chart)
                    .unwrap()
                    .attacks()
                    .events()
                    .is_empty()
            );
            assert_eq!(
                Draft::capture(&reopened, 1, Scope::Chart)
                    .unwrap()
                    .attacks()
                    .events()
                    .len(),
                1
            );
            let mut local = Draft::capture(&session, 1, Scope::Chart).unwrap();
            local.attacks_mut().remove(0).unwrap();
            local.apply(&mut session, 1).unwrap();
            let final_bytes = session.serialize().unwrap();
            session.undo().unwrap();
            assert_eq!(session.serialize().unwrap(), cleared);
            session.redo().unwrap();
            assert_eq!(session.serialize().unwrap(), final_bytes);
            session.undo().unwrap();
        }
        session.undo().unwrap();
        assert_eq!(session.serialize().unwrap(), before);
    }
}
