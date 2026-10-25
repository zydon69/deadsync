use crate::Error;
use std::{collections::BTreeMap, ops::Range, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Sm,
    Ssc,
}

impl Format {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Sm => "sm",
            Self::Ssc => "ssc",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tag {
    pub name: String,
    pub value: Range<usize>,
    pub chart: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Chart {
    pub steps_type: String,
    pub description: String,
    pub difficulty: String,
    pub meter: String,
    pub notes: Range<usize>,
}

/// Spans refer to immutable source bytes, so a replacement never invalidates
/// another chart's identity or an unknown property. Reading is lazy for notes.
#[derive(Clone, Debug)]
pub struct Document {
    source: Arc<[u8]>,
    pub format: Format,
    tags: Vec<Tag>,
    charts: Vec<Chart>,
    replacements: BTreeMap<(usize, usize), Arc<[u8]>>,
    added: BTreeMap<(Option<usize>, String), Arc<[u8]>>,
    added_spelling: BTreeMap<(Option<usize>, String), String>,
}

impl Document {
    pub fn parse(source: impl Into<Arc<[u8]>>, format: Format) -> Result<Self, Error> {
        let source = source.into();
        let mut tags = Vec::new();
        let mut charts: Vec<Chart> = Vec::new();
        let mut pos = 0;
        let mut chart = None;
        while pos < source.len() {
            if source[pos..].starts_with(b"//") {
                pos = skip_comment(&source, pos);
                continue;
            }
            if source[pos] != b'#' {
                pos += 1;
                continue;
            }
            let start = pos + 1;
            pos = delimiter(&source, start, b':').ok_or_else(|| {
                Error::InvalidFormat(format!("Unterminated tag at byte {}", start - 1))
            })?;
            let name = std::str::from_utf8(&source[start..pos])
                .map_err(|_| Error::InvalidFormat("Invalid tag name".into()))?
                .trim()
                .to_ascii_uppercase();
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(Error::InvalidFormat(format!(
                    "Invalid tag name at byte {start}"
                )));
            }
            let value_start = pos + 1;
            pos = delimiter(&source, value_start, b';')
                .ok_or_else(|| Error::InvalidFormat(format!("Unterminated {name} tag")))?;
            let value = value_start..pos;
            if format == Format::Ssc && name == "NOTEDATA" {
                chart = Some(charts.len());
                charts.push(Chart {
                    steps_type: String::new(),
                    description: String::new(),
                    difficulty: String::new(),
                    meter: String::new(),
                    notes: pos..pos,
                });
            }
            if format == Format::Sm && matches!(name.as_str(), "NOTES" | "NOTES2") {
                let mut fields = Vec::new();
                let mut offset = value_start;
                for _ in 0..5 {
                    let next = delimiter(&source, offset, b':')
                        .filter(|end| *end < pos)
                        .ok_or_else(|| {
                            Error::InvalidFormat("SM chart requires six fields".into())
                        })?;
                    fields.push(decode(&source[offset..next]));
                    offset = next + 1;
                }
                charts.push(Chart {
                    steps_type: fields[0].clone(),
                    description: fields[1].clone(),
                    difficulty: fields[2].clone(),
                    meter: fields[3].clone(),
                    notes: offset..pos,
                });
                tags.push(Tag {
                    name,
                    value,
                    chart: Some(charts.len() - 1),
                });
            } else {
                if let Some(index) = chart {
                    let current = &mut charts[index];
                    match name.as_str() {
                        "STEPSTYPE" => current.steps_type = decode(&source[value.clone()]),
                        "DESCRIPTION" => current.description = decode(&source[value.clone()]),
                        "DIFFICULTY" => current.difficulty = decode(&source[value.clone()]),
                        "METER" => current.meter = decode(&source[value.clone()]),
                        "NOTES" | "NOTES2" => current.notes = value.clone(),
                        _ => {}
                    }
                }
                tags.push(Tag { name, value, chart });
            }
            pos += 1;
        }
        if charts.iter().any(|c| c.notes.is_empty()) {
            return Err(Error::InvalidFormat(
                "Chart is missing its note data".into(),
            ));
        }
        Ok(Self {
            source,
            format,
            tags,
            charts,
            replacements: BTreeMap::new(),
            added: BTreeMap::new(),
            added_spelling: BTreeMap::new(),
        })
    }

    pub fn charts(&self) -> &[Chart] {
        &self.charts
    }
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }
    pub fn source(&self) -> &Arc<[u8]> {
        &self.source
    }

    pub fn value(&self, range: Range<usize>) -> &[u8] {
        self.replacements
            .get(&(range.start, range.end))
            .map_or(&self.source[range], AsRef::as_ref)
    }

    pub fn tag(&self, chart: Option<usize>, name: &str) -> Option<&Tag> {
        self.tags
            .iter()
            .rev()
            .find(|tag| tag.chart == chart && tag.name.eq_ignore_ascii_case(name))
    }

