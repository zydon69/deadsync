//! Non-undoable source active-chart state, maintained at committed boundaries.
use super::*;
use crate::{
    attacks::{
        placement::Timeline,
        selection::{Lifecycle, Selected},
    },
    timing_edit::Scope,
};
impl Session {
    /// A chart index is resolved to its stable Session chart identity immediately.
    pub fn select_attack(&mut self, chart: usize, scope: Scope, row: u64) -> Result<(), Error> {
        let id = self
            .chart_ids
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Attack chart unavailable".into()))?
            .clone();
        let timeline = Timeline::capture_scope(self, chart, scope)?;
        if !timeline.markers().iter().any(|event| event.row == row) {
            return Err(Error::InvalidEdit("Attack event unavailable".into()));
        }
        self.attack_selection.insert(id, Selected { scope, row });
        Ok(())
    }
    pub fn selected_attack_row(&self, chart: usize, scope: Scope) -> Option<u64> {
        self.attack_selection
            .get(self.chart_ids.get(chart)?)
            .filter(|selected| selected.scope == scope)
            .map(|selected| selected.row)
    }
    /// Source RemoveActiveChart disposes its LastSelectedAttackEvent.
    pub fn forget_chart_attack(&mut self, chart: usize) {
        if let Some(id) = self.chart_ids.get(chart) {
            self.attack_selection.remove(id);
        }
    }
    pub(crate) fn commit_attack_lifecycle(&mut self, effect: Lifecycle) -> Result<(), Error> {
        if self.chart_ids.get(effect.chart) != Some(&effect.identity) {
            return Err(Error::InvalidEdit(
                "Attack lifecycle chart identity changed".into(),
            ));
        }
        if effect.has_changes() {
            self.execute(
                "Attack event lifetime".into(),
                Change::AttackIdentity(effect),
            )?;
        }
        Ok(())
    }
    pub(super) fn apply_attack_lifecycle(&mut self, effect: &Lifecycle, forward: bool) {
        let mapping = if forward {
            &effect.forward
        } else {
            &effect.backward
        };
        let Some(selected) = self.attack_selection.get_mut(&effect.identity) else {
            return;
        };
        if selected.scope != effect.scope {
            return;
        }
        if let Some(target) = mapping.get(&selected.row) {
            if let Some(row) = target {
                selected.row = *row;
            } else {
                self.attack_selection.remove(&effect.identity);
            }
        }
    }
    pub(super) fn prune_attack_selection(&mut self) {
        let remove: Vec<_> = self
            .attack_selection
            .iter()
            .filter_map(|(id, selected)| {
                let chart = self.chart_ids.iter().position(|candidate| candidate == id);
                let exists = chart
                    .and_then(|chart| Timeline::capture_scope(self, chart, selected.scope).ok())
                    .is_some_and(|timeline| {
                        timeline
                            .markers()
                            .iter()
                            .any(|event| event.row == selected.row)
                    });
                (!exists).then(|| id.clone())
            })
            .collect();
        for id in remove {
            self.attack_selection.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        attacks::{Modifier, inspector::Draft},
        document::{Document, Format},
    };
    fn fixture() -> Session {
        let mut session=Session::new(Document::parse(b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000,0100;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0010,0001;".as_slice(),Format::Ssc).unwrap());
        for chart in 0..2 {
            let mut draft = Draft::capture(&session, chart, Scope::Chart).unwrap();
            draft.add(48, Modifier::default()).unwrap();
            draft
                .add(
                    144,
                    Modifier {
                        name: "Reverse".into(),
                        ..Modifier::default()
                    },
                )
                .unwrap();
            draft.apply(&mut session, chart).unwrap();
        }
        session
    }
    #[test]
    fn selected_object_survives_value_and_order_changing_moves_but_not_delete_undo() {
        let mut session = fixture();
        let scope = Scope::Chart;
        let revision = session.revision();
        let before = session.serialize().unwrap();
        session.select_attack(0, scope, 144).unwrap();
        assert_eq!(session.revision(), revision);
        assert_eq!(session.serialize().unwrap(), before);
        let mut draft = Draft::capture(&session, 0, scope).unwrap();
        let mut value = draft.attacks().events()[1].modifiers[0].clone();
        value.level = 75.;
        draft.set_at_row(1, 0, 144, value).unwrap();
        draft.apply(&mut session, 0).unwrap();
        assert_eq!(session.selected_attack_row(0, scope), Some(144));
        let mut draft = Draft::capture(&session, 0, scope).unwrap();
        assert_eq!(draft.move_to_row(1, 24).unwrap(), 0);
        draft.apply(&mut session, 0).unwrap();
        assert_eq!(session.selected_attack_row(0, scope), Some(24));
        session.undo().unwrap();
        assert_eq!(session.selected_attack_row(0, scope), Some(144));
        session.redo().unwrap();
        assert_eq!(session.selected_attack_row(0, scope), Some(24));
        let mut draft = Draft::capture(&session, 0, scope).unwrap();
        draft.remove(0).unwrap();
        draft.apply(&mut session, 0).unwrap();
        assert_eq!(session.selected_attack_row(0, scope), None);
        session.undo().unwrap();
        assert_eq!(session.selected_attack_row(0, scope), None);
        session.redo().unwrap();
        assert_eq!(session.selected_attack_row(0, scope), None);
    }
    #[test]
    fn same_value_paste_has_new_identity_and_delete_add_does_not_resurrect_selection() {
        let mut session = fixture();
        let scope = Scope::Chart;
        session.select_attack(0, scope, 144).unwrap();
        let mut draft = Draft::capture(&session, 0, scope).unwrap();
        let clip = draft.copy_events(&[1]).unwrap();
        draft.paste_events(&clip, 144).unwrap();
        assert!(!draft.has_changes());
        let before = session.serialize().unwrap();
        let revision = session.revision();
        assert!(draft.apply(&mut session, 0).unwrap());
        assert_eq!(session.serialize().unwrap(), before);
        assert_ne!(session.revision(), revision);
        assert_eq!(session.selected_attack_row(0, scope), None);
        session.select_attack(0, scope, 144).unwrap();
        session.undo().unwrap();
        assert_eq!(session.selected_attack_row(0, scope), None);
        session.redo().unwrap();
        assert_eq!(session.selected_attack_row(0, scope), None);
        session.select_attack(0, scope, 48).unwrap();
        let mut draft = Draft::capture(&session, 0, scope).unwrap();
        let value = draft.attacks().events()[0].modifiers[0].clone();
        draft.remove(0).unwrap();
        draft.add(48, value).unwrap();
        draft.apply(&mut session, 0).unwrap();
        assert_eq!(session.selected_attack_row(0, scope), None);
    }
    #[test]
    fn chart_scope_session_lifetime_and_atomic_refusal_are_independent_of_history() {
        let mut session = fixture();
        let scope = Scope::Chart;
        session.select_attack(0, scope, 144).unwrap();
        session.select_attack(1, scope, 48).unwrap();
        let before = session.serialize().unwrap();
        let revision = session.revision();
        assert!(
            session
                .edit_chart_atomic(0, "Rejected", |staged| {
                    let mut draft = Draft::capture(staged, 0, scope)?;
                    draft.remove(1)?;
                    draft.apply(staged, 0)?;
                    Err(Error::InvalidEdit("Rejected after staging".into()))
                })
                .is_err()
        );
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(session.revision(), revision);
        assert_eq!(session.selected_attack_row(0, scope), Some(144));
        assert_eq!(session.selected_attack_row(1, scope), Some(48));
        let mut stale = Draft::capture(&session, 0, scope).unwrap();
        stale.remove(1).unwrap();
        session.set_text(None, "TITLE", "New title").unwrap();
        assert!(stale.apply(&mut session, 0).is_err());
        assert_eq!(session.selected_attack_row(0, scope), Some(144));
        session.forget_chart_attack(0);
        session.undo().unwrap();
        assert_eq!(session.selected_attack_row(0, scope), None);
        assert_eq!(session.selected_attack_row(1, scope), Some(48));
        let other = Session::new(session.snapshot_document().unwrap());
        assert_eq!(other.selected_attack_row(1, scope), None);
        let duplicate = session.duplicate_chart(1, session.revision()).unwrap();
        assert_eq!(session.selected_attack_row(duplicate, scope), None);
        assert_eq!(session.selected_attack_row(1, scope), Some(48));
        session.undo().unwrap(); // undo duplication, retaining the original chart
        session.remove_chart(1, None).unwrap();
        assert_eq!(session.selected_attack_row(1, scope), None);
        session.undo().unwrap();
        assert_eq!(session.selected_attack_row(1, scope), None);
        // Redo disposes the actual restored chart view/object too. Its GUID can
        // return on the following Undo, but its selected attack must not return.
        session.select_attack(1, scope, 48).unwrap();
        session.redo().unwrap();
        session.undo().unwrap();
        assert_eq!(session.selected_attack_row(1, scope), None);
    }
}
