//! Read-only song/chart headers sharing the editable document's tokenizer.
//! No note spans, editing commands or writer entry point are exposed.
use super::{Format, decode_property, parse_parts};
use crate::Error;
use std::sync::Arc;
/// Resource bounds for read-only previews; editable parsing is unchanged.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub source_bytes: usize,
    pub charts: usize,
    pub tags: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            source_bytes: 16 * 1024 * 1024,
            charts: 4096,
            tags: 65536,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chart {
    pub steps_type: String,
    pub description: String,
    pub difficulty: String,
    pub meter: String,
}
#[derive(Clone, Debug)]
pub struct Metadata {
    source: Arc<[u8]>,
    format: Format,
    tags: Vec<super::Tag>,
    charts: Vec<Chart>,
}
impl Metadata {
    pub fn parse(source: impl Into<Arc<[u8]>>, format: Format) -> Result<Self, Error> {
        Self::parse_with_limits(source, format, Limits::default())
    }
    pub fn parse_with_limits(
        source: impl Into<Arc<[u8]>>,
        format: Format,
        limits: Limits,
    ) -> Result<Self, Error> {
        let source = source.into();
        if source.len() > limits.source_bytes {
            return Err(Error::Unsupported("Metadata byte limit exceeded".into()));
        }
        let parts = parse_parts(source, format, Some((limits.charts, limits.tags)))?;
        Ok(Self {
            source: parts.source,
            format,
            tags: parts.tags,
            charts: parts
                .charts
                .into_iter()
                .map(|chart| Chart {
                    steps_type: chart.steps_type,
                    description: chart.description,
                    difficulty: chart.difficulty,
                    meter: chart.meter,
                })
                .collect(),
        })
    }
    pub fn charts(&self) -> &[Chart] {
        &self.charts
    }
    /// Song-level value, decoded by the same routine as strict Document::text.
    pub fn text(&self, name: &str) -> Option<String> {
        self.tags
            .iter()
            .rev()
            .find(|tag| tag.chart.is_none() && tag.name.eq_ignore_ascii_case(name))
            .map(|tag| decode_property(&self.source[tag.value.clone()], self.format, false, name))
    }
}
