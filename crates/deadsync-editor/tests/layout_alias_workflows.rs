use deadsync_editor::{
    document::{Document, Format, metadata::Metadata},
    history::Session,
    notes::{self, Grid, Note, Position},
    storage::FileSession,
};
use serde_json::Value;
use std::sync::Arc;
fn decode(s: &str) -> Vec<u8> {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let (mut bits, mut count, mut out) = (0u32, 0, Vec::new());
    for b in s.bytes().take_while(|v| *v != b'=') {
        bits = (bits << 6) | alphabet.iter().position(|v| *v == b).unwrap() as u32;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out
}

#[test]
fn real_fumen_metadata_and_full_reader_alias_cases_preserve_source() {
    let request: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/layout-aliases.request.json"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "fixtures/grooveauthor/layout-aliases.expected.json"
    ))
    .unwrap();
    let mut full = 0;
    let mut native_trim = 0;
    for (input, result) in request["Cases"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["Cases"].as_array().unwrap())
    {
        assert_eq!(input["Name"], result["Name"]);
        let name = input["Name"].as_str().unwrap();
        let bytes = decode(input["Bytes"].as_str().unwrap());
        let format = if input["Format"] == "sm" {
            Format::Sm
        } else {
            Format::Ssc
        };
        let document = Document::parse(Arc::<[u8]>::from(bytes.clone()), format).unwrap();
        assert_eq!(document.serialize(), bytes, "{name}");
        let metadata = Metadata::parse(Arc::<[u8]>::from(bytes.clone()), format).unwrap();
        assert_eq!(
            metadata.charts()[0].steps_type,
            document.charts()[0].steps_type,
            "{name}"
        );
        let source_charts = result["Metadata"].as_array();
        if let Some(chart) = source_charts.and_then(|charts| charts.first()) {
            assert_eq!(
                document.charts()[0].steps_type,
                chart["Layout"].as_str().unwrap(),
                "{name}"
            );
        } else if format == Format::Ssc && (name.contains("padded") || name.contains("comment")) {
            // Existing Document trim/comment policy deliberately accepts these
            // eight aliases that Fumen's SSC StepsType parser drops.
            assert!(
                notes::layout_columns(&document.charts()[0].steps_type).is_some(),
                "{name}"
            );
            native_trim += 1;
        } else {
            assert!(
                notes::layout_columns(&document.charts()[0].steps_type).is_none(),
                "{name}"
            );
        }
        if let Some(chart) = result["Full"].as_array().and_then(|charts| charts.first()) {
            let mut session = Session::new(document);
            let grid = session.grid(0).unwrap();
            assert_eq!(
                grid.columns() as u64,
                chart["Columns"].as_u64().unwrap(),
                "{name}"
            );
            assert_eq!(
                grid.players() as u64,
                chart["Players"].as_u64().unwrap(),
                "{name}"
            );
            let mut actual: Vec<_> = grid
                .iter()
                .map(|(p, _)| (p.row, p.lane as u64, p.player as u64))
                .collect();
            let mut source: Vec<_> = chart["Notes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| {
                    (
                        n["Row"].as_u64().unwrap(),
                        n["Lane"].as_u64().unwrap(),
                        n["Player"].as_u64().unwrap(),
                    )
                })
                .collect();
            actual.sort();
            source.sort();
            assert_eq!(actual, source, "{name}");
            let snapshot = deadsync_editor::chart_order::Snapshot::resident(&mut session).unwrap();
            assert_eq!(
                snapshot.keys()[0].steps_type,
                chart["Layout"].as_str().unwrap(),
                "{name}"
            );
            assert_eq!(session.serialize().unwrap(), bytes, "{name}");
            full += 1;
        }
    }
    assert_eq!(full, 48);
    assert_eq!(native_trim, 8);
}

fn source(format: Format, layout: &str, notes: &str) -> String {
    let chart = match format {
        Format::Sm => format!("#NOTES:\n // before\n {layout} // after\n :keep:Hard:9::{notes};"),
        Format::Ssc => format!(
            "#NOTEDATA:;#STEPSTYPE:\n // before\n {layout} // after\n ;#DIFFICULTY:Hard;#METER:9;#NOTES:{notes};"
        ),
    };
    format!("#VERSION:0.83;#TITLE:Aliases;#MUSIC:music.ogg;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};#BPMS:0=120;{chart}#VENDOR:raw\\:peer;")
}

