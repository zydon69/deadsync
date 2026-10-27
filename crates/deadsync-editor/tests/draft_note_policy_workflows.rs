//! Prospective fake flags use the editor timing pass, checked against committed heads.
use deadsync_editor::{
    autoplay::Timeline,
    document::{Document, Format},
    draft_note_policy::{Cache, MAX_DRAFTS},
    history::Session,
    lane_entry::Draft,
    notes::{Kind, Note, Position},
};
fn session(timing: &str) -> Session {
    Session::new(Document::parse(format!("#VERSION:0.83;#BPMS:0=120;#GASongDataVersion:1;#GASongData:{{\"SyncOffset\":0}};{timing}#NOTEDATA:;#STEPSTYPE:dance-routine;#NOTES:00000001\n00000000\n00000000\n00000000\n&\n00000000;").into_bytes(),Format::Ssc).unwrap())
}
fn draft(row: u64, lane: u16, player: u32, kind: Kind) -> Draft {
    Draft {
        position: Position { row, lane, player },
        kind,
        end: None,
    }
}
fn note(d: Draft) -> Note {
    Note {
        kind: d.kind,
        end: d.end,
        attributes: Default::default(),
        tail_attributes: Default::default(),
    }
}
#[test]
fn probes_match_committed_heads_through_fake_warp_delay_and_negative_stop_phases() {
    for timing in [
        "",
        "#FAKES:1=1;",
        "#WARPS:1=1;",
        "#DELAYS:1=0.125;#WARPS:1=1;",
        "#STOPS:1=-0.5;",
        "#DELAYS:1=0.125;#STOPS:1=-0.5;",
        "#STOPS:1=-0.5;#WARPS:1=1;#FAKES:3=0.5;",
    ] {
        let s = session(timing);
        let mut cache = Cache::capture(&s, 0).unwrap();
        let probes: Vec<_> = [
            0, 1, 47, 48, 49, 95, 96, 97, 143, 144, 145, 191, 192, 193, 239, 240,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            draft(
                row,
                (i % 8) as u16,
                (i / 8) as u32,
                if i == 0 {
                    Kind::Fake
                } else if i % 3 == 0 {
                    Kind::Mine
                } else {
                    Kind::Tap
                },
            )
        })
        .collect();
        cache.prepare(&s, probes.iter().copied()).unwrap();
        assert!(
            s.loaded_grid(0).is_none(),
            "classification never loads the committed chart grid"
        );
        let mut committed = session(timing);
        committed
            .edit_notes(
                0,
                "probe",
                probes.iter().map(|&d| (d.position, Some(note(d)))),
            )
            .unwrap();
        let timeline = Timeline::capture(&committed, 0).unwrap();
        for d in probes {
            let actual = timeline
                .presentation(&committed, 0, d.position, 0.0, false, false, false)
                .unwrap()
                .unwrap()
                .fake;
            assert_eq!(cache.fake(&s, d).unwrap(), actual, "{timing}, {d:?}");
        }
    }
}
#[test]
fn exact_boundaries_and_participants_share_source_classification() {
    let s = session("#FAKES:1=1;");
    let mut cache = Cache::capture(&s, 0).unwrap();
    let probes = [
        draft(47, 0, 0, Kind::Tap),
        draft(48, 0, 0, Kind::Tap),
        draft(48, 0, 1, Kind::Tap),
        draft(95, 1, 0, Kind::Mine),
        draft(96, 2, 0, Kind::Tap),
        draft(0, 3, 0, Kind::Fake),
    ];
    cache.prepare(&s, probes).unwrap();
    for (d, expected) in probes
        .into_iter()
        .zip([false, true, true, true, false, true])
    {
        assert_eq!(cache.fake(&s, d).unwrap(), expected);
    }
}
#[test]
fn identical_drafts_and_transport_frames_do_not_replay_timing() {
    let s = session("#STOPS:1=-0.5;#WARPS:3=2;");
    let mut cache = Cache::capture(&s, 0).unwrap();
    let a = draft(49, 0, 0, Kind::Tap);
    let b = draft(150, 1, 1, Kind::Mine);
    cache.prepare(&s, [a, b]).unwrap();
    let work = cache.work();
    assert_eq!(work.builds, 1);
    assert_eq!(work.probe_heads, 2);
    for _ in 0..1000 {
        cache.prepare(&s, [b, a]).unwrap();
        cache.fake(&s, a).unwrap();
        assert_eq!(cache.work(), work);
    }
    cache.prepare(&s, [draft(50, 0, 0, Kind::Tap)]).unwrap();
    assert_eq!(cache.work().builds, 2);
    assert_eq!(cache.work().probe_heads, 1);
}
#[test]
fn bounds_and_invalid_probes_leave_previous_valid_result_intact() {
    let s = session("#FAKES:1=1;");
    let mut cache = Cache::capture(&s, 0).unwrap();
    let d = draft(48, 0, 0, Kind::Tap);
    cache.prepare(&s, [d]).unwrap();
    let work = cache.work();
    assert!(cache.prepare(&s, [d, d]).is_err());
    assert!(
        cache
            .prepare(
                &s,
                (0..=MAX_DRAFTS).map(|r| draft(r as u64, 0, 0, Kind::Tap))
            )
            .is_err()
    );
    assert!(cache.prepare(&s, [draft(48, 8, 0, Kind::Tap)]).is_err());
    assert_eq!(cache.work(), work);
    assert!(cache.fake(&s, d).unwrap());
    cache.prepare(&s, []).unwrap();
    assert!(cache.fake(&s, d).is_err());
}
#[test]
fn foreign_session_external_edit_and_undo_require_exact_content_binding() {
    let mut s = session("#WARPS:1=1;");
    let mut cache = Cache::capture(&s, 0).unwrap();
    let d = draft(48, 0, 0, Kind::Tap);
    cache.prepare(&s, [d]).unwrap();
    let foreign = session("#WARPS:1=1;");
    assert!(!cache.is_current(&foreign, 0));
    assert!(cache.fake(&foreign, d).is_err());
    s.edit_notes(
        0,
        "external",
        [(
            Position {
                row: 288,
                lane: 3,
                player: 0,
            },
            Some(Note::tap()),
        )],
    )
    .unwrap();
    assert!(!cache.is_current(&s, 0));
    assert!(cache.prepare(&s, [d]).is_err());
    s.undo().unwrap();
    // ContentToken deliberately restores the original immutable content on undo.
    assert!(cache.is_current(&s, 0));
    assert!(cache.fake(&s, d).unwrap());
}

#[test]
fn extending_a_sustain_tail_reuses_unchanged_head_classification() {
    let s = session("#FAKES:1=1;");
    let mut cache = Cache::capture(&s, 0).unwrap();
    let mut d = draft(48, 0, 0, Kind::Hold);
    d.end = Some(96);
    cache.prepare(&s, [d]).unwrap();
    let work = cache.work();
    assert!(cache.fake(&s, d).unwrap());
    for end in 97..300 {
        d.end = Some(end);
        cache.prepare(&s, [d]).unwrap();
        assert!(cache.fake(&s, d).unwrap());
        assert_eq!(cache.work(), work);
    }
}