    pub fn text(&self, chart: Option<usize>, name: &str) -> Option<String> {
        self.raw_property(chart, name).map(decode)
    }

    pub fn raw_property(&self, chart: Option<usize>, name: &str) -> Option<&[u8]> {
        self.added
            .get(&(chart, name.to_ascii_uppercase()))
            .map(AsRef::as_ref)
            .or_else(|| {
                self.tag(chart, name)
                    .map(|tag| self.value(tag.value.clone()))
            })
    }

    pub(crate) fn added_property(&self, chart: Option<usize>, name: &str) -> Option<Arc<[u8]>> {
        self.added.get(&(chart, name.to_ascii_uppercase())).cloned()
    }

    pub(crate) fn set_added_property(
        &mut self,
        chart: Option<usize>,
        name: &str,
        value: Option<Arc<[u8]>>,
    ) -> Result<(), Error> {
        if chart.is_some_and(|chart| chart >= self.charts.len())
            || (self.format == Format::Sm && chart.is_some())
        {
            return Err(Error::InvalidEdit(
                "Chart property scope is invalid for this format".into(),
            ));
        }
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            return Err(Error::InvalidEdit("Invalid property name".into()));
        }
        let key = (chart, name.to_ascii_uppercase());
        if let Some(value) = value {
            // Custom extras are case-sensitive in Fumen/GrooveAuthor. Keep
            // the spelling of the first insertion while lookups stay folded.
            self.added_spelling
                .entry(key.clone())
                .or_insert_with(|| name.to_owned());
            self.added.insert(key, value);
        } else {
            self.added.remove(&key);
            self.added_spelling.remove(&key);
        }
        Ok(())
    }

    /// Capture the exact previous replacement, rather than a reserialized value,
    /// so undo restores the original formatting and comments as well as data.
    pub(crate) fn replacement(&self, range: &Range<usize>) -> Option<Arc<[u8]>> {
        self.replacements.get(&(range.start, range.end)).cloned()
    }

    pub(crate) fn replace(
        &mut self,
        range: Range<usize>,
        value: Option<Arc<[u8]>>,
    ) -> Result<(), Error> {
        if range.start > range.end || range.end > self.source.len() {
            return Err(Error::InvalidEdit(
                "Replacement lies outside the source".into(),
            ));
        }
        for &(start, end) in self.replacements.keys() {
            if (start, end) != (range.start, range.end) && start < range.end && range.start < end {
                return Err(Error::InvalidEdit("Overlapping source replacements".into()));
            }
        }
        let key = (range.start, range.end);
        if let Some(value) = value {
            if value.as_ref() == &self.source[range] {
                self.replacements.remove(&key);
            } else {
                self.replacements.insert(key, value);
            }
        } else {
            self.replacements.remove(&key);
        }
        Ok(())
    }

    /// Clone current chart metadata without rewriting any existing chart.
    /// Generated note data replaces the source body; chart-local unknown tags
    /// and legacy metadata bytes remain intact in the new block.
    pub(crate) fn cloned_chart_bytes(
        &self,
        chart: usize,
        layout: &str,
        notes: Arc<[u8]>,
    ) -> Result<Vec<u8>, Error> {
        let descriptor = self
            .charts
            .get(chart)
            .ok_or_else(|| Error::InvalidEdit("Source chart does not exist".into()))?;
        let (start, end) = if self.format == Format::Ssc {
            let marker = self
                .tags
                .iter()
                .find(|tag| tag.chart == Some(chart) && tag.name == "NOTEDATA")
                .ok_or_else(|| {
                    Error::InvalidFormat("Source chart has no NOTEDATA marker".into())
                })?;
            let start = self.marker_start(marker);
            let end = self
                .tags
                .iter()
                .find(|tag| tag.chart == Some(chart + 1) && tag.name == "NOTEDATA")
                .map_or(self.source.len(), |tag| self.marker_start(tag));
            (start, end)
        } else {
            let marker = self
                .tags
                .iter()
                .find(|tag| {
                    tag.chart == Some(chart) && matches!(tag.name.as_str(), "NOTES" | "NOTES2")
                })
                .ok_or_else(|| Error::InvalidFormat("Source chart has no NOTES marker".into()))?;
            (self.marker_start(marker), marker.value.end + 1)
        };
        let mut clone = Self::parse(Arc::<[u8]>::from(&self.source[start..end]), self.format)?;
        if clone.charts.len() != 1 {
            return Err(Error::InvalidFormat("Ambiguous source chart block".into()));
        }
        if self.format == Format::Ssc {
            for tag in self
                .tags
                .iter()
                .filter(|tag| tag.chart == Some(chart) && tag.name != "NOTEDATA")
            {
                let range = tag.value.start - start..tag.value.end - start;
                let value = if tag.name == "STEPSTYPE" {
                    Arc::from(layout.as_bytes())
                } else if matches!(tag.name.as_str(), "NOTES" | "NOTES2") {
                    Arc::clone(&notes)
                } else {
                    Arc::from(self.value(tag.value.clone()))
                };
                clone.replace(range, Some(value))?;
            }
            if self.tag(Some(chart), "STEPSTYPE").is_none() {
                clone.set_added_property(
                    Some(0),
                    "STEPSTYPE",
                    Some(Arc::from(layout.as_bytes())),
                )?;
            }
            for ((scope, name), value) in &self.added {
                if *scope == Some(chart)
                    && !matches!(name.as_str(), "STEPSTYPE" | "NOTES" | "NOTES2" | "NOTEDATA")
                {
                    clone.set_added_property(
                        Some(0),
                        self.added_spelling
                            .get(&(*scope, name.clone()))
                            .map_or(name.as_str(), String::as_str),
                        Some(Arc::clone(value)),
                    )?;
                }
            }
        } else {
            let value_start = self
                .tags
                .iter()
                .find(|tag| tag.chart == Some(chart))
                .unwrap()
                .value
                .start
                - start;
            let first_colon = delimiter(&clone.source, value_start, b':')
                .ok_or_else(|| Error::InvalidFormat("Invalid SM chart header".into()))?;
            clone.replace(
                value_start..first_colon,
                Some(format!("\n     {layout}").into_bytes().into()),
            )?;
            clone.replace(
                descriptor.notes.start - start..descriptor.notes.end - start,
                Some(notes),
            )?;
        }
        Ok(clone.serialize())
    }

    fn marker_start(&self, tag: &Tag) -> usize {
        self.source[..tag.value.start]
            .iter()
            .rposition(|&byte| byte == b'#')
            .unwrap_or(0)
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut replacements = self.replacements.clone();
        let mut insertions: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
        for ((chart, name), value) in &self.added {
            let at = if let Some(chart) = chart {
                self.charts[*chart].notes.start
            } else {
                self.tags
                    .iter()
                    .find(|tag| tag.chart.is_some())
                    .map_or(self.source.len(), |tag| {
                        // Insert before the tag marker, not inside its value.
                        self.source[..tag.value.start]
                            .iter()
                            .rposition(|&byte| byte == b'#')
                            .unwrap_or(0)
                    })
            };
            let at = if chart.is_some() {
                self.source[..at]
                    .iter()
                    .rposition(|&byte| byte == b'#')
                    .unwrap_or(at)
            } else {
                at
            };
            let bytes = insertions.entry(at).or_default();
            bytes.push(b'\n');
            bytes.push(b'#');
            bytes.extend_from_slice(
                self.added_spelling
                    .get(&(*chart, name.clone()))
                    .map_or(name.as_str(), String::as_str)
                    .as_bytes(),
            );
            bytes.push(b':');
            bytes.extend_from_slice(value);
            bytes.extend_from_slice(b";\n");
        }
        for (at, bytes) in insertions {
            replacements.insert((at, at), bytes.into());
        }
        let mut output = Vec::with_capacity(self.source.len());
        let mut cursor = 0;
        for (&(start, end), value) in &replacements {
            output.extend_from_slice(&self.source[cursor..start]);
            output.extend_from_slice(value);
            cursor = end;
        }
        output.extend_from_slice(&self.source[cursor..]);
        output
    }
}

