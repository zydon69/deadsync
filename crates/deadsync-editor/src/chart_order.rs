//! Presentation order from GrooveAuthor EditorChart.ChartComparer (48e18f71,
//! MIT). Source indices are retained: sorting never rewrites simfile sections.
//!
//! The original uses CurrentCulture for nonempty names/descriptions. Callers
//! must supply that collation; Rust's lexical string order is not equivalent.
use crate::{
    Error,
    chart_identity::Identity,
    chart_text::{Field, Text},
    document::Document,
    generation::{editor_timing::EditorTimeline, metadata::Guid, timing::GenerationTiming},
    history::Session,
};
use std::cmp::Ordering;

/// Native cultural sorting boundary. The editor validates the returned source
/// indices before changing focus; transports must not rewrite song contents.
pub trait Orderer {
    fn order(&mut self, snapshot: &Snapshot) -> Result<Vec<usize>, Error>;
}

/// Bind a queued native operation to its original live song content and focus.
pub struct Binding {
    token: crate::history::ContentToken,
    chart: usize,
    identity: Guid,
}
impl Binding {
    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let identity = session
            .chart_ids()
            .get(chart)
            .cloned()
            .ok_or_else(|| Error::InvalidEdit("No active chart to order".into()))?;
        Ok(Self {
            token: session.content_token(),
            chart,
            identity,
        })
    }
    pub fn is_current(&self, session: &Session, chart: usize) -> bool {
        chart == self.chart
            && session.accepts_content_token(&self.token)
            && session.chart_ids().get(chart) == Some(&self.identity)
    }
}

/// ChartComparer's complete type rank, including the two Couple layouts that
/// DeadSync supports in addition to the original editor's visible chart list.
pub const TYPE_ORDER: [&str; 16] = [
    "dance-single",
    "dance-double",
    "dance-couple",
    "dance-routine",
    "dance-solo",
    "dance-threepanel",
    "pump-single",
    "pump-halfdouble",
    "pump-double",
    "pump-couple",
    "pump-routine",
    "smx-beginner",
    "smx-single",
    "smx-dual",
    "smx-full",
    "smx-team",
];

/// Immutable comparison inputs. The GUID is a resident editor identity, not a
/// profile GUID or persisted chart property. Keep it stable across edits and
/// undo/redo; assign a new one when adding or duplicating a chart, like upstream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Key {
    pub steps_type: String,
    pub difficulty: i32,
    pub rating: i32,
    pub name: String,
    pub description: String,
    pub step_count: usize,
    pub guid: Guid,
}
impl Key {
    pub fn read(
        document: &Document,
        chart: usize,
        step_count: usize,
        guid: Guid,
    ) -> Result<Self, Error> {
        let identity = Identity::read(document, chart)?;
        let text = Text::read(document, chart)?;
        let descriptor = &document.charts()[chart];
        // Fumen promotes StepF2 Pump co-op before EditorChart is constructed.
        // Keep the original bytes while comparing the same effective type.
        let steps_type = if descriptor.steps_type == "pump-double"
            && crate::stepf2::is_coop(document.value(descriptor.notes.clone()), "pump-double")
        {
            "pump-routine".to_owned()
        } else {
            descriptor.steps_type.clone()
        };
        let difficulty = identity
            .difficulty()
            .map(|value| {
                crate::chart_identity::Difficulty::ALL
                    .iter()
                    .position(|candidate| *candidate == value)
                    .unwrap() as i32
            })
            .unwrap_or_else(|| identity.raw_difficulty().trim().parse::<i32>().unwrap_or(0));
        Ok(Self {
            steps_type,
            difficulty,
            rating: identity.rating(),
            name: text.get(Field::Name).to_owned(),
            description: text.get(Field::Description).to_owned(),
            step_count,
            guid,
        })
    }
}

