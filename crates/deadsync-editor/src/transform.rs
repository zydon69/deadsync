//! Selection transforms preserve owners, sustain tails, and note attributes.
//! Refer to GrooveAuthor EditorActions/Action*Selection*.cs (MIT).
pub(crate) mod preview;
use crate::{
    Error,
    history::Session,
    notes::{Kind, Position},
    pad::Pad,
};
use std::{collections::BTreeSet, sync::atomic::AtomicBool};

#[derive(Clone, Copy, Debug)]
pub enum Transform {
    Mirror,
    Flip,
    MirrorAndFlip,
    ShiftRows(i64),
    ShiftLanes {
        right: bool,
        wrap: bool,
    },
    AssignPlayer(u32),
    SwapPlayers(u32, u32),
    Convert {
        from: Kind,
        to: Kind,
        sustain_rows: u64,
    },
}

pub fn apply(session: &mut Session, chart: usize, start: u64, end: u64, player: Option<u32>, transform: Transform) -> Result<bool, Error> {
    if start > end { return Err(Error::InvalidEdit("Selection ends before it starts".into())); }
    let heads = session.grid(chart)?.iter().filter(|(position, _)| (start..=end).contains(&position.row) && player.is_none_or(|p| p == position.player)).map(|(position, _)| position).collect();
    apply_positions(session, chart, &heads, transform)
}
/// Same native reference engine for exact checked heads. Spatial ForceInsert and
/// strict ownership collision contracts remain separate inside the preview.
pub fn apply_positions(session: &mut Session, chart: usize, heads: &BTreeSet<Position>, transform: Transform) -> Result<bool, Error> {
    if let Transform::Convert { from, to, sustain_rows } = transform {
        let plan = crate::note_operation::Plan::prepare_bounded(
            session, chart, crate::note_operation::Scope::Heads(heads.clone()),
            crate::note_operation::Operation::Convert { from, to, sustain_rows },
            usize::MAX, &AtomicBool::new(false),
        ).map_err(Error::from)?;
        return plan.apply_named(session, chart, &format!("{transform:?}")).map_err(Into::into);
    }
    // Legacy immediate commands use the same atomic transaction as reviews.
    // Their existing range/player scope and lack of a scan quota are retained.
    let plan = crate::selected_transform::Plan::prepare_bounded(
        session, chart, crate::note_operation::Scope::Heads(heads.clone()),
        transform, usize::MAX, &AtomicBool::new(false),
    ).map_err(Error::from)?;
    let label = match transform {
        Transform::ShiftRows(_) => "Shift note rows".into(),
        Transform::ShiftLanes { .. } => "Shift note columns".into(),
        Transform::Mirror | Transform::Flip | Transform::MirrorAndFlip => "Transform note lanes".into(),
        _ => format!("{transform:?}"),
    };
    plan.apply_named(session, chart, plan.impacts(), &label).map_err(Into::into)
}

fn symmetry_lane(pad: &Pad, lane: u16, transform: Transform) -> Option<u16> {
    let lane = if matches!(transform, Transform::Mirror | Transform::MirrorAndFlip) {
        pad.mirror[usize::from(lane)]?
    } else {
        lane
    };
    if matches!(transform, Transform::Flip | Transform::MirrorAndFlip) {
        pad.flip[usize::from(lane)]
    } else {
        Some(lane)
    }
}

fn shift(row: u64, delta: i64) -> Result<u64, Error> {
    if delta >= 0 {
        row.checked_add(delta as u64)
    } else {
        row.checked_sub(delta.unsigned_abs())
    }
    .ok_or_else(|| Error::InvalidEdit("Shift exceeds the valid row range".into()))
}

#[cfg(test)]
mod selection_history_tests {
    use super::*;
    use crate::{document::{Document, Format}, event_clipboard::Event};

    #[test]
    fn direct_range_and_swap_routes_publish_the_same_selection_history_as_reviews() {
        for mode in 0..3 {
            let mut session = Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0000\n0000;".to_vec(), Format::Ssc).unwrap());
            let original = Position { row: 0, lane: 0, player: 0 };
            let target = if mode == 0 { Position { lane: 3, ..original } } else { original };
            let before = session.serialize().unwrap();
            let changed = match mode {
                0 => apply(&mut session, 0, 0, 0, None, Transform::Mirror).unwrap(),
                1 => apply(&mut session, 0, 0, 0, None, Transform::Convert { from: Kind::Tap, to: Kind::Mine, sustain_rows: 0 }).unwrap(),
                _ => crate::note_swap::apply_detailed(&mut session, 0, &BTreeSet::from([original]), crate::note_swap::Pair::TapsMines).unwrap(),
            };
            assert!(changed);
            let after = session.serialize().unwrap();
            assert_eq!(session.take_event_selection_update().unwrap().events(), &[Event::Note(target)]);
            session.undo().unwrap();
            assert_eq!(session.serialize().unwrap(), before);
            assert_eq!(session.take_event_selection_update().unwrap().events(), &[Event::Note(original)]);
            session.redo().unwrap();
            assert_eq!(session.serialize().unwrap(), after);
            assert_eq!(session.take_event_selection_update().unwrap().events(), &[Event::Note(target)]);
        }
    }
}