#[test]
fn public_layout_and_stepf2_apis_share_semantics_without_unknown_fallback() {
    for (alias, canonical, columns) in [
        ("smx_double6", "smx-dual", 6),
        ("smx-double10", "smx-full", 10),
        ("smx_couple", "smx-team", 10),
        ("smx-routine", "smx-team", 10),
        ("40", "smx-dual", 6),
    ] {
        assert_eq!(notes::canonical_layout(alias), Some(canonical));
        assert_eq!(notes::layout_columns(alias), Some(columns));
        assert_eq!(
            Grid::parse_layout(
                format!("1{}", "0".repeat(columns as usize - 1)).as_bytes(),
                alias
            )
            .unwrap()
            .columns(),
            columns
        );
    }
    for layout in ["pump_double", "8", "pump-double"] {
        assert!(deadsync_editor::stepf2::is_coop(b"X000000000", layout));
        assert_eq!(
            Grid::parse_layout(b"X000000000", layout).unwrap().players(),
            1
        );
    }
    for layout in ["pump_routine", "10"] {
        assert_eq!(
            Grid::parse_layout(b"X000000000", layout).unwrap().players(),
            2
        );
    }
    for layout in ["999", "-1", "SMX_COUPLE", "unknown", ""] {
        assert_eq!(notes::layout_columns(layout), None);
    }
}

#[test]
fn escaped_stepf2_alias_normalization_is_reversible_after_save() {
    for format in [Format::Sm, Format::Ssc] {
        for alias in ["pump_double", "8", r"pump\_double", r"pump\-double"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(format!("song.{}", format.extension()));
            let original = source(format, alias, "XY00000000");
            std::fs::write(&path, &original).unwrap();
            let mut file = FileSession::open(&path).unwrap();
            assert_eq!(
                file.session.document().charts()[0].steps_type,
                "pump-double"
            );
            assert!(file.session.normalize_stepf2_chart(0).unwrap());
            let changed = source(format, "pump-routine", "XY00000000");
            assert_eq!(file.session.serialize().unwrap(), changed.as_bytes());
            file.save().unwrap();
            assert!(file.session.undo().unwrap());
            assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
            assert!(file.session.redo().unwrap());
            file.save().unwrap();
            let mut reopened = FileSession::open(&path).unwrap();
            assert_eq!(reopened.session.grid(0).unwrap().players(), 2);
            assert_eq!(std::fs::read(&path).unwrap(), changed.as_bytes());
        }
    }
}

#[test]
fn team_alias_owners_views_music_palette_and_edits_keep_raw_bytes() {
    for format in [Format::Sm, Format::Ssc] {
        for alias in ["smx_couple", "smx-routine", "42"] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(format!("song.{}", format.extension()));
            std::fs::write(dir.path().join("music.ogg"), b"synthetic path fixture").unwrap();
            let original = source(format, alias, "1000000000&0000000001");
            std::fs::write(&path, &original).unwrap();
            let mut file = FileSession::open(&path).unwrap();
            assert_eq!(file.session.document().charts()[0].steps_type, "smx-team");
            assert_eq!(file.session.grid(0).unwrap().players(), 2);
            let mut views =
                deadsync_editor::chart_views::ActiveChartViews::empty(&file.session, 0.0).unwrap();
            views.show(&file.session, 0, true).unwrap();
            assert_eq!(views.entries()[0].source_index(), 0);
            assert!(views.entries()[0].clock(&file.session).is_ok());
            let preview =
                deadsync_editor::playback::Preview::capture(&file.session, 0, &path).unwrap();
            assert_eq!(preview.music(), dir.path().join("music.ogg"));
            assert_eq!(preview.music_time(48).unwrap(), 0.5);
            let palette =
                deadsync_editor::note_color_palette::Palette::capture(Arc::new(Default::default()))
                    .unwrap();
            for player in [0, 1] {
                assert_eq!(
                    palette.tap(alias, 0, 9, player, false).unwrap(),
                    palette.tap("smx-team", 0, 9, player, false).unwrap()
                );
                assert_eq!(
                    palette.mine(alias, player, false).unwrap(),
                    palette.mine("smx-team", player, false).unwrap()
                );
            }
            let pos = Position {
                player: 1,
                lane: 8,
                row: 48,
            };
            file.session
                .edit_notes(0, "Alias edit", [(pos, Some(Note::tap()))])
                .unwrap();
            assert!(file.session.undo().unwrap());
            assert_eq!(file.session.serialize().unwrap(), original.as_bytes());
            assert!(file.session.redo().unwrap());
            file.save().unwrap();
            let saved = std::fs::read(&path).unwrap();
            assert!(String::from_utf8_lossy(&saved).contains(alias));
            assert!(saved.ends_with(b"#VENDOR:raw\\:peer;"));
            let mut reopened = FileSession::open(&path).unwrap();
            assert!(
                reopened
                    .session
                    .grid(0)
                    .unwrap()
                    .iter()
                    .any(|(p, _)| p == pos)
            );
            assert_eq!(
                reopened.session.document().charts()[0].steps_type,
                "smx-team"
            );
        }
    }
}
