//! Prepared in-place history migration. Never clones/replays external actions.
use super::{Change, Entry, Session};
use crate::{
    Error,
    document::{Document, media_relocation as media},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};
#[derive(Clone, Copy)]
enum Stack {
    Undo,
    Redo,
}
enum Update {
    Document(Document),
    Value {
        before: Option<Arc<[u8]>>,
        after: Option<Arc<[u8]>>,
    },
    Added {
        before: Option<Arc<[u8]>>,
        after: Option<Arc<[u8]>>,
    },
}
struct Slot {
    stack: Stack,
    entry: usize,
    path: Vec<usize>,
    update: Update,
}
struct Guard {
    identity: Arc<()>,
    next: u64,
    revision: u64,
    content: u64,
    saved: u64,
    undo: usize,
    redo: usize,
}
pub(crate) struct Plan {
    guard: Guard,
    current: Document,
    slots: Vec<Slot>,
    stamps: BTreeMap<u64, u64>,
    next: u64,
    bytes: Vec<u8>,
}
/// The mutable borrow excludes edits between guard validation and publication.
/// Storage can retain this guard while writing its disjoint path/baseline fields.
pub(crate) struct Publication<'a> {
    session: &'a mut Session,
    plan: Plan,
}
impl Plan {
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn relocated_content_token(&self) -> super::ContentToken {
        super::ContentToken {
            identity: Arc::clone(&self.guard.identity),
            revision: self.stamps[&self.guard.content],
        }
    }
    pub(crate) fn prepare(session: &Session, old: &Path, new: &Path) -> Result<Self, Error> {
        if !old.is_absolute()
            || !new.is_absolute()
            || !new
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case(session.document.format.extension()))
        {
            return Err(Error::Unsupported(
                "Relocation requires absolute same-format simfile paths".into(),
            ));
        }
        let current = media::document(&session.document, old, new)?;
        let mut slots = Vec::new();
        for (stack, entries, forward) in [
            (Stack::Undo, &session.undo, false),
            (Stack::Redo, &session.redo, true),
        ] {
            let mut context = session.document.clone();
            for (entry, change) in entries.iter().enumerate().rev() {
                walk(
                    &change.change,
                    &mut context,
                    forward,
                    old,
                    new,
                    stack,
                    entry,
                    &mut Vec::new(),
                    &mut slots,
                )?;
            }
        }
        let mut revisions = BTreeSet::new();
        revisions.extend([session.revision, session.content_revision]);
        if session.saved_revision != u64::MAX {
            revisions.insert(session.saved_revision);
        }
        for entry in session.undo.iter().chain(&session.redo) {
            revisions.extend([
                entry.before_revision,
                entry.after_revision,
                entry.before_content_revision,
                entry.after_content_revision,
            ]);
        }
        let maximum = revisions
            .iter()
            .copied()
            .max()
            .unwrap_or(0)
            .max(session.next_revision);
        let mut next = maximum.checked_add(1).ok_or_else(overflow)?;
        let mut stamps = BTreeMap::new();
        for old in revisions {
            stamps.insert(old, next);
            next = next.checked_add(1).ok_or_else(overflow)?;
        }
        let bytes = session.serialize_document(current.clone())?;
        Ok(Self {
            guard: Guard {
                identity: Arc::clone(&session.identity),
                next: session.next_revision,
                revision: session.revision,
                content: session.content_revision,
                saved: session.saved_revision,
                undo: session.undo.len(),
                redo: session.redo.len(),
            },
            current,
            slots,
            stamps,
            next,
            bytes,
        })
    }
    pub(crate) fn validate(&self, session: &Session) -> Result<(), Error> {
        let g = &self.guard;
        if !Arc::ptr_eq(&g.identity, &session.identity)
            || g.next != session.next_revision
            || g.revision != session.revision
            || g.content != session.content_revision
            || g.saved != session.saved_revision
            || g.undo != session.undo.len()
            || g.redo != session.redo.len()
        {
            return Err(Error::InvalidEdit("Save As relocation became stale".into()));
        }
        Ok(())
    }
    pub(crate) fn publication(self, session: &mut Session) -> Result<Publication<'_>, Error> {
        self.validate(session)?;
        Ok(Publication {
            session,
            plan: self,
        })
    }
}
fn overflow() -> Error {
    Error::InvalidEdit("Editor relocation revision overflow".into())
}
fn walk(
    change: &Change,
    context: &mut Document,
    forward: bool,
    old: &Path,
    new: &Path,
    stack: Stack,
    entry: usize,
    path: &mut Vec<usize>,
    slots: &mut Vec<Slot>,
) -> Result<(), Error> {
    let update = match change {
        Change::Group(changes) => {
            for offset in 0..changes.len() {
                let index = if forward {
                    offset
                } else {
                    changes.len() - 1 - offset
                };
                path.push(index);
                walk(
                    &changes[index],
                    context,
                    forward,
                    old,
                    new,
                    stack,
                    entry,
                    path,
                    slots,
                )?;
                path.pop();
            }
            None
        }
        Change::DocumentSwap(state) => {
            let relocated = media::document(&state.document, old, new)?;
            *context = state.document.clone();
            Some(Update::Document(relocated))
        }
        Change::Value {
            range,
            before,
            after,
        } => {
            let update = media::range_is_media(context, range)
                .then(|| {
                    Ok::<_, Error>(Update::Value {
                        before: media::replacement(context, range, before, old, new)?,
                        after: media::replacement(context, range, after, old, new)?,
                    })
                })
                .transpose()?;
            context.replace(range.clone(), if forward { after } else { before }.clone())?;
            update
        }
        Change::AddedProperty {
            chart,
            name,
            before,
            after,
        } => {
            let update = media::is_media(*chart, &name.to_ascii_uppercase())
                .then(|| {
                    Ok::<_, Error>(Update::Added {
                        before: media::added(before, old, new)?,
                        after: media::added(after, old, new)?,
                    })
                })
                .transpose()?;
            context.set_added_property(
                *chart,
                name,
                if forward { after } else { before }.clone(),
            )?;
            update
        }
        // Exhaustive on purpose: adding a history variant requires an explicit
        // decision, rather than accidentally executing a persisted side effect.
        Change::ReceptorPreferences(_) | Change::NoteColorPreferences(_) | Change::AudioPreferences(_)
        | Change::ProfileCreation { .. }
        | Change::ProfileDeletion { .. }
        | Change::PlayerCount { .. }
        | Change::Notes { .. } => None,
    };
    if let Some(update) = update {
        slots.push(Slot {
            stack,
            entry,
            path: path.clone(),
            update,
        });
    }
    Ok(())
}
impl Publication<'_> {
    #[cfg(test)]
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.plan.bytes
    }
    /// Assignments only: all allocation, path work, validation and serialization
    /// occurred before obtaining this exclusive publication borrow.
    pub(crate) fn commit(self) {
        let Self { session, plan } = self;
        session.document = plan.current;
        for slot in plan.slots {
            let entries = match slot.stack {
                Stack::Undo => &mut session.undo,
                Stack::Redo => &mut session.redo,
            };
            let mut change = &mut entries[slot.entry].change;
            for index in slot.path {
                match change {
                    Change::Group(changes) => change = &mut changes[index],
                    _ => unreachable!("Validated relocation group"),
                }
            }
            match (change, slot.update) {
                (Change::DocumentSwap(state), Update::Document(document)) => {
                    state.document = document
                }
                (
                    Change::Value { before, after, .. },
                    Update::Value {
                        before: b,
                        after: a,
                    },
                )
                | (
                    Change::AddedProperty { before, after, .. },
                    Update::Added {
                        before: b,
                        after: a,
                    },
                ) => {
                    *before = b;
                    *after = a;
                }
                _ => unreachable!("Validated relocation slot"),
            }
        }
        let remap = |stamp: &mut u64| *stamp = plan.stamps[&*stamp];
        for entry in session.undo.iter_mut().chain(&mut session.redo) {
            remap_entry(entry, &remap);
        }
        remap(&mut session.revision);
        remap(&mut session.content_revision);
        if session.saved_revision != u64::MAX {
            remap(&mut session.saved_revision);
        }
        session.next_revision = plan.next;
    }
}
fn remap_entry(entry: &mut Entry, remap: &impl Fn(&mut u64)) {
    remap(&mut entry.before_revision);
    remap(&mut entry.after_revision);
    remap(&mut entry.before_content_revision);
    remap(&mut entry.after_content_revision);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::Format,
        notes::{Note, Position},
    };
    fn session() -> Session {
        Session::new(Document::parse(b"\xef\xbb\xbf#VERSION:0.83;#MUSIC:music.ogg;#Unknown:literal\\;value;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),Format::Ssc).unwrap())
    }
    fn paths() -> (&'static Path, &'static Path) {
        (
            Path::new("/pack/old/song.ssc"),
            Path::new("/pack/new/song.ssc"),
        )
    }
    fn prepare(session: &Session) -> Plan {
        let (old, new) = paths();
        Plan::prepare(session, old, new).unwrap()
    }
    fn music(session: &Session) -> String {
        session.document.text(None, "MUSIC").unwrap()
    }
    #[test]
    fn none_replacement_history_keeps_original_asset_meaning_and_invalidates_old_tokens() {
        let mut session = session();
        let identity = Arc::clone(&session.identity);
        let source = Arc::clone(session.document.source());
        session.set_text(None, "MUSIC", "alternate.ogg").unwrap();
        session.set_text(None, "MUSIC", "redo.ogg").unwrap();
        session.undo().unwrap();
        let old_token = session.content_token();
        let revision_token = session.revision_token();
        let undo = session.undo_label().unwrap().to_owned();
        let redo = session.redo_label().unwrap().to_owned();
        let plan = prepare(&session);
        let publication = plan.publication(&mut session).unwrap();
        assert!(publication.bytes().starts_with(b"\xef\xbb\xbf"));
        publication.commit();
        assert!(Arc::ptr_eq(&identity, &session.identity));
        assert!(Arc::ptr_eq(&source, session.document.source()));
        assert!(!session.accepts_content_token(&old_token));
        assert!(!session.accepts_token(&revision_token));
        assert_eq!(session.undo_label(), Some(undo.as_str()));
        assert_eq!(session.redo_label(), Some(redo.as_str()));
        assert_eq!(music(&session), "../old/alternate.ogg");
        assert!(session.is_dirty());
        session.mark_saved();
        session.undo().unwrap();
        assert_eq!(music(&session), "../old/music.ogg");
        assert!(session.is_dirty());
        session.redo().unwrap();
        assert!(!session.is_dirty());
        session.redo().unwrap();
        assert_eq!(music(&session), "../old/redo.ogg");
        assert!(session.is_dirty());
        assert!(
            String::from_utf8_lossy(&session.serialize().unwrap())
                .contains("#Unknown:literal\\;value;")
        );
    }
    #[test]
    fn structural_redo_uses_its_own_document_ranges_without_replacing_grids_or_guids() {
        let mut session = session();
        session.set_text(None, "MUSIC", "before-swap.ogg").unwrap();
        session
            .edit_notes(
                0,
                "Pending notes",
                [(
                    Position {
                        player: 0,
                        lane: 1,
                        row: 48,
                    },
                    Some(Note::tap()),
                )],
            )
            .unwrap();
        let original_ids = session.chart_ids.clone();
        session.duplicate_chart(0, session.revision()).unwrap();
        let duplicate_ids = session.chart_ids.clone();
        session.grid(1).unwrap();
        session.set_text(None, "MUSIC", "after-swap.ogg").unwrap();
        session.undo().unwrap();
        session.undo().unwrap();
        let grid_ptr = session.loaded_grid(0).unwrap() as *const _;
        let serialized = session.serialize().unwrap();
        prepare(&session)
            .publication(&mut session)
            .unwrap()
            .commit();
        assert_eq!(session.loaded_grid(0).unwrap() as *const _, grid_ptr);
        assert_eq!(session.chart_ids, original_ids);
        let relocated = Document::parse(session.serialize().unwrap(), Format::Ssc).unwrap();
        let previous = Document::parse(serialized, Format::Ssc).unwrap();
        assert_eq!(
            relocated.text(None, "UNKNOWN"),
            previous.text(None, "UNKNOWN")
        );
        session.redo().unwrap();
        assert_eq!(session.chart_ids, duplicate_ids);
        assert!(session.loaded_grid(1).is_some());
        assert_eq!(music(&session), "../old/before-swap.ogg");
        session.redo().unwrap();
        assert_eq!(music(&session), "../old/after-swap.ogg");
        session.undo().unwrap();
        session.undo().unwrap();
        assert_eq!(session.chart_ids, original_ids);
        assert_eq!(
            session.grid(0).unwrap().get(Position {
                player: 0,
                lane: 1,
                row: 48
            }),
            Some(&Note::tap())
        );
    }
    #[test]
    fn added_media_history_preserves_absence_and_redo_paths() {
        let mut session = session();
        session
            .set_text(None, "BANNER", "images/banner.png")
            .unwrap();
        session.undo().unwrap();
        prepare(&session)
            .publication(&mut session)
            .unwrap()
            .commit();
        assert_eq!(session.document.text(None, "BANNER"), None);
        session.redo().unwrap();
        assert_eq!(
            session.document.text(None, "BANNER").as_deref(),
            Some("../old/images/banner.png")
        );
        session.undo().unwrap();
        assert_eq!(session.document.text(None, "BANNER"), None);
    }
    #[test]
    fn edit_then_undo_rejects_plan_even_when_visible_revision_returns() {
        let mut session = session();
        let plan = prepare(&session);
        let revision = session.revision();
        let bytes = session.serialize().unwrap();
        session.set_text(None, "TITLE", "New redo branch").unwrap();
        session.undo().unwrap();
        assert_eq!(session.revision(), revision);
        assert_eq!(session.serialize().unwrap(), bytes);
        assert!(plan.publication(&mut session).is_err());
        assert_eq!(session.redo_label(), Some("Change TITLE"));
    }
    #[test]
    fn preparation_and_publication_never_replay_or_replace_external_preference_receipts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preferences.json");
        let file = crate::audio_preferences::File::open(&path).unwrap();
        let before = file.audio();
        let mut after = before;
        after.set_music_rate(1.1).unwrap();
        let mut session = session();
        session
            .set_audio_preferences(before, after, Some(&file))
            .unwrap();
        let receipt = match &session.undo[0].change {
            Change::AudioPreferences(change) => &**change as *const _,
            _ => unreachable!(),
        };
        let external = b"changed by another process";
        std::fs::write(&path, external).unwrap();
        prepare(&session)
            .publication(&mut session)
            .unwrap()
            .commit();
        let after_receipt = match &session.undo[0].change {
            Change::AudioPreferences(change) => &**change as *const _,
            _ => unreachable!(),
        };
        assert_eq!(receipt, after_receipt);
        assert_eq!(std::fs::read(path).unwrap(), external);
        assert!(
            !session.is_dirty(),
            "preference-only timeline remains song-clean"
        );
    }
    #[test]
    fn overflow_and_incompatible_paths_fail_before_mutation() {
        let mut session = session();
        let bytes = session.serialize().unwrap();
        let revision = session.revision();
        assert!(Plan::prepare(&session, Path::new("relative.ssc"), paths().1).is_err());
        assert!(Plan::prepare(&session, paths().0, Path::new("/pack/new/song.sm")).is_err());
        session.next_revision = u64::MAX;
        assert!(Plan::prepare(&session, paths().0, paths().1).is_err());
        assert_eq!(session.serialize().unwrap(), bytes);
        assert_eq!(session.revision(), revision);
    }
    #[test]
    fn profile_deletion_receipt_and_disk_state_are_untouched_by_migration() {
        let directory = tempfile::tempdir().unwrap();
        let profile = crate::generation::profiles::Profile::new_pattern(
            "SaveAs receipt",
            crate::generation::config::PatternConfig::sixteenths(),
        )
        .unwrap();
        let file = profile.export_new(directory.path()).unwrap();
        let path = file.path().to_owned();
        let mut session = session();
        session.delete_profile(&file).unwrap();
        assert!(!path.exists());
        let receipt = &session.undo.last().unwrap().change as *const _;
        prepare(&session)
            .publication(&mut session)
            .unwrap()
            .commit();
        assert_eq!(receipt, &session.undo.last().unwrap().change as *const _);
        assert!(!path.exists());
        session.undo().unwrap();
        assert!(path.exists());
        session.redo().unwrap();
        assert!(!path.exists());
    }
    #[test]
    fn stepf2_attributes_and_pending_peer_grid_survive_staged_output_and_publication() {
        let source=b"#VERSION:0.83;#MUSIC:music.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:pump-double;#NOTES:{X|s|1|%}[7]000000000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;";
        let mut session = Session::new(Document::parse(source.as_slice(), Format::Ssc).unwrap());
        let coop = session.grid(0).unwrap().clone();
        assert!(coop.uses_stepf2());
        session
            .edit_notes(
                1,
                "Pending peer",
                [(
                    Position {
                        player: 0,
                        lane: 2,
                        row: 96,
                    },
                    Some(Note::tap()),
                )],
            )
            .unwrap();
        session.set_text(None, "TITLE", "Redo title").unwrap();
        session.undo().unwrap();
        let ids = session.chart_ids.clone();
        let peer = session.grid(1).unwrap().clone();
        let plan = prepare(&session);
        let bytes = plan.bytes.clone();
        assert!(String::from_utf8_lossy(&bytes).contains("{X|s|1|%}[7]"));
        let publication = plan.publication(&mut session).unwrap();
        assert_eq!(publication.bytes(), bytes);
        publication.commit();
        assert_eq!(session.chart_ids, ids);
        assert_eq!(session.grid(0).unwrap(), &coop);
        assert_eq!(session.grid(1).unwrap(), &peer);
        assert_eq!(session.serialize().unwrap(), bytes);
        session.redo().unwrap();
        session.undo().unwrap();
        assert_eq!(session.grid(1).unwrap(), &peer);
    }
    #[test]
    fn chart_music_none_replacement_preserves_override_and_song_fallback_across_history() {
        let mut session=Session::new(Document::parse(b"#VERSION:0.83;#MUSIC:song.ogg;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#MUSIC:chart.ogg;#NOTES:0000;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;".as_slice(),Format::Ssc).unwrap());
        session.set_text(Some(0), "MUSIC", "edited.ogg").unwrap();
        let ids = session.chart_ids.clone();
        prepare(&session)
            .publication(&mut session)
            .unwrap()
            .commit();
        assert_eq!(
            session.document.text(Some(0), "MUSIC").as_deref(),
            Some("../old/edited.ogg")
        );
        assert_eq!(session.document.text(Some(1), "MUSIC"), None);
        assert_eq!(
            session.document.text(None, "MUSIC").as_deref(),
            Some("../old/song.ogg")
        );
        session.undo().unwrap();
        assert_eq!(
            session.document.text(Some(0), "MUSIC").as_deref(),
            Some("../old/chart.ogg")
        );
        session.redo().unwrap();
        assert_eq!(
            session.document.text(Some(0), "MUSIC").as_deref(),
            Some("../old/edited.ogg")
        );
        assert_eq!(session.chart_ids, ids);
    }
    #[test]
    fn chart_music_added_override_and_structural_redo_keep_peer_assets_and_loaded_grids() {
        let mut session = session();
        session.set_text(Some(0), "MUSIC", "added.ogg").unwrap();
        session.duplicate_chart(0, session.revision()).unwrap();
        session.grid(0).unwrap();
        session.grid(1).unwrap();
        session.set_text(Some(1), "MUSIC", "peer.ogg").unwrap();
        session.undo().unwrap();
        session.undo().unwrap();
        // Structural undo restores its original lazy grid state. Load the
        // current grid before checking relocation keeps that allocation.
        session.grid(0).unwrap();
        let ids = session.chart_ids.clone();
        let grid = session.loaded_grid(0).unwrap() as *const _;
        prepare(&session)
            .publication(&mut session)
            .unwrap()
            .commit();
        assert_eq!(session.loaded_grid(0).unwrap() as *const _, grid);
        assert_eq!(session.chart_ids, ids);
        assert_eq!(
            session.document.text(Some(0), "MUSIC").as_deref(),
            Some("../old/added.ogg")
        );
        session.undo().unwrap();
        assert_eq!(session.document.text(Some(0), "MUSIC"), None);
        session.redo().unwrap();
        session.redo().unwrap();
        assert_eq!(
            session.document.text(Some(0), "MUSIC").as_deref(),
            Some("../old/added.ogg")
        );
        assert_eq!(
            session.document.text(Some(1), "MUSIC").as_deref(),
            Some("../old/added.ogg")
        );
        session.redo().unwrap();
        assert_eq!(
            session.document.text(Some(1), "MUSIC").as_deref(),
            Some("../old/peer.ogg")
        );
    }
}
