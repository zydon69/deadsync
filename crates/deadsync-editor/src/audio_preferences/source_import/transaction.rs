//! Leaf ownership for one reviewed global preference transaction.
//! Objects merge recursively; arrays and scalar values are atomic leaves.
use super::{Snapshot, invalid};
use crate::Error;
use serde_json::{Map, Value};
use std::sync::Arc;
#[derive(Clone, Debug)]
struct Leaf {
    path: Vec<String>,
    before: Option<Value>,
    after: Option<Value>,
}
#[derive(Clone, Debug)]
pub struct Patch {
    leaves: Arc<[Leaf]>,
    empty_before: Arc<[Vec<String>]>,
    empty_after: Arc<[Vec<String>]>,
}
impl Patch {
    pub fn between(before: &Map<String, Value>, after: &Map<String, Value>) -> Self {
        fn visit(
            path: &mut Vec<String>,
            before: Option<&Value>,
            after: Option<&Value>,
            out: &mut Vec<Leaf>,
        ) {
            if before == after {
                return;
            }
            let left = before.and_then(Value::as_object);
            let right = after.and_then(Value::as_object);
            // Object creation/deletion owns its leaves, allowing independent unknown
            // fields to be added to an archive without being erased by Undo.
            if (left.is_some() || before.is_none())
                && (right.is_some() || after.is_none())
                && (left.is_some() || right.is_some())
            {
                let keys = left
                    .into_iter()
                    .flat_map(|v| v.keys())
                    .chain(right.into_iter().flat_map(|v| v.keys()))
                    .collect::<std::collections::BTreeSet<_>>();
                for key in keys {
                    path.push(key.clone());
                    visit(
                        path,
                        left.and_then(|m| m.get(key)),
                        right.and_then(|m| m.get(key)),
                        out,
                    );
                    path.pop();
                }
            } else {
                out.push(Leaf {
                    path: path.clone(),
                    before: before.cloned(),
                    after: after.cloned(),
                });
            }
        }
        let mut leaves = Vec::new();
        let keys = before
            .keys()
            .chain(after.keys())
            .collect::<std::collections::BTreeSet<_>>();
        for key in keys {
            visit(
                &mut vec![key.clone()],
                before.get(key),
                after.get(key),
                &mut leaves,
            );
        }
        fn empty_paths(root: &Map<String, Value>) -> Arc<[Vec<String>]> {
            fn collect(
                root: &Map<String, Value>,
                path: &mut Vec<String>,
                out: &mut Vec<Vec<String>>,
            ) {
                for (key, value) in root {
                    if let Value::Object(child) = value {
                        path.push(key.clone());
                        if child.is_empty() {
                            out.push(path.clone());
                        } else {
                            collect(child, path, out);
                        }
                        path.pop();
                    }
                }
            }
            let mut paths = Vec::new();
            collect(root, &mut Vec::new(), &mut paths);
            paths.into()
        }
        let left = empty_paths(before);
        let right = empty_paths(after);
        let left_set = left.iter().collect::<std::collections::BTreeSet<_>>();
        let right_set = right.iter().collect::<std::collections::BTreeSet<_>>();
        let empty_before = left
            .iter()
            .filter(|p| !right_set.contains(p))
            .cloned()
            .collect::<Vec<_>>()
            .into();
        let empty_after = right
            .iter()
            .filter(|p| !left_set.contains(p))
            .cloned()
            .collect::<Vec<_>>()
            .into();
        Self {
            leaves: leaves.into(),
            empty_before,
            empty_after,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty() && self.empty_before == self.empty_after
    }
    /// Apply already-authenticated live overrides onto a freshly merged disk
    /// projection. Unowned disk metadata remains visible in the runtime File.
    fn overlay(&self, current: &Map<String, Value>) -> Result<Map<String, Value>, Error> {
        fn value<'a>(
            root: &'a Map<String, Value>,
            path: &[String],
        ) -> Result<Option<&'a Value>, Error> {
            let mut root = root;
            for (i, key) in path.iter().enumerate() {
                let item = root.get(key);
                if i + 1 == path.len() {
                    return Ok(item);
                }
                match item {
                    Some(Value::Object(map)) => root = map,
                    None => return Ok(None),
                    _ => {
                        return Err(invalid(
                            "Concurrent preference object conflicts with live override",
                        ));
                    }
                }
            }
            Ok(None)
        }
        let mut leaves = self.leaves.to_vec();
        for leaf in &mut leaves {
            leaf.before = value(current, &leaf.path)?.cloned();
        }
        let mut empty_before = Vec::new();
        for path in self.empty_before.iter() {
            match value(current, path)? {
                Some(Value::Object(_)) => empty_before.push(path.clone()),
                None => {}
                _ => {
                    return Err(invalid(
                        "Concurrent preference object conflicts with live override",
                    ));
                }
            }
        }
        Self {
            leaves: leaves.into(),
            empty_before: empty_before.into(),
            empty_after: Arc::clone(&self.empty_after),
        }
        .apply(current, true)
    }
    pub fn apply(
        &self,
        current: &Map<String, Value>,
        forward: bool,
    ) -> Result<Map<String, Value>, Error> {
        fn get<'a>(
            root: &'a Map<String, Value>,
            path: &[String],
        ) -> Result<Option<&'a Value>, Error> {
            let mut map = root;
            for (i, key) in path.iter().enumerate() {
                let value = map.get(key);
                if i + 1 == path.len() {
                    return Ok(value);
                }
                match value {
                    None => return Ok(None),
                    Some(Value::Object(next)) => map = next,
                    _ => return Err(invalid("Preference object changed since review")),
                }
            }
            Ok(None)
        }
        let (expected_empty, target_empty) = if forward {
            (&self.empty_before, &self.empty_after)
        } else {
            (&self.empty_after, &self.empty_before)
        };
        let expected_set = expected_empty
            .iter()
            .collect::<std::collections::BTreeSet<_>>();
        for path in expected_empty.iter().chain(target_empty.iter()) {
            let value = get(current, path)?;
            if value.is_some_and(|v| !v.is_object())
                || (expected_set.contains(path) && value.is_none())
            {
                return Err(invalid("Preference object changed since review"));
            }
        }
        // Validate every owned leaf before changing anything, including same-value
        // edit/Undo invalidation provided separately by the Session token.
        for leaf in self.leaves.iter() {
            let expected = if forward { &leaf.before } else { &leaf.after };
            if get(current, &leaf.path)? != expected.as_ref() {
                return Err(invalid(format!(
                    "Preference changed since review: {}",
                    leaf.path.join(".")
                )));
            }
        }
        fn set(map: &mut Map<String, Value>, path: &[String], value: Option<Value>) {
            if path.len() == 1 {
                if let Some(value) = value {
                    map.insert(path[0].clone(), value);
                } else {
                    map.remove(&path[0]);
                }
                return;
            }
            if !map.contains_key(&path[0]) {
                map.insert(path[0].clone(), Value::Object(Map::new()));
            }
            let child = map
                .get_mut(&path[0])
                .and_then(Value::as_object_mut)
                .expect("validated object path");
            set(child, &path[1..], value);
            if child.is_empty() {
                map.remove(&path[0]);
            }
        }
        let mut target = current.clone();
        for leaf in self.leaves.iter() {
            set(
                &mut target,
                &leaf.path,
                if forward {
                    leaf.after.clone()
                } else {
                    leaf.before.clone()
                },
            );
        }
        // Preserve explicitly empty unknown objects without claiming concurrently
        // added child leaves. Scalar conflicts still fail in the parent lookup.
        let (remove, restore) = if forward {
            (&self.empty_before, &self.empty_after)
        } else {
            (&self.empty_after, &self.empty_before)
        };
        let restore_set = restore.iter().collect::<std::collections::BTreeSet<_>>();
        for path in remove.iter() {
            if !restore_set.contains(path)
                && get(&target, path)?.is_some_and(|v| v.as_object().is_some_and(Map::is_empty))
            {
                set(&mut target, path, None);
            }
        }
        for path in restore.iter() {
            if get(&target, path)?.is_none() {
                set(&mut target, path, Some(Value::Object(Map::new())));
            }
        }
        Ok(target)
    }
}
/// One history entry owns separate live and persisted leaves: unsaved live
/// preferences present before import must remain unsaved after Undo.
pub(crate) struct Change {
    runtime: Patch,
    persisted: Patch,
    path: std::path::PathBuf,
    persist: bool,
}
impl std::fmt::Debug for Change {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalSourcePreferences")
            .field("path", &self.path)
            .field("persist", &self.persist)
            .finish_non_exhaustive()
    }
}
impl Change {
    pub(crate) fn is_empty(&self) -> bool {
        self.runtime.is_empty() && (!self.persist || self.persisted.is_empty())
    }
    pub(crate) fn prepare(before: &Snapshot, after: &Snapshot) -> Result<Self, Error> {
        let live_before = before.file.projected_root()?;
        let live_after = after.file.projected_root()?;
        let mut disk_before = before.file.as_ref().clone();
        disk_before.settings = disk_before.written.clone();
        Ok(Self {
            runtime: Patch::between(&live_before, &live_after),
            persisted: Patch::between(&disk_before.projected_root()?, &live_after),
            path: before.file.path.clone(),
            persist: !before.file.path.as_os_str().is_empty(),
        })
    }
    pub(crate) fn apply(&self, current: &Snapshot, forward: bool) -> Result<Update, Error> {
        if current.file.path != self.path {
            return Err(invalid("Application preference target changed"));
        }
        // Runtime CAS happens before opening or writing the persisted preference
        // file. The Session additionally authenticates the current publication.
        let runtime_root = self
            .runtime
            .apply(&current.file.projected_root()?, forward)?;
        let mut baseline = current.file.as_ref().clone();
        baseline.settings = baseline.written.clone();
        let live_overrides = Patch::between(&baseline.projected_root()?, &runtime_root);
        let (mut published, live) = if self.persist {
            let fresh = super::File::open(&self.path)?;
            let disk_root = self.persisted.apply(&fresh.projected_root()?, forward)?;
            let merged_live = live_overrides.overlay(&disk_root)?;
            // Parse all final runtime settings before publication, including preserved
            // concurrently-added archive fields; never discover a typed error after IO.
            let live = super::File::from_root(&self.path, fresh.expected.clone(), merged_live)?;
            let mut disk = super::File::from_root(&self.path, fresh.expected.clone(), disk_root)?;
            disk.original = fresh.original;
            disk.written = fresh.written;
            disk.save()?;
            (disk, live)
        } else {
            // With no persisted file, the projected root is the publication
            // baseline too. Keeping the previous root with new typed settings
            // loses explicitly imported default-valued fields and makes a
            // subsequent Undo conflict with our own successful import.
            let live =
                super::File::from_root(&self.path, current.file.expected.clone(), runtime_root)?;
            (live.clone(), live)
        };
        published.settings = live.settings;
        Ok(Update {
            snapshot: Snapshot::capture(&published),
        })
    }
}
/// Prepared publication is boxed in Session pending updates; its raw archive
/// and full File settings remain shared through the opaque Snapshot.
#[derive(Clone, Debug)]
pub struct Update {
    pub(crate) snapshot: Snapshot,
}
impl Update {
    pub fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_overrides_keep_new_disk_archive_metadata_and_unsaved_values() {
        let baseline = object(serde_json::json!({"Volume":0.5,"Local":1}));
        let runtime = object(serde_json::json!({"Volume":0.7,"Local":2,"Archive":{"Raw":"x"}}));
        let disk = object(
            serde_json::json!({"Volume":0.7,"Local":1,"Archive":{"Raw":"x","Vendor":8},"DiskVendor":9}),
        );
        let merged = Patch::between(&baseline, &runtime).overlay(&disk).unwrap();
        assert_eq!(merged["Local"], 2);
        assert_eq!(merged["Archive"]["Vendor"], 8);
        assert_eq!(merged["DiskVendor"], 9);
    }
    #[test]
    fn an_empty_archive_remains_mergeable_without_losing_its_original_shape() {
        let before = object(serde_json::json!({"Archive":{}}));
        let after = object(serde_json::json!({"Archive":{"Raw":"x"}}));
        let patch = Patch::between(&before, &after);
        assert_eq!(patch.apply(&after, false).unwrap(), before);
        let mut current = after.clone();
        current["Archive"]["Vendor"] = Value::from(2);
        assert_eq!(
            patch.apply(&current, false).unwrap()["Archive"],
            serde_json::json!({"Vendor":2})
        );
    }
    fn object(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }
    #[test]
    fn unknown_archive_leaves_survive_undo_and_redo() {
        let before = object(serde_json::json!({"Version":1,"Audio":{"Volume":1,"Vendor":7}}));
        let after = object(
            serde_json::json!({"Version":1,"Audio":{"Volume":0.5,"Vendor":7},"Archive":{"Raw":"// exact"}}),
        );
        let patch = Patch::between(&before, &after);
        let mut current = patch.apply(&before, true).unwrap();
        current["Archive"]
            .as_object_mut()
            .unwrap()
            .insert("Vendor".into(), Value::from(9));
        let undone = patch.apply(&current, false).unwrap();
        assert_eq!(undone["Archive"], serde_json::json!({"Vendor":9}));
        let redone = patch.apply(&undone, true).unwrap();
        assert_eq!(
            redone["Archive"],
            serde_json::json!({"Raw":"// exact","Vendor":9})
        );
    }
    #[test]
    fn conflicts_are_checked_before_any_mutation() {
        let before = object(serde_json::json!({"Audio":{"Volume":1},"Scroll":{"Speed":2}}));
        let after = object(serde_json::json!({"Audio":{"Volume":0.5},"Scroll":{"Speed":3}}));
        let patch = Patch::between(&before, &after);
        let mut current = before.clone();
        current["Scroll"]["Speed"] = Value::from(4);
        assert!(patch.apply(&current, true).is_err());
        assert_eq!(current["Audio"]["Volume"], Value::from(1));
    }
    #[test]
    fn arrays_are_owned_as_whole_values_and_empty_objects_are_changes() {
        let before = object(serde_json::json!({"Bindings":[["A"]]}));
        let after = object(serde_json::json!({"Bindings":[["B"]],"Empty":{}}));
        let patch = Patch::between(&before, &after);
        assert_eq!(patch.apply(&before, true).unwrap(), after);
        assert_eq!(patch.apply(&after, false).unwrap(), before);
    }
}
