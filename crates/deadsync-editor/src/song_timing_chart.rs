//! Song-wide timing reference, independent of chart focus and preview clocks.
//!
//! Load resolves the serialized per-type index before presentation sorting.
//! Subsequent sorting and renaming retain the resident chart identity.
use crate::{
    Error, chart_order::Snapshot, document::chart_type, generation::metadata::Guid,
    open_preferences::Preferences,
};
use serde_json::{Map, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    identity: Option<Guid>,
}

impl Choice {
    pub(crate) fn restored(identity:Option<Guid>)->Self{Self{identity}}
    /// `snapshot` contains only the charts successfully loaded by the editor,
    /// in their original file order. Invalid custom data follows source fallback.
    pub fn load(
        snapshot: &Snapshot,
        custom: Option<&Map<String, Value>>,
        preferences: &Preferences,
    ) -> Result<Self, Error> {
        let selected = custom
            .and_then(|fields| saved_reference(fields).ok().flatten())
            .and_then(|(kind, index)| {
                snapshot
                    .keys()
                    .iter()
                    .filter(|key| chart_type_value(&key.steps_type) == Some(kind))
                    .nth(index)
                    .map(|key| key.guid.clone())
            });
        if selected.is_some() {
            return Ok(Self { identity: selected });
        }
        let index = preferences.select(snapshot, snapshot.source_indices())?;
        Ok(Self {
            identity: index.and_then(|index| identity(snapshot, index).cloned()),
        })
    }

    pub fn identity(&self) -> Option<&Guid> {
        self.identity.as_ref()
    }

    /// The source setter accepts null, but refuses another song's chart.
    pub fn set(&mut self, snapshot: &Snapshot, chart: Option<usize>) -> Result<bool, Error> {
        let next = chart
            .map(|index| {
                identity(snapshot, index)
                    .cloned()
                    .ok_or_else(|| invalid("Timing chart does not belong to the loaded song"))
            })
            .transpose()?;
        if self.identity == next {
            return Ok(false);
        }
        self.identity = next;
        Ok(true)
    }

    /// Deleting the chosen chart uses current preferences and the current
    /// cultural order. Deleting a peer, sorting and metadata edits keep it.
    pub fn reconcile_after_deletion(
        &mut self,
        snapshot: &Snapshot,
        preferences: &Preferences,
        cultural_order: &[usize],
    ) -> Result<(), Error> {
        if self.identity.is_none() {
            return Ok(());
        }
        if self
            .identity
            .as_ref()
            .is_some_and(|id| snapshot.keys().iter().any(|key| &key.guid == id))
        {
            return Ok(());
        }
        let index = preferences.select(snapshot, cultural_order)?;
        self.identity = index.and_then(|index| identity(snapshot, index).cloned());
        Ok(())
    }

    /// Encode against the candidate's actual output order, never a UI index.
    /// The caller merges these two fields with the single song metadata object.
    pub fn output_fields(
        &self,
        snapshot: &Snapshot,
        output_order: &[usize],
    ) -> Result<Map<String, Value>, Error> {
        validate_order(snapshot, output_order)?;
        let mut fields = Map::new();
        let Some(id) = &self.identity else {
            fields.insert("DefaultChartType".into(), Value::Null);
            fields.insert("DefaultChartIndex".into(), 0.into());
            return Ok(fields);
        };
        let selected = snapshot
            .keys()
            .iter()
            .find(|key| &key.guid == id)
            .ok_or_else(|| invalid("Selected song timing chart no longer exists"))?;
        let keys: std::collections::HashMap<_, _> = snapshot
            .source_indices()
            .iter()
            .copied()
            .zip(snapshot.keys())
            .collect();
        let mut index = 0i32;
        for &source in output_order {
            let key = keys[&source];
            if key.guid == *id {
                fields.insert(
                    "DefaultChartType".into(),
                    selected.steps_type.replace('-', "_").into(),
                );
                fields.insert("DefaultChartIndex".into(), index.into());
                return Ok(fields);
            }
            if key.steps_type == selected.steps_type {
                index = index
                    .checked_add(1)
                    .ok_or_else(|| invalid("Timing chart index overflow"))?;
            }
        }
        Err(invalid("Timing chart is absent from output order"))
    }
}

fn identity(snapshot: &Snapshot, source: usize) -> Option<&Guid> {
    snapshot
        .source_indices()
        .iter()
        .position(|index| *index == source)
        .map(|index| &snapshot.keys()[index].guid)
}

fn validate_order(snapshot: &Snapshot, order: &[usize]) -> Result<(), Error> {
    let mut expected = snapshot.source_indices().to_vec();
    expected.sort_unstable();
    let mut actual = order.to_vec();
    actual.sort_unstable();
    if actual != expected {
        return Err(invalid(
            "Timing chart output order is not an exhaustive permutation",
        ));
    }
    Ok(())
}

fn saved_reference(fields: &Map<String, Value>) -> Result<Option<(i32, usize)>, ()> {
    // System.Text.Json deserializes the entire V1 object before assigning any
    // member. A malformed SyncOffset invalidates its chart reference as well.
    if fields
        .get("SyncOffset")
        .is_some_and(|value| value.as_f64().is_none_or(|value| !value.is_finite()))
    {
        return Err(());
    }
    let index = match fields.get("DefaultChartIndex") {
        None => 0,
        Some(value) => i32::try_from(value.as_i64().ok_or(())?).map_err(|_| ())?,
    };
    let kind = match fields.get("DefaultChartType") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::Number(number)) => i32::try_from(number.as_i64().ok_or(())?).map_err(|_| ())?,
        Some(Value::String(text)) => parse_chart_type(text)?,
        _ => return Err(()),
    };
    Ok(usize::try_from(index).ok().map(|index| (kind, index)))
}

fn chart_type_value(kind: &str) -> Option<i32> {
    chart_type::ALL
        .iter()
        .position(|candidate| *candidate == kind)
        .map(|index| index as i32)
}

fn parse_chart_type(text: &str) -> Result<i32, ()> {
    let text = text.trim();
    if let Ok(value) = text.parse::<i32>() {
        return Ok(value);
    }
    let mut result = 0;
    for name in text.split(',') {
        let name = name.trim();
        let value = chart_type::ALL
            .iter()
            .position(|candidate| candidate.replace('-', "_").eq_ignore_ascii_case(name))
            .ok_or(())?;
        result |= value as i32;
    }
    Ok(result)
}

fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