/// Exact comparison precedence. Unknown types compare by the remaining fields,
/// without inventing an alphabetical type tie breaker. Empty strings sort last.
/// `None` mirrors the reference's null chart behavior.
pub fn compare_with(
    left: Option<&Key>,
    right: Option<&Key>,
    mut collate: impl FnMut(&str, &str) -> Ordering,
) -> Ordering {
    let (left, right) = match (left, right) {
        (None, None) => return Ordering::Equal,
        (None, _) => return Ordering::Greater,
        (_, None) => return Ordering::Less,
        (Some(left), Some(right)) => (left, right),
    };
    let rank = |key: &Key| TYPE_ORDER.iter().position(|kind| *kind == key.steps_type);
    let types = match (rank(left), rank(right)) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    types
        .then_with(|| left.difficulty.wrapping_sub(right.difficulty).cmp(&0))
        .then_with(|| left.rating.wrapping_sub(right.rating).cmp(&0))
        .then_with(|| strings(&left.name, &right.name, &mut collate))
        .then_with(|| strings(&left.description, &right.description, &mut collate))
        .then_with(|| left.step_count.cmp(&right.step_count))
        .then_with(|| left.guid.cmp(&right.guid))
}
fn strings(left: &str, right: &str, collate: &mut impl FnMut(&str, &str) -> Ordering) -> Ordering {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => collate(left, right),
    }
}

/// Capture live working notes and current metadata without mutating the song.
/// Loading a missing grid only populates Session's read cache. In particular,
/// fake/warped notes use the already-ported editor clock's step-count rules.
pub struct Snapshot {
    keys: Vec<Key>,
    source_indices: Vec<usize>,
}
impl Snapshot {
    /// Use the identities owned by Session, including structural history.
    pub fn resident(session: &mut Session) -> Result<Self, Error> {
        let identities = session.chart_ids().to_vec();
        Self::read(session, &identities)
    }
    pub fn read(session: &mut Session, identities: &[Guid]) -> Result<Self, Error> {
        if identities.len() != session.document().charts().len() {
            return Err(Error::InvalidEdit(
                "Chart ordering identities do not match the song".into(),
            ));
        }
        let mut keys = Vec::with_capacity(identities.len());
        let mut source_indices = Vec::with_capacity(identities.len());
        for (index, guid) in identities.iter().enumerate() {
            // Upstream keeps unsupported charts outside its editable list.
            // Preserve their source blocks, without trying to parse their grids.
            if crate::notes::layout_columns(&session.document().charts()[index].steps_type)
                .is_none()
            {
                continue;
            }
            let timing = GenerationTiming::from_document(session.document(), index)?;
            let grid = session.grid(index)?;
            let steps = EditorTimeline::build_clock(&timing, grid, &[])?.step_count();
            keys.push(Key::read(session.document(), index, steps, guid.clone())?);
            source_indices.push(index);
        }
        Ok(Self {
            keys,
            source_indices,
        })
    }
    pub fn keys(&self) -> &[Key] {
        &self.keys
    }
    /// Source document index corresponding to each comparison key. Unsupported
    /// charts retain their source blocks but do not occur in this editable set.
    pub fn source_indices(&self) -> &[usize] {
        &self.source_indices
    }
    /// Returns original source indices, including Couple. Use these for display
    /// and selection; do not feed their positions back as document indices.
    /// `collate` must be a consistent culture comparator. Imported signed
    /// ratings/difficulties whose range overflows the original subtraction are
    /// rejected: upstream's comparator becomes nontransitive at those extremes.
    pub fn indices_with(
        &self,
        mut collate: impl FnMut(&str, &str) -> Ordering,
    ) -> Result<Vec<usize>, Error> {
        for (name, values) in [
            (
                "difficulty",
                self.keys
                    .iter()
                    .map(|key| key.difficulty)
                    .collect::<Vec<_>>(),
            ),
            (
                "rating",
                self.keys.iter().map(|key| key.rating).collect::<Vec<_>>(),
            ),
        ] {
            if let (Some(min), Some(max)) = (values.iter().min(), values.iter().max()) {
                if i64::from(*max) - i64::from(*min) > i64::from(i32::MAX) {
                    return Err(Error::InvalidEdit(format!(
                        "Chart {name} range overflows the original ordering comparator"
                    )));
                }
            }
        }
        let mut indices: Vec<_> = (0..self.keys.len()).collect();
        indices
            .sort_by(|a, b| compare_with(Some(&self.keys[*a]), Some(&self.keys[*b]), &mut collate));
        Ok(indices
            .into_iter()
            .map(|index| self.source_indices[index])
            .collect())
    }
}
