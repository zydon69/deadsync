//! Prepared output only. Durable publication belongs to the shared storage layer.
use super::*;
use crate::{
    document::escape_text,
    notes::{Note, Position},
    stepf2::Attributes,
};
use std::collections::{BTreeMap, BTreeSet};

pub struct Output {
    bytes: Arc<[u8]>,
    format: Format,
    token: ContentToken,
    ids: Vec<Guid>,
    culture: Option<crate::source_output_order::CultureGuard>,
}
impl Output {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn format(&self) -> Format {
        self.format
    }
    pub fn is_current(&self, session: &Session) -> bool {
        session.accepts_content_token(&self.token)
            && session.chart_ids() == self.ids.as_slice()
            && self
                .culture
                .as_ref()
                .is_none_or(|culture| culture.is_current())
    }
}
impl Plan {
    /// Generate a bounded detached file after exact loss review. This never
    /// rebases the live Session or calls a same-format Save As publication.
    pub fn generate(
        &self,
        session: &Session,
        acknowledged: &BTreeSet<usize>,
        maximum_bytes: usize,
        cancelled: &AtomicBool,
    ) -> Result<Output, Error> {
        self.validate_review(session, acknowledged)?;
        check(cancelled)?;
        let document = Document::parse_with_context(
            self.source.clone(),
            self.source_format,
            self.load_context,
        )?;
        require_scope_mappings(&document, self.target, self.options.anonymize)?;
        let expected_preview = if self.target == Format::Ssc {
            crate::song_text::resident::effective(&document)?
        } else {
            String::new()
        };
        let document = if self.target == Format::Ssc
            && (document.format == Format::Sm
                || crate::song_text::resident::read(&document)?.is_some())
        {
            crate::song_text::resident::materialize_for_output(&document, self.target)?
        } else {
            document
        };
        let mut source = Session::new(document);
        let mut output = Buffer {
            bytes: Vec::new(),
            maximum: maximum_bytes,
        };
        if self.target == Format::Ssc {
            output.tag("VERSION", b"0.83")?;
        }
        for name in SONG_FIELDS {
            check(cancelled)?;
            if self.options.anonymize && SONG_ANON.contains(name) {
                continue;
            }
            if self.target == Format::Sm && SM_OMITTED_SONG_FIELDS.contains(name) {
                continue;
            }
            if let Some(value) = source.document().raw_property(None, name) {
                output.tag(name, value)?;
            }
        }
        let base = match self.timing_chart {
            Some(chart) => inspect_timing(&source, chart)?,
            None => chartless_timing(source.document())?,
        };
        write_timing(&mut output, &base, self.target, true)?;
        if !self.options.omit_custom_data {
            write_custom(
                &mut output,
                source.document(),
                None,
                None,
                self.source_format,
                self.target,
            )?;
        }
        let mut expected = Vec::new();
        for (output_index, &chart) in self.output_order.iter().enumerate() {
            check(cancelled)?;
            let descriptor = &source.document().charts()[chart];
            let layout = descriptor.steps_type.clone();
            let spelling = if self.options.outfox_smx {
                match layout.as_str() {
                    "smx-dual" => "smx-double6",
                    "smx-full" => "smx-double10",
                    "smx-team" => "smx-routine",
                    _ => layout.as_str(),
                }
            } else {
                layout.as_str()
            };
            let description = if self.options.anonymize {
                String::new()
            } else {
                crate::chart_text::Text::read(source.document(), chart)?
                    .get(crate::chart_text::Field::Description)
                    .to_owned()
            };
            let difficulty = descriptor.difficulty.clone();
            let meter = descriptor.meter.clone();
            if !meter.is_empty() && meter.parse::<i32>().is_err() {
                return Err(invalid(
                    "A non-integer rating requires a reviewed target-format policy",
                ));
            }
            let timing = inspect_timing(&source, chart)?;
            if self.target == Format::Ssc {
                output.tag("NOTEDATA", b"")?;
                output.tag("STEPSTYPE", spelling.as_bytes())?;
                output.tag("DESCRIPTION", &escape_text(&description))?;
                output.tag("DIFFICULTY", &escape_text(&difficulty))?;
                output.tag("METER", &escape_text(&meter))?;
                for name in CHART_FIELDS {
                    if self.options.anonymize
                        && matches!(*name, "CHARTNAME" | "CHARTSTYLE" | "CREDIT")
                    {
                        continue;
                    }
                    if let Some(field) = crate::chart_text::Field::ALL
                        .into_iter()
                        .find(|field| field.tag() == *name)
                    {
                        if let Some(value) =
                            crate::chart_text::local_value(source.document(), chart, field)?
                        {
                            output.tag(name, &escape_text(&value))?;
                        }
                    } else if let Some(value) = source.document().raw_property(Some(chart), name) {
                        output.tag(name, value)?;
                    }
                }
                // Always retain independent chart attacks. Chart timing omission
                // must not migrate attacks from an unrelated fallback chart.
                if !self.options.omit_chart_timing {
                    write_timing(&mut output, &timing, self.target, false)?;
                } else {
                    // SSCWriter emits attacks and display BPM independently of
                    // ForceOnlySongLevelTiming. The source precheck still
                    // refuses unequal display values when timing is omitted.
                    for (name, value) in timing
                        .iter()
                        .filter(|(name, _)| matches!(*name, "ATTACKS" | "DISPLAYBPM"))
                    {
                        output.tag(name, value.as_bytes())?;
                    }
                }
                if !self.options.omit_custom_data {
                    write_custom(
                        &mut output,
                        source.document(),
                        Some(chart),
                        Some(output_index),
                        self.source_format,
                        self.target,
                    )?;
                }
                output.push(b"#NOTES:")?;
            } else {
                // SM chart custom properties belong to song scope, preceding
                // NOTES. The indexed spelling supplies the destination chart.
                if !self.options.omit_custom_data {
                    write_custom(
                        &mut output,
                        source.document(),
                        Some(chart),
                        Some(output_index),
                        self.source_format,
                        self.target,
                    )?;
                }
                output.push(b"#NOTES:")?;
                for field in [spelling, &description, &difficulty, &meter] {
                    output.push(&escape_text(field))?;
                    output.push(b":")?;
                }
                let radar = if self.source_format == Format::Sm {
                    source.document().sm_header_text(chart, 4)?
                } else {
                    source
                        .document()
                        .text(Some(chart), "RADARVALUES")
                        .unwrap_or_default()
                };
                output.push(&escape_text(&radar))?;
                output.push(b":")?;
            }
            let mut grid = source.grid(chart)?.clone();
            if !crate::notes::supports_variable_players(&layout)
                && Some(grid.players()) != crate::notes::default_players(&layout)
            {
                return Err(invalid(
                    "Fixed-layout multistream data is not accepted by the original target reader",
                ));
            }
            let step = self.options.stepf2_pump_multiplayer
                && layout.starts_with("pump-")
                && grid.players() > 1;
            if step {
                // This is the exact loss identified by StepF2MineOwner warning;
                // validate_review above requires its individual acknowledgement.
                let mines: Vec<_> = grid
                    .iter()
                    .filter(|(p, n)| p.player > 0 && n.kind == Kind::Mine)
                    .map(|(p, n)| (p, n.clone()))
                    .collect();
                for (position, note) in mines {
                    grid.set(position, None)?;
                    let target = Position {
                        player: 0,
                        ..position
                    };
                    if grid.get(target).is_some() {
                        return Err(invalid(
                            "Flattening StepF2 mine owner would replace another note",
                        ));
                    }
                    grid.set(target, Some(note))?;
                }
            }
            let notes = if step {
                crate::stepf2::serialize(&grid)?
            } else {
                grid.serialize_standard()?
            };
            check(cancelled)?;
            output.push(&notes)?;
            output.push(b";\n")?;
            expected.push((layout, grid, step, timing));
        }
        check(cancelled)?;
        let bytes: Arc<[u8]> = output.bytes.into();
        // Reuse the strict target reader and effective timing loaders. No file
        // escapes if the generated target fails any preservation postcondition.
        let mut reopened = Session::new(Document::parse_with_context(
            bytes.clone(),
            self.target,
            self.load_context,
        )?);
        if crate::song_text::resident::effective(reopened.document())? != expected_preview {
            return Err(invalid(
                "Target reader changed the effective song preview path",
            ));
        }
        if !self.options.omit_custom_data
            && crate::song_properties::sync_offset(source.document())?
                != crate::song_properties::sync_offset(reopened.document())?
        {
            return Err(invalid(
                "Target reader changed custom song SyncOffset; compatible custom-song mapping is required",
            ));
        }
        if reopened.document().charts().len() != expected.len() {
            return Err(invalid("Export changed the chart count"));
        }
        let expected_reader = expected_timing_projection(
            self.target,
            self.options.omit_chart_timing,
            &base,
            &expected,
            self.load_context,
            maximum_bytes,
        )?;
        for (chart, (layout, grid, step, _timing)) in expected.iter().enumerate() {
            check(cancelled)?;
            if reopened.document().charts()[chart].steps_type != *layout {
                return Err(invalid("Export changed a chart layout"));
            }
            let actual = reopened.grid(chart)?;
            if grid.columns() != actual.columns()
                || grid.players() != actual.players()
                || semantic_notes(grid, *step)? != semantic_notes(actual, *step)?
            {
                return Err(invalid(
                    "Target reader cannot preserve all note kinds, owners, attributes or declared players",
                ));
            }
            let actual = inspect_timing(&reopened, chart)?;
            let normalized_expected = inspect_timing(&expected_reader, chart)?;
            for (field, value) in &actual {
                if self.target == Format::Sm
                    && matches!(*field, "FAKES" | "LABELS" | "COMBOS" | "SPEEDS" | "SCROLLS")
                {
                    continue;
                }
                let expected = &normalized_expected;
                if expected
                    .iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, v)| v)
                    != Some(value)
                {
                    return Err(invalid(&format!(
                        "Target reader changed effective timing beyond the reviewed output policy: chart {chart}, {field}, expected {:?}, actual {value:?}",
                        expected
                            .iter()
                            .find(|(name, _)| name == field)
                            .map(|(_, value)| value)
                    )));
                }
            }
        }
        if !self.is_current(session) {
            return Err(invalid("Source changed during export generation"));
        }
        Ok(Output {
            bytes,
            format: self.target,
            token: self.token.clone(),
            ids: self.ids.clone(),
            culture: self.order.as_ref().map(|order| order.culture()),
        })
    }
}

