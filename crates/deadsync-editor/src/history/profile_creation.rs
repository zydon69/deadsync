//! Source inline New is one file+reference action, independent of pending UI fields.
use super::{Change, RevisionToken, Session, song_presentation};
use crate::{
    Error,
    generation::{
        metadata::{ChartData, Guid},
        profiles::{Kind, Loaded, Profile},
    },
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Target {
    Chart(usize),
    Marker { chart: usize, row: u64 },
}
pub struct Plan {
    token: RevisionToken,
    target: Target,
    kind: Kind,
}
pub struct Committed {
    before: RevisionToken,
    after: RevisionToken,
    target: Target,
    kind: Kind,
    guid: Guid,
    dependency: (PathBuf, Arc<[u8]>),
    application: song_presentation::Application,
}
impl Plan {
    pub(crate) fn chart(session: &Session, chart: usize) -> Result<Self, Error> {
        ChartData::existing_or_initial(session.document(), chart)?;
        Ok(Self {
            token: session.revision_token(),
            target: Target::Chart(chart),
            kind: Kind::Expressed,
        })
    }
    pub(crate) fn marker(
        session: &Session,
        chart: usize,
        row: u64,
        kind: Kind,
    ) -> Result<Self, Error> {
        let data = ChartData::read(session.document(), chart)?
            .ok_or_else(|| invalid("Missing marker metadata"))?;
        if !data.patterns.as_ref().is_some_and(|p| p.contains_key(&row)) {
            return Err(invalid("Inspected marker no longer exists"));
        }
        Ok(Self {
            token: session.revision_token(),
            target: Target::Marker { chart, row },
            kind,
        })
    }
    pub fn publish(
        &self,
        session: &mut Session,
        directory: &Path,
        profile: &Profile,
    ) -> Result<Committed, Error> {
        if !session.accepts_token(&self.token) || profile.config().kind() != self.kind {
            return Err(invalid(
                "New profile binding belongs to another document, revision or kind",
            ));
        }
        if !session.allows_external_edits {
            return Err(invalid("Staged chart edits cannot create profile files"));
        }
        let loaded = Loaded::directory(directory)?;
        if loaded
            .profiles
            .contains_key(&(self.kind, profile.guid().clone()))
        {
            return Err(invalid("Profile identity already exists"));
        }
        session.retry_profile_durability()?;
        let chart = match self.target {
            Target::Chart(chart) | Target::Marker { chart, .. } => chart,
        };
        let mut data = ChartData::existing_or_initial(session.document(), chart)?;
        match (self.target, self.kind) {
            (_, Kind::Expressed) => data.expressed_chart_config = profile.guid().clone(),
            (Target::Marker { row, .. }, kind) => {
                let marker = data
                    .patterns
                    .as_mut()
                    .and_then(|p| p.get_mut(&row))
                    .ok_or_else(|| invalid("Inspected marker no longer exists"))?;
                if kind == Kind::Pattern {
                    marker.pattern_config_guid = profile.guid().clone();
                } else {
                    marker.performed_chart_config_guid = profile.guid().clone();
                }
            }
            _ => {
                return Err(invalid(
                    "A chart-wide binding requires an expressed profile",
                ));
            }
        }
        let mut staged = Session::staged(session.document.clone());
        staged.chart_ids = session.chart_ids.clone();
        staged.song_timing = session.song_timing.clone();
        data.save(&mut staged, chart)?;
        let mut changes: Vec<_> = staged.undo.into_iter().map(|entry| entry.change).collect();
        // Last forward / first backward: disk collision or CAS refusal rolls
        // back metadata and leaves both history stacks untouched.
        changes.push(Change::ProfileCreation {
            profile: Box::new(profile.clone()),
            directory: directory.to_owned(),
            file: None,
            removed: None,
        });
        let before = session.revision_token();
        let content = session.content_token();
        let charts = session.chart_ids.clone();
        session.execute(
            "Create and assign generation profile".into(),
            Change::Group(changes),
        )?;
        let dependency = match &session.undo.last().expect("just committed").change {
            Change::Group(changes) => match changes.last().expect("creation appended") {
                Change::ProfileCreation {
                    file: Some(file), ..
                } => file.snapshot(),
                _ => unreachable!("typed creation group"),
            },
            _ => unreachable!("typed creation group"),
        };
        Ok(Committed {
            before,
            dependency,
            after: session.revision_token(),
            target: self.target,
            kind: self.kind,
            guid: profile.guid().clone(),
            application: song_presentation::preserved_authoring(content, charts, session),
        })
    }
}
impl Committed {
    pub fn kind(&self) -> Kind {
        self.kind
    }
    pub(crate) fn guid(&self) -> &Guid {
        &self.guid
    }
    pub(crate) fn dependency(&self) -> &(PathBuf, Arc<[u8]>) {
        &self.dependency
    }
    pub fn application(&self) -> &crate::song_properties::Application {
        &self.application
    }
    pub(crate) fn rebind(
        &self,
        session: &Session,
        token: &RevisionToken,
        chart: usize,
        row: Option<u64>,
    ) -> Result<Guid, Error> {
        let target = match row {
            Some(row) => Target::Marker { chart, row },
            None => Target::Chart(chart),
        };
        if self.target != target
            || !session.accepts_token(&self.after)
            || !Arc::ptr_eq(&self.before.identity, &token.identity)
            || self.before.revision != token.revision
        {
            return Err(invalid(
                "New profile receipt does not authenticate this parent draft",
            ));
        }
        Ok(self.guid.clone())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::{Document, Format},
        generation::marker_edit::Draft,
    };
    fn session(format: Format) -> Session {
        let bytes: &[u8] = if format == Format::Sm {
            b"#BPMS:0=120;#NOTES:dance-single:source:Edit:9:0,0,0,0,0:1000;"
        } else {
            b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000;"
        };
        let mut session = Session::new(Document::parse(bytes, format).unwrap());
        crate::quick_add::Plan::prepare(&session, 0, crate::quick_add::Kind::Pattern, 96.)
            .unwrap()
            .apply(&mut session, 0)
            .unwrap();
        session
    }
    #[test]
    fn new_binding_is_one_history_action_preserves_other_pending_fields_and_undo_redo_exact() {
        for format in [Format::Sm, Format::Ssc] {
            for kind in [Kind::Pattern, Kind::Performed, Kind::Expressed] {
                let root = tempfile::tempdir().unwrap();
                let mut session = session(format);
                // QuickAdd's real default length is already192. Establish a
                // distinct committed baseline so192 is a genuine pending edit.
                let mut initial = Draft::at_cursor(&session, 0, 96).unwrap();
                initial.definition.length = 48;
                assert!(
                    initial.apply(&mut session, 0).unwrap(),
                    "{format:?}/{kind:?}: fixture baseline"
                );
                let before = session.serialize().unwrap();
                let mut draft = Draft::at_cursor(&session, 0, 96).unwrap();
                draft.start = 144;
                draft.definition.length = 192;
                draft
                    .definition
                    .extra
                    .insert("Opaque".into(), serde_json::json!({"kept":true}));
                let plan = draft.new_profile_plan(&session, kind).unwrap();
                let profile = Profile::new_editor_default(kind).unwrap();
                let file = root.path().join(profile.filename());
                let committed = plan.publish(&mut session, root.path(), &profile).unwrap();
                draft
                    .after_inline_profile_creation(&session, &committed)
                    .unwrap();
                assert!(file.is_file());
                assert_eq!(draft.start, 144);
                assert_eq!(draft.definition.length, 192);
                assert_eq!(draft.definition.extra["Opaque"]["kept"], true);
                assert!(draft.has_changes());
                let data = ChartData::read(session.document(), 0).unwrap().unwrap();
                let marker = &data.patterns.as_ref().unwrap()[&96];
                let id = match kind {
                    Kind::Pattern => &marker.pattern_config_guid,
                    Kind::Performed => &marker.performed_chart_config_guid,
                    Kind::Expressed => &data.expressed_chart_config,
                };
                assert_eq!(id, profile.guid());
                assert_eq!(
                    marker.length, 48,
                    "{format:?}/{kind:?}: New must not publish pending length"
                );
                assert!(
                    !marker.extra.contains_key("Opaque"),
                    "{format:?}/{kind:?}: New must not publish pending metadata"
                );
                assert!(!data.patterns.unwrap().contains_key(&144));
                let published = session.serialize().unwrap();
                let profile_bytes = std::fs::read(&file).unwrap();
                // Apply the actual receipt-rebound parent, not merely a fresh
                // reconstruction. Its unrelated start/length/opaque fields must
                // remain pending and commit as the next independent action.
                assert!(
                    draft.apply(&mut session, 0).unwrap(),
                    "{format:?}/{kind:?}: Apply rebound parent"
                );
                let applied = ChartData::read(session.document(), 0).unwrap().unwrap();
                let moved = &applied.patterns.as_ref().unwrap()[&144];
                assert_eq!(moved.length, 192);
                assert_eq!(moved.extra["Opaque"]["kept"], true);
                let bound = match kind {
                    Kind::Pattern => &moved.pattern_config_guid,
                    Kind::Performed => &moved.performed_chart_config_guid,
                    Kind::Expressed => &applied.expressed_chart_config,
                };
                assert_eq!(bound, profile.guid());
                assert!(!applied.patterns.as_ref().unwrap().contains_key(&96));
                assert_eq!(std::fs::read(&file).unwrap(), profile_bytes);
                assert!(session.undo().unwrap());
                assert_eq!(
                    session.serialize().unwrap(),
                    published,
                    "{format:?}/{kind:?}: Undo pending fields"
                );
                assert!(
                    file.is_file(),
                    "Undo pending fields must keep the separately created profile"
                );
                assert!(session.undo().unwrap());
                assert!(!file.exists());
                assert_eq!(session.serialize().unwrap(), before);
                assert!(session.redo().unwrap());
                assert_eq!(session.serialize().unwrap(), published);
                assert_eq!(std::fs::read(&file).unwrap(), profile_bytes);
                // A later ordinary edit uses the restored baseline length48.
                let mut fresh = Draft::at_cursor(&session, 0, 96).unwrap();
                assert_eq!(
                    fresh.definition.length, 48,
                    "{format:?}/{kind:?}: Redo creation baseline"
                );
                fresh.definition.length = 192;
                assert!(fresh.has_changes());
                assert!(
                    fresh.apply(&mut session, 0).unwrap(),
                    "{format:?}/{kind:?}: later ordinary Apply"
                );
                assert!(session.undo().unwrap());
                assert_eq!(session.serialize().unwrap(), published);
            }
        }
    }
    #[test]
    fn creation_collision_and_undo_cas_refuse_without_partial_reference_or_history() {
        let root = tempfile::tempdir().unwrap();
        let mut session = session(Format::Ssc);
        let draft = Draft::at_cursor(&session, 0, 96).unwrap();
        let plan = draft.new_profile_plan(&session, Kind::Pattern).unwrap();
        let profile = Profile::new_editor_default(Kind::Pattern).unwrap();
        let path = root.path().join(profile.filename());
        let before = session.serialize().unwrap();
        let revision = session.revision();
        std::fs::write(&path, b"external").unwrap();
        assert!(plan.publish(&mut session, root.path(), &profile).is_err());
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(session.revision(), revision);
        assert_eq!(std::fs::read(&path).unwrap(), b"external");
        std::fs::remove_file(&path).unwrap();
        plan.publish(&mut session, root.path(), &profile).unwrap();
        let published = session.serialize().unwrap();
        let revision = session.revision();
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"changed externally").unwrap();
        assert!(session.undo().is_err());
        assert_eq!(session.serialize().unwrap(), published);
        assert_eq!(session.revision(), revision);
        assert_eq!(std::fs::read(&path).unwrap(), b"changed externally");
        std::fs::write(&path, bytes).unwrap();
        assert!(session.undo().unwrap());
        assert_eq!(session.serialize().unwrap(), before);
    }
    #[test]
    fn chart_draft_binding_receipt_preserves_pending_fields_and_stale_foreign_plans_refuse() {
        let root = tempfile::tempdir().unwrap();
        let mut session = session(Format::Ssc);
        let mut draft = crate::chart_properties::Draft::capture(&session, 0).unwrap();
        draft.set_music_offset(0.125).unwrap();
        let plan = draft.new_profile_plan(&session).unwrap();
        let profile = Profile::new_editor_default(Kind::Expressed).unwrap();
        let mut other =
            Session::new(Document::parse(session.serialize().unwrap(), Format::Ssc).unwrap());
        assert!(plan.publish(&mut other, root.path(), &profile).is_err());
        let committed = plan.publish(&mut session, root.path(), &profile).unwrap();
        draft
            .after_inline_profile_creation(&session, &committed)
            .unwrap();
        assert_eq!(draft.expression(), profile.guid());
        assert!(draft.has_changes());
        assert!(
            plan.publish(
                &mut session,
                root.path(),
                &Profile::new_editor_default(Kind::Expressed).unwrap()
            )
            .is_err()
        );
        assert!(draft.apply(&mut session, 0).unwrap());
        assert!(session.undo().unwrap());
        assert_eq!(
            crate::chart_properties::Draft::capture(&session, 0)
                .unwrap()
                .expression(),
            profile.guid()
        );
    }
    #[test]
    fn replacing_one_pending_profile_dependency_keeps_other_exact_byte_guards() {
        let root = tempfile::tempdir().unwrap();
        let mut session = session(Format::Ssc);
        let pattern = Profile::new_editor_default(Kind::Pattern).unwrap();
        let performed = Profile::new_editor_default(Kind::Performed).unwrap();
        let old = pattern.export_new(root.path()).unwrap();
        let other = performed.export_new(root.path()).unwrap();
        let catalog =
            crate::generation::profile_catalog::Task::capture(&session, 0, Some(root.path()))
                .unwrap()
                .run(&std::sync::atomic::AtomicBool::new(false))
                .unwrap();
        let mut draft = Draft::at_cursor(&session, 0, 96).unwrap();
        draft
            .select_profiles(&session, 0, &catalog, pattern.guid(), performed.guid())
            .unwrap();
        draft.definition.length = 333;
        let next = Profile::new_editor_default(Kind::Pattern).unwrap();
        let plan = draft.new_profile_plan(&session, Kind::Pattern).unwrap();
        let committed = plan.publish(&mut session, root.path(), &next).unwrap();
        draft
            .after_inline_profile_creation(&session, &committed)
            .unwrap();
        // Obsolete target dependency is no longer part of the pending choice.
        std::fs::remove_file(old.path()).unwrap();
        let bytes = std::fs::read(other.path()).unwrap();
        std::fs::write(other.path(), b"external").unwrap();
        let before = session.serialize().unwrap();
        let revision = session.revision();
        assert!(draft.apply(&mut session, 0).is_err());
        assert_eq!(session.serialize().unwrap(), before);
        assert_eq!(session.revision(), revision);
        std::fs::write(other.path(), bytes).unwrap();
        assert!(draft.apply(&mut session, 0).unwrap());
        let data = ChartData::read(session.document(), 0).unwrap().unwrap();
        assert_eq!(
            data.patterns.as_ref().unwrap()[&96].performed_chart_config_guid,
            *performed.guid()
        );
        assert_eq!(
            data.patterns.as_ref().unwrap()[&96].pattern_config_guid,
            *next.guid()
        );
    }
}
