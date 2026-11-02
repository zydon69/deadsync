//! Prospective timing clock validation without cloning or replaying external history.
use super::{ApplicationReviewToken, Change, RevisionToken, Session};
use crate::{
    Error,
    chart_focus::Position,
    timing_edit::live::{Anchor, Edit},
};
use std::sync::Arc;
enum Operation {
    Edit { label: String, change: Change },
    History { redo: bool },
    None,
}
pub struct Plan {
    review: ApplicationReviewToken,
    revision: RevisionToken,
    chart: usize,
    position: Position,
    operation: Operation,
}
pub struct Application {
    changed: bool,
    position: Position,
}
impl Application {
    pub fn changed(&self) -> bool {
        self.changed
    }
    pub fn into_position(self) -> Position {
        self.position
    }
}
fn invalid() -> Error {
    Error::InvalidEdit("History is not a typed timing edit".into())
}
// The grammar intentionally excludes DocumentSwap and all external receipts.
fn copy_property_change(change: &Change) -> Result<Change, Error> {
    Ok(match change {
        Change::Value {
            range,
            before,
            after,
        } => Change::Value {
            range: range.clone(),
            before: before.clone(),
            after: after.clone(),
        },
        Change::AddedProperty {
            chart,
            name,
            before,
            after,
        } => Change::AddedProperty {
            chart: *chart,
            name: name.clone(),
            before: before.clone(),
            after: after.clone(),
        },
        Change::AttackIdentity(change) => Change::AttackIdentity(change.clone()),
        Change::Group(changes) => Change::Group(
            changes
                .iter()
                .map(copy_property_change)
                .collect::<Result<_, _>>()?,
        ),
        Change::TimingEdit(change) => Change::TimingEdit(Box::new(copy_property_change(change)?)),
        _ => return Err(invalid()),
    })
}
pub(crate) fn presentation_candidate(session: &Session) -> Result<Session, Error> {
    let mut staged = Session::staged(session.document.clone());
    staged.identity = Arc::clone(&session.identity);
    staged.revision = session.revision;
    staged.content_revision = session.content_revision;
    staged.next_revision = session.next_revision;
    staged.chart_ids = session.chart_ids.clone();
    staged.song_timing = Some(session.song_timing_choice()?);
    staged.attack_selection = session.attack_selection.clone();
    // Song-wide attack anchoring observes every already-loaded resident grid.
    // Copy these only at commit preparation, never during a UI frame.
    staged.grids = session.grids.clone();
    staged.touched_grids = session.touched_grids.clone();
    Ok(staged)
}
impl Plan {
    pub(crate) fn prepare_edit(
        session: &Session,
        chart: usize,
        anchor: Anchor,
        edit: Edit<'_>,
    ) -> Result<Self, Error> {
        if chart >= session.chart_ids.len() {
            return Err(invalid());
        }
        let mut staged = presentation_candidate(session)?;
        let changed = edit.apply(&mut staged, chart)?;
        let operation = if changed {
            if staged.undo.len() != 1 {
                return Err(invalid());
            }
            let entry = staged.undo.pop().ok_or_else(invalid)?;
            // Validate the sealed grammar before a real publication.
            copy_property_change(&entry.change)?;
            Operation::Edit {
                label: entry.label,
                change: Change::TimingEdit(Box::new(entry.change)),
            }
        } else {
            Operation::None
        };
        let position = anchor.position(&staged, chart)?;
        Ok(Self {
            review: session.application_review_token(),
            revision: session.revision_token(),
            chart,
            position,
            operation,
        })
    }
    pub fn prepare_history(
        session: &Session,
        chart: usize,
        redo: bool,
        anchor: Anchor,
    ) -> Result<Self, Error> {
        let entry = (if redo {
            session.redo.last()
        } else {
            session.undo.last()
        })
        .filter(|entry| matches!(entry.change, Change::TimingEdit(_)))
        .ok_or_else(invalid)?;
        if chart >= session.chart_ids.len() {
            return Err(invalid());
        }
        let mut staged = presentation_candidate(session)?;
        let mut change = copy_property_change(&entry.change)?;
        staged.apply(&mut change, redo)?;
        staged.revision = if redo {
            entry.after_revision
        } else {
            entry.before_revision
        };
        staged.content_revision = if redo {
            entry.after_content_revision
        } else {
            entry.before_content_revision
        };
        let position = anchor.position(&staged, chart)?;
        Ok(Self {
            review: session.application_review_token(),
            revision: session.revision_token(),
            chart,
            position,
            operation: Operation::History { redo },
        })
    }
    pub fn apply(self, session: &mut Session) -> Result<Application, Error> {
        if !session.accepts_application_review(&self.review)
            || !session.accepts_token(&self.revision)
        {
            return Err(Error::ExternalChange);
        }
        let changed = match self.operation {
            Operation::Edit { label, change } => {
                session.execute(label, change)?;
                true
            }
            Operation::History { redo } => {
                if redo {
                    session.redo()?
                } else {
                    session.undo()?
                }
            }
            Operation::None => false,
        };
        debug_assert!(self.position.is_bound_to(session, self.chart));
        Ok(Application {
            changed,
            position: self.position,
        })
    }
}
impl Session {
    /// Closed-view history has no chart cursor to project. Validate the same
    /// property-only action before publication without inventing a focus chart.
    pub fn apply_unfocused_timing_history(&mut self, redo: bool) -> Result<bool, Error> {
        let entry = (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .filter(|entry| matches!(entry.change, Change::TimingEdit(_)))
        .ok_or_else(invalid)?;
        let mut staged = presentation_candidate(self)?;
        staged.apply(&mut copy_property_change(&entry.change)?, redo)?;
        if redo { self.redo() } else { self.undo() }
    }
    pub fn history_changes_timing_edit(&self, redo: bool) -> bool {
        (if redo {
            self.redo.last()
        } else {
            self.undo.last()
        })
        .is_some_and(|entry| matches!(entry.change, Change::TimingEdit(_)))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        timing_edit::{self as t, Scope},
    };
    fn session() -> Session {
        Session::new(Document::parse(b"#VERSION:0.83;#OFFSET:0;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100,0010,0001;".as_slice(),Format::Ssc).unwrap())
    }
    fn prepared(s: &Session, kind: usize, anchor: Anchor) -> Plan {
        macro_rules! draft {($ty:ty,$variant:ident,$($args:expr),*)=>{{let mut d=<$ty>::capture(s,0,Scope::Chart).unwrap();d.set($($args),*).unwrap();Edit::$variant(&d).prepare(s,0,anchor).unwrap()}};}
        match kind {
            0 => draft!(t::TempoDraft, Tempo, 0, 240.),
            1 => draft!(
                t::signature::Draft,
                Signatures,
                0,
                t::signature::Signature {
                    numerator: 3,
                    denominator: 4
                }
            ),
            2 => draft!(t::scroll::Draft, Scrolls, 0, -1.),
            3 => draft!(t::tick::Draft, Ticks, 0, 8),
            4 => draft!(
                t::combo::Draft,
                Combos,
                0,
                t::combo::Multipliers::parse("2x/3x").unwrap()
            ),
            5 => draft!(
                t::speed::Draft,
                Speeds,
                0,
                t::speed::Speed::parse("2x/48rows").unwrap()
            ),
            6 => draft!(t::label::Draft, Labels, 48, "Live"),
            7 | 8 => {
                let mut d = t::pause::Draft::capture(s, 0, Scope::Chart).unwrap();
                d.set(
                    if kind == 7 {
                        t::pause::Kind::Stop
                    } else {
                        t::pause::Kind::Delay
                    },
                    48,
                    0.25,
                )
                .unwrap();
                Edit::Pauses(&d).prepare(s, 0, anchor).unwrap()
            }
            9 | 10 => {
                let mut d = t::region::Draft::capture(s, 0, Scope::Chart).unwrap();
                d.set(
                    if kind == 9 {
                        t::region::Kind::Warp
                    } else {
                        t::region::Kind::Fake
                    },
                    48,
                    24,
                )
                .unwrap();
                Edit::Regions(&d).prepare(s, 0, anchor).unwrap()
            }
            _ => unreachable!(),
        }
    }
    #[test]
    fn all_eleven_prospective_clocks_publish_once_and_history_reprojects_source_anchors() {
        for kind in 0..11 {
            for anchor in [
                Anchor::MusicTime(1.375),
                Anchor::Row(-0.25),
                Anchor::Row(144.25),
            ] {
                let mut s = session();
                let bytes = s.serialize().unwrap();
                let revision = s.revision();
                let plan = prepared(&s, kind, anchor);
                assert_eq!(s.serialize().unwrap(), bytes);
                assert_eq!(s.revision(), revision);
                let applied = plan.apply(&mut s).unwrap();
                assert!(applied.changed());
                assert!(applied.position.is_bound_to(&s, 0));
                match anchor {
                    Anchor::MusicTime(time) => assert_eq!(applied.position.music_time(), time),
                    Anchor::Row(row) => assert_eq!(applied.position.row(), row),
                }
                assert!(s.history_changes_timing_edit(false));
                let after = s.serialize().unwrap();
                for redo in [false, true] {
                    let result = Plan::prepare_history(&s, 0, redo, anchor)
                        .unwrap()
                        .apply(&mut s)
                        .unwrap();
                    assert!(result.position.is_bound_to(&s, 0));
                    assert_eq!(
                        s.serialize().unwrap(),
                        if redo { after.clone() } else { bytes.clone() }
                    );
                }
            }
        }
    }
    #[test]
    fn stale_and_invalid_clock_targets_refuse_before_document_or_redo_mutation() {
        let mut s = session();
        let plan = prepared(&s, 0, Anchor::Row(0.25));
        s.set_text(None, "TITLE", "Changed").unwrap();
        let bytes = s.serialize().unwrap();
        let revision = s.revision();
        assert!(plan.apply(&mut s).is_err());
        assert_eq!(s.serialize().unwrap(), bytes);
        assert_eq!(s.revision(), revision);
        let mut d = t::TempoDraft::capture(&s, 0, Scope::Chart).unwrap();
        d.set(0, 240.).unwrap();
        assert!(
            Edit::Tempo(&d)
                .prepare(&s, 0, Anchor::MusicTime(f64::NAN))
                .is_err()
        );
        assert_eq!(s.serialize().unwrap(), bytes);
        s.undo().unwrap();
        let plan = prepared(&s, 0, Anchor::Row(0.25));
        s.redo().unwrap();
        s.undo().unwrap();
        assert!(
            plan.apply(&mut s).is_err(),
            "reused revision after Undo must not reuse a review epoch"
        );
    }
    #[test]
    fn closed_view_history_needs_no_synthetic_chart_cursor() {
        let mut s = session();
        let before = s.serialize().unwrap();
        prepared(&s, 0, Anchor::Row(0.25)).apply(&mut s).unwrap();
        let after = s.serialize().unwrap();
        assert!(s.apply_unfocused_timing_history(false).unwrap());
        assert_eq!(s.serialize().unwrap(), before);
        assert!(s.apply_unfocused_timing_history(true).unwrap());
        assert_eq!(s.serialize().unwrap(), after);
    }
}