pub fn escape_text(text: &str) -> Vec<u8> {
    let mut result = Vec::with_capacity(text.len());
    for byte in text.bytes() {
        if matches!(byte, b'\\' | b':' | b';' | b'#' | b'/') {
            result.push(b'\\');
        }
        result.push(byte);
    }
    result
}

fn decode(source: &[u8]) -> String {
    let mut bytes = Vec::new();
    let mut pos = 0;
    while pos < source.len() {
        if source[pos..].starts_with(b"//") {
            pos = skip_comment(source, pos);
            continue;
        }
        if source[pos] == b'\\' && pos + 1 < source.len() {
            bytes.extend_from_slice(&source[pos..pos + 2]);
            pos += 2;
            continue;
        }
        bytes.push(source[pos]);
        pos += 1;
    }
    rssp::parse::decode_unescape_trim(&bytes).into_owned()
}

fn skip_comment(source: &[u8], pos: usize) -> usize {
    source[pos..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(source.len(), |end| pos + end + 1)
}

fn delimiter(source: &[u8], mut pos: usize, needle: u8) -> Option<usize> {
    while pos < source.len() {
        if source[pos] == b'\\' {
            pos += 2;
            continue;
        }
        if source[pos..].starts_with(b"//") {
            pos = skip_comment(source, pos);
            continue;
        }
        if source[pos] == needle {
            return Some(pos);
        }
        // A new tag cannot begin inside an unterminated value.
        if source[pos] == b'#' {
            return None;
        }
        pos += 1;
    }
    None
}