/// Interpret an independent projection of the reviewed timing fields through
/// the existing target reader. Raw writer bytes remain source-compatible:
/// notably a null fallback still writes an empty BPMS tag. A reader may insert
/// an initial tempo from earlier-chart histograms or DISPLAYBPM; comparing raw
/// empty strings with its effective clock would reject a faithful export.
fn expected_timing_projection(
    target: Format,
    omit_chart_timing: bool,
    base: &[(&'static str, String)],
    charts: &[(String, Grid, bool, Vec<(&'static str, String)>)],
    context: crate::document::LoadContext,
    maximum: usize,
) -> Result<Session, Error> {
    let mut projection = Buffer {
        bytes: Vec::new(),
        maximum,
    };
    if target == Format::Ssc {
        projection.tag("VERSION", b"0.83")?;
    }
    write_timing(&mut projection, base, target, true)?;
    for (layout, grid, _, timing) in charts {
        // Keep the projection loadable without copying source notes.
        let notes = Grid::new(grid.columns(), grid.players())?.serialize_standard()?;
        if target == Format::Ssc {
            projection.tag("NOTEDATA", b"")?;
            projection.tag("STEPSTYPE", layout.as_bytes())?;
            projection.tag("DIFFICULTY", b"Easy")?;
            projection.tag("METER", b"1")?;
            if !omit_chart_timing {
                write_timing(&mut projection, timing, target, false)?;
            } else {
                for (name, value) in timing
                    .iter()
                    .filter(|(name, _)| matches!(*name, "ATTACKS" | "DISPLAYBPM"))
                {
                    projection.tag(name, value.as_bytes())?;
                }
            }
            projection.tag("NOTES", &notes)?;
        } else {
            projection.push(b"#NOTES:")?;
            projection.push(layout.as_bytes())?;
            projection.push(b"::Easy:1:0:")?;
            projection.push(&notes)?;
            projection.push(b";\n")?;
        }
    }
    Ok(Session::new(Document::parse_with_context(
        projection.bytes,
        target,
        context,
    )?))
}

struct Buffer {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Buffer {
    fn push(&mut self, value: &[u8]) -> Result<(), Error> {
        if self
            .bytes
            .len()
            .checked_add(value.len())
            .is_none_or(|n| n > self.maximum)
        {
            return Err(invalid("Export output byte limit exceeded"));
        }
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    fn tag(&mut self, name: &str, value: &[u8]) -> Result<(), Error> {
        self.push(b"#")?;
        self.push(name.as_bytes())?;
        self.push(b":")?;
        self.push(value)?;
        self.push(b";\n")
    }
}
pub(super) fn chartless_timing(document: &Document) -> Result<Vec<(&'static str, String)>, Error> {
    let offset = document
        .text(None, "OFFSET")
        .map(|value| {
            value
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .ok_or_else(|| invalid("Invalid song music offset"))
        })
        .transpose()?
        .unwrap_or(0.0);
    Ok(vec![
        ("BPMS", String::new()),
        ("STOPS", String::new()),
        ("DELAYS", String::new()),
        ("WARPS", String::new()),
        ("SCROLLS", String::new()),
        ("SPEEDS", String::new()),
        ("TIMESIGNATURES", "0=4=4".into()),
        ("TICKCOUNTS", String::new()),
        ("COMBOS", String::new()),
        ("FAKES", String::new()),
        ("LABELS", String::new()),
        ("ATTACKS", String::new()),
        ("OFFSET", offset.to_string()),
    ])
}
fn write_timing(
    output: &mut Buffer,
    values: &[(&str, String)],
    format: Format,
    song: bool,
) -> Result<(), Error> {
    for (name, value) in values {
        if format == Format::Sm
            && matches!(
                *name,
                "WARPS" | "FAKES" | "LABELS" | "COMBOS" | "SPEEDS" | "SCROLLS"
            )
        {
            continue;
        }
        // EditorSong removes source song DISPLAYBPM for SSC and delegates
        // every display value to the chart writer, including explicit Actual.
        if format == Format::Ssc && song && matches!(*name, "ATTACKS" | "DISPLAYBPM") {
            continue;
        }
        output.tag(name, value.as_bytes())?;
    }
    Ok(())
}
const SONG_FIELDS: &[&str] = &[
    "TITLE",
    "TITLETRANSLIT",
    "SUBTITLE",
    "SUBTITLETRANSLIT",
    "ARTIST",
    "ARTISTTRANSLIT",
    "GENRE",
    "CREDIT",
    "ORIGIN",
    "BANNER",
    "BACKGROUND",
    "JACKET",
    "CDIMAGE",
    "DISCIMAGE",
    "CDTITLE",
    "LYRICSPATH",
    "PREVIEWVID",
    "MUSIC",
    "PREVIEW",
    "SAMPLESTART",
    "SAMPLELENGTH",
    "LASTSECONDHINT",
    "SELECTABLE",
];
const CHART_FIELDS: &[&str] = &[
    "CHARTNAME",
    "CHARTSTYLE",
    "CREDIT",
    "MUSIC",
    "RADARVALUES",
    "SAMPLESTART",
    "SAMPLELENGTH",
    "SELECTABLE",
];
fn write_custom(
    output: &mut Buffer,
    document: &Document,
    chart: Option<usize>,
    output_index: Option<usize>,
    source: Format,
    target: Format,
) -> Result<(), Error> {
    let prefix = if chart.is_some() {
        "GAChartData"
    } else {
        "GASongData"
    };
    for name in [format!("{prefix}Version"), prefix.to_owned()] {
        let source_name = if chart.is_none() {
            crate::song_properties::custom_property_key(document, &name)
        } else if source == Format::Sm {
            format!("{name}CHART{:04}", chart.unwrap())
        } else {
            name.clone()
        };
        let scope = if source == Format::Sm { None } else { chart };
        if let Some(value) = document.raw_property(scope, &source_name) {
            let target_suffix = match (target, output_index) {
                (Format::Sm, Some(index)) => format!("CHART{index:04}"),
                (Format::Sm, None) => "SONG".into(),
                _ => String::new(),
            };
            if chart.is_some() && name == "GAChartData" {
                // This native envelope is not a source-compatible field. Its
                // supported text is emitted above; target SM omissions have
                // already required explicit review.
                let mut data =
                    crate::generation::metadata::ChartData::read(document, chart.unwrap())?
                        .ok_or_else(|| invalid("Missing chart metadata during export"))?;
                if data.extra.remove(crate::chart_text::EXTENSION).is_some() {
                    let json = serde_json::to_string(&data)
                        .map_err(|error| invalid(&error.to_string()))?;
                    output.tag(&format!("{name}{target_suffix}"), &escape_text(&json))?;
                } else {
                    output.tag(&format!("{name}{target_suffix}"), value)?;
                }
            } else {
                output.tag(&format!("{name}{target_suffix}"), value)?;
            }
        }
    }
    Ok(())
}
#[derive(Debug, PartialEq, Eq)]
struct SemanticNote {
    kind: Kind,
    end: Option<u64>,
    head: String,
    tail: String,
}
fn semantic_notes(grid: &Grid, step: bool) -> Result<BTreeMap<Position, SemanticNote>, Error> {
    grid.iter()
        .map(|(position, note)| {
            Ok((
                position,
                SemanticNote {
                    kind: note.kind,
                    end: note.end,
                    head: decoration(note, &note.attributes, step, true)?,
                    tail: decoration(note, &note.tail_attributes, step, false)?,
                },
            ))
        })
        .collect()
}
fn decoration(note: &Note, raw: &str, step: bool, head: bool) -> Result<String, Error> {
    if !step {
        return Ok(raw.into());
    }
    let (attributes, trailing) = Attributes::read(raw)?.unwrap_or((
        Attributes {
            fake: head && note.kind == Kind::Fake,
            ..Default::default()
        },
        raw,
    ));
    attributes.suffix('N', trailing)
}

fn require_scope_mappings(
    document: &Document,
    _target: Format,
    _anonymize: bool,
) -> Result<(), Error> {
    for tag in document.tags() {
        let name = tag.name.as_str();
        if tag.chart.is_none() && name.eq_ignore_ascii_case(crate::song_text::resident::TAG) {
            crate::song_text::resident::read(document)?;
            continue;
        }
        if is_native_timing_metadata(document, name)
            || is_custom(name)
            || timing_edit::TIMING_TAGS.contains(&name)
            || matches!(name, "ATTACKS" | "DISPLAYBPM")
        {
            continue;
        }
        let structural = matches!(
            name,
            "NOTES" | "NOTES2" | "NOTEDATA" | "STEPSTYPE" | "DESCRIPTION" | "DIFFICULTY" | "METER"
        );
        if structural {
            if tag.chart.is_none() {
                return Err(invalid("Chart structural property occurs outside a chart"));
            }
            continue;
        }
        if tag.chart.is_none() && (SONG_FIELDS.contains(&name) || name == "VERSION") {
            continue;
        }
        if tag.chart.is_some() && CHART_FIELDS.contains(&name) {
            // Known SSC-only metadata has an explicit preflight warning.
            // Plan::generate validates all acknowledgements before reaching here.
            continue;
        }
        return Err(invalid(
            "Source property scope has no proven target mapping",
        ));
    }
    Ok(())
}
