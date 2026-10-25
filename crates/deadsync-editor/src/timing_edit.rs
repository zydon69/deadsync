//! Scoped, reversible timing edits. A first chart-local timing field detaches
//! the entire SSC timing set, so inherited fields must be copied together.
pub mod tempo;
use crate::{
    Error,
    document::{Document, Format, escape_text},
    history::{RevisionToken, Session},
};
use std::sync::Arc;
use tempo::Tempos;

pub const TIMING_TAGS: [&str; 12] = [
    "BPMS",
    "STOPS",
    "DELAYS",
    "TIMESIGNATURES",
    "TICKCOUNTS",
    "COMBOS",
    "WARPS",
    "SPEEDS",
    "SCROLLS",
    "FAKES",
    "LABELS",
    "OFFSET",
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Song,
    Chart,
}
pub struct TempoDraft {
    token: RevisionToken,
    chart: usize,
    scope: Scope,
    inherited: bool,
    original: Tempos,
    tempos: Tempos,
    inherited_fields: Vec<(String, Arc<[u8]>)>,
    affected: Vec<usize>,
}
impl TempoDraft {
    pub fn capture(session: &Session, chart: usize, scope: Scope) -> Result<Self, Error> {
        let document = session.document();
        if chart >= document.charts().len() {
            return Err(invalid("Timing chart does not exist"));
        }
        if scope == Scope::Chart {
            if document.format == Format::Sm {
                return Err(invalid("SM timing is shared by the entire song"));
            }
            if !supports_chart_timing(document) {
                return Err(invalid(
                    "Chart timing requires SSC version 0.7 or newer; legacy timing migration is not implemented yet",
                ));
            }
        }
        let inherited = scope == Scope::Chart && !uses_chart_timing(document, chart);
        let origin = if scope == Scope::Chart && !inherited {
            Some(chart)
        } else {
            None
        };
        let raw = document.text(origin, "BPMS").unwrap_or_default();
        let mut tempos = Tempos::parse(&raw, 120.0)?;
        if tempos.source_lacked_initial() {
            let fallback = if scope == Scope::Chart {
                chart_fallback(document, chart)?
            } else {
                song_fallback(document)
            };
            tempos = Tempos::parse(&raw, fallback)?;
        }
        let inherited_fields = if inherited {
            inherited_fields_for_write(document, chart, &["BPMS"])?
        } else {
            Vec::new()
        };
        let affected = if scope == Scope::Chart {
            vec![chart]
        } else {
            (0..document.charts().len())
                .filter(|chart| !uses_chart_timing(document, *chart))
                .collect()
        };
        Ok(Self {
            token: session.revision_token(),
            chart,
            scope,
            inherited,
            original: tempos.clone(),
            tempos,
            inherited_fields,
            affected,
        })
    }
    pub fn scope(&self) -> Scope {
        self.scope
    }
    pub fn inherited(&self) -> bool {
        self.inherited
    }
    pub fn affected_charts(&self) -> &[usize] {
        &self.affected
    }
    pub fn tempos(&self) -> &Tempos {
        &self.tempos
    }
    pub fn set(&mut self, row: u64, bpm: f64) -> Result<bool, Error> {
        self.tempos.set(row, bpm)
    }
    pub fn remove(&mut self, row: u64) -> Result<bool, Error> {
        self.tempos.remove(row)
    }
    pub fn move_node(&mut self, from: u64, to: u64) -> Result<bool, Error> {
        self.tempos.move_node(from, to)
    }
    pub fn has_changes(&self) -> bool {
        self.tempos != self.original
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(invalid("BPM draft belongs to another chart or revision"));
        }
        if !self.has_changes() {
            return Ok(false);
        }
        let target = if self.scope == Scope::Chart {
            Some(self.chart)
        } else {
            None
        };
        let mut properties: Vec<_> = self
            .inherited_fields
            .iter()
            .map(|(name, bytes)| (target, name.clone(), bytes.clone()))
            .collect();
        properties.push((
            target,
            "BPMS".to_owned(),
            Arc::from(escape_text(&self.tempos.serialize())),
        ));
        session.set_property_bytes_atomic(&properties, "Edit BPM timing")
    }
}
/// Any first local SSC timing field, including OFFSET, must preserve the
/// complete inherited set. The caller replaces its own target fields separately.
pub(crate) fn inherited_fields_for_write(
    document: &Document,
    chart: usize,
    exclude: &[&str],
) -> Result<Vec<(String, Arc<[u8]>)>, Error> {
    if chart >= document.charts().len() {
        return Err(invalid("Timing chart does not exist"));
    }
    if document.format != Format::Ssc
        || TIMING_TAGS
            .iter()
            .any(|tag| document.raw_property(Some(chart), tag).is_some())
    {
        return Ok(Vec::new());
    }
    Ok(TIMING_TAGS
        .iter()
        .filter(|tag| !exclude.contains(tag))
        .filter_map(|tag| {
            document
                .raw_property(None, tag)
                .map(|bytes| ((*tag).to_owned(), Arc::from(bytes)))
        })
        .collect())
}
pub fn uses_chart_timing(document: &Document, chart: usize) -> bool {
    document.format == Format::Ssc
        && supports_chart_timing(document)
        && TIMING_TAGS
            .iter()
            .any(|tag| document.raw_property(Some(chart), tag).is_some())
}
/// Whether the file version supports a separate timing set per SSC chart.
pub fn supports_chart_timing(document: &Document) -> bool {
    if document.format != Format::Ssc {
        return false;
    }
    let version = rssp::parse::parse_version(
        document.raw_property(None, "VERSION"),
        rssp::timing::TimingFormat::Ssc,
    );
    rssp::timing::steps_timing_allowed(version, rssp::timing::TimingFormat::Ssc)
}
fn song_fallback(document: &Document) -> f64 {
    // Match the song/default fallback after the earlier-chart histogram.
    let first = document
        .text(None, "BPMS")
        .and_then(|text| tempo::parse_pairs(&text).ok())
        .and_then(|pairs| {
            pairs
                .into_iter()
                .find(|(beat, _)| *beat == 0.0)
                .map(|(_, bpm)| bpm)
        })
        .filter(|bpm| bpm.is_finite() && *bpm >= tempo::MIN_BPM);
    first
        .or_else(|| {
            document
                .text(None, "DISPLAYBPM")
                .and_then(|value| value.split(':').next()?.trim().parse::<f64>().ok())
                .filter(|bpm| bpm.is_finite() && *bpm >= tempo::MIN_BPM)
        })
        .unwrap_or(120.0)
}
fn chart_fallback(document: &Document, chart: usize) -> Result<f64, Error> {
    // EditorSong enumerates chart-type groups in first-seen order, and charts
    // inside each group in load order. Rebuild that histogram before loading
    // each chart: a missing initial tempo can itself depend on earlier groups.
    let mut groups: Vec<(&str, Vec<f64>)> = Vec::new();
    for index in 0..chart {
        let histogram = grouped_histogram(&groups);
        let scope = uses_chart_timing(document, index).then_some(index);
        let raw = document.text(scope, "BPMS").unwrap_or_default();
        let mut values = Tempos::parse(&raw, 120.0)?;
        if values.source_lacked_initial() {
            values = Tempos::parse(
                &raw,
                most_common(&histogram).unwrap_or_else(|| song_fallback(document)),
            )?;
        }
        let kind = document.charts()[index].steps_type.as_str();
        if let Some((_, starts)) = groups.iter_mut().find(|(name, _)| *name == kind) {
            starts.push(values.at(0));
        } else {
            groups.push((kind, vec![values.at(0)]));
        }
    }
    Ok(most_common(&grouped_histogram(&groups)).unwrap_or_else(|| song_fallback(document)))
}
fn grouped_histogram(groups: &[(&str, Vec<f64>)]) -> Vec<(f64, usize)> {
    let mut histogram: Vec<(f64, usize)> = Vec::new();
    for start in groups.iter().flat_map(|(_, starts)| starts) {
        if let Some(entry) = histogram.iter_mut().find(|entry| entry.0 == *start) {
            entry.1 += 1;
        } else {
            histogram.push((*start, 1));
        }
    }
    histogram
}
fn most_common(histogram: &[(f64, usize)]) -> Option<f64> {
    histogram
        .iter()
        .reduce(|left, right| if left.1 > right.1 { left } else { right })
        .map(|entry| entry.0)
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
