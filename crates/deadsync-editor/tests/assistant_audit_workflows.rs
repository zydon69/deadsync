use deadsync_editor::{
    assistant::Toolbox,
    document::{Document, Format},
    file_origin::{NewSong, Origin},
    history::Session,
    storage::FileSession,
    workspace::Workspace,
};
use serde_json::{Value, json};
use std::sync::{Arc, atomic::AtomicBool};
fn workspace() -> Workspace {
    let document=Document::parse(Arc::<[u8]>::from(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#DIFFICULTY:Edit;#METER:1;#NOTES:1111;".as_slice()),Format::Ssc).unwrap();
    Workspace::from_file(
        FileSession::from_new(NewSong {
            origin: Origin::Untitled {
                resource_base: std::env::temp_dir(),
            },
            session: Session::new(document),
        })
        .unwrap(),
        0,
    )
    .unwrap()
}
#[test]
fn cursor_pages_every_note_of_a_chord_exactly_once() {
    let mut workspace = workspace();
    let mut toolbox = Toolbox::default();
    let cancelled = AtomicBool::new(false);
    let mut cursor = None;
    let mut notes = Vec::new();
    loop {
        let mut arguments = json!({"chart":0,"limit":1});
        if let Some(cursor) = cursor {
            arguments["cursor"] = cursor;
        }
        let page = toolbox
            .call(
                &mut workspace,
                "inspect_chart",
                arguments,
                false,
                &cancelled,
            )
            .unwrap();
        notes.extend(page["notes"].as_array().unwrap().iter().cloned());
        cursor = page.get("next_cursor").filter(|v| !v.is_null()).cloned();
        if cursor.is_none() {
            break;
        }
        assert!(notes.len() <= 4);
    }
    assert_eq!(notes.len(), 4);
    let lanes: std::collections::BTreeSet<_> =
        notes.iter().map(|v| v["lane"].as_u64().unwrap()).collect();
    assert_eq!(lanes, std::collections::BTreeSet::from([0, 1, 2, 3]));
}
#[test]
fn unchanged_note_preview_does_not_claim_an_applied_edit() {
    let mut workspace = workspace();
    let mut toolbox = Toolbox::default();
    let cancelled = AtomicBool::new(false);
    let preview = toolbox
        .call(
            &mut workspace,
            "preview_note_edits",
            json!({"chart":0,"edits":[{"row":0,"lane":0,"kind":"tap"}]}),
            false,
            &cancelled,
        )
        .unwrap();
    let revision = workspace.file().session.revision();
    let response = toolbox
        .call(
            &mut workspace,
            "apply_preview",
            json!({"preview_id":preview["preview_id"]}),
            false,
            &cancelled,
        )
        .unwrap();
    assert_eq!(response["applied"], Value::Bool(false));
    assert_eq!(workspace.file().session.revision(), revision);
    assert!(!workspace.file().session.is_dirty());
}
#[test]
fn empty_pad_fits_without_panicking_and_nonempty_cannot_fit_in_it() {
    let empty = deadsync_editor::pad::Pad {
        starting_positions: Vec::new(),
        arrows: Vec::new(),
        mirror: Vec::new(),
        flip: Vec::new(),
    };
    let full = deadsync_editor::pad::Pad::builtin("dance-single").unwrap();
    assert!(empty.can_fit_within(&empty));
    assert!(empty.can_fit_within(&full));
    assert!(!full.can_fit_within(&empty));
}

#[test]
fn inspected_note_kinds_can_be_used_directly_in_edits() {
    let mut workspace = workspace();
    let mut toolbox = Toolbox::default();
    let cancelled = AtomicBool::new(false);
    let page = toolbox
        .call(
            &mut workspace,
            "inspect_chart",
            json!({}),
            false,
            &cancelled,
        )
        .unwrap();
    let note = &page["notes"][0];
    assert_eq!(note["kind"], "tap");
    let revision = workspace.file().session.revision();
    let preview = toolbox
        .call(
            &mut workspace,
            "preview_note_edits",
            json!({"edits":[note]}),
            false,
            &cancelled,
        )
        .unwrap();
    toolbox
        .call(
            &mut workspace,
            "apply_preview",
            json!({"preview_id":preview["preview_id"]}),
            false,
            &cancelled,
        )
        .unwrap();
    assert_eq!(workspace.file().session.revision(), revision);
}

#[test]
fn unavailable_history_preserves_the_pending_proposal() {
    let mut workspace = workspace();
    let mut toolbox = Toolbox::default();
    let cancelled = AtomicBool::new(false);
    toolbox
        .call(
            &mut workspace,
            "preview_note_edits",
            json!({"edits":[{"row":48,"lane":0,"kind":"tap"}]}),
            false,
            &cancelled,
        )
        .unwrap();
    let preview = toolbox.preview().cloned();
    for name in ["undo", "redo"] {
        let _ = toolbox.call(&mut workspace, name, json!({}), false, &cancelled);
        assert_eq!(toolbox.preview(), preview.as_ref());
    }
}

#[test]
fn chart_pages_are_chronological_and_bounded_by_end_row() {
    let mut workspace = workspace();
    let mut toolbox = Toolbox::default();
    let cancelled = AtomicBool::new(false);
    let preview = toolbox
        .call(
            &mut workspace,
            "preview_note_edits",
            json!({"edits":[
                {"row":192,"lane":0,"kind":"tap"},
                {"row":48,"lane":3,"kind":"tap"},
                {"row":96,"lane":1,"kind":"tap"}
            ]}),
            false,
            &cancelled,
        )
        .unwrap();
    toolbox
        .call(
            &mut workspace,
            "apply_preview",
            json!({"preview_id":preview["preview_id"]}),
            false,
            &cancelled,
        )
        .unwrap();
    let first = toolbox
        .call(
            &mut workspace,
            "inspect_chart",
            json!({"start_row":1,"end_row":96,"limit":1}),
            false,
            &cancelled,
        )
        .unwrap();
    assert_eq!(first["notes"][0]["row"], 48);
    assert_eq!(first["notes"][0]["lane"], 3);
    let second = toolbox
        .call(
            &mut workspace,
            "inspect_chart",
            json!({"start_row":1,"end_row":96,"limit":1,"cursor":first["next_cursor"]}),
            false,
            &cancelled,
        )
        .unwrap();
    assert_eq!(second["notes"][0]["row"], 96);
    assert!(second["next_cursor"].is_null());
}

#[test]
fn connected_transport_recovers_after_invalid_json() {
    use std::io::Cursor;
    let input = b"{broken\n\n{\"jsonrpc\":\"2.0\",\"id\":42,\"method\":\"ping\"}\n";
    let mut output = Vec::new();
    // No connection exists: the valid frame must still receive a tool error,
    // proving that malformed input did not terminate the transport loop.
    deadsync_editor::assistant::bridge::connect(
        std::path::Path::new("/nonexistent-deadsync-review-connection"),
        Cursor::new(input),
        &mut output,
    )
    .unwrap();
    let responses: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["error"]["code"], -32700);
    assert!(responses[0]["id"].is_null());
    assert_eq!(responses[1]["id"], 42);
}
