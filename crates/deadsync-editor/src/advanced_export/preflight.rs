mod chart_metadata;
mod writer;
pub use writer::Output;
use super::Options;
use crate::{
    Error,
    document::{Document, Format},
    generation::metadata::{ChartData, Guid},
    history::{ContentToken, Session},
    notes::{Grid, Kind, Position},
    timing_edit::{self, Scope},
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Warning,
    Error,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    UnsupportedChart,
    InvalidChart,
    UnequalTiming,
    UnequalSharedEvent,
    OmittedEvent,
    OmittedChartMetadata,
    OmittedCustomData,
    AnonymizedField,
    StepF2Players,
    StepF2NoteKind,
    StepF2MineOwner,
    StepF2Layout,
    StepF2Collision,
    StepF2Attributes,
    UnknownProperty,
    ChartAttacksWithoutSplitTiming,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub chart: Option<usize>,
    pub identity: Option<Guid>,
    pub severity: Severity,
    pub kind: IssueKind,
    pub field: String,
    pub position: Option<Position>,
    pub detail: String,
}
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub bytes: usize,
    pub charts: usize,
    pub notes: usize,
    pub issues: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            bytes: 128 * 1024 * 1024,
            charts: 4096,
            notes: 2_000_000,
            issues: 100_000,
        }
    }
}
/// An immutable source snapshot and complete bounded review, not a writer receipt.
pub struct Plan {
    token: ContentToken,
    source: Arc<[u8]>,
    source_format: Format,
    target: Format,
    timing_chart: usize,
    options: Options,
    ids: Vec<Guid>,
    issues: Vec<Issue>,
}
impl Plan {
    pub fn prepare(
        session: &mut Session,
        target: Format,
        timing_chart: usize,
        options: Options,
        limits: Limits,
        cancelled: &AtomicBool,
    ) -> Result<Self, Error> {
        check(cancelled)?;
        if session.document().charts().len() > limits.charts {
            return Err(invalid("Export chart limit exceeded"));
        }
        if timing_chart >= session.document().charts().len() {
            return Err(invalid("Choose an existing timing chart"));
        }
        // Reject an oversized immutable source before any export-sized clone.
        // Pending serializers retain their own established allocation guards;
        // the final byte cap below is not a peak-memory guarantee.
        if session.document().source().len() > limits.bytes {
            return Err(invalid("Export input source byte limit exceeded"));
        }
        let mut resident_heads = 0usize;
        for chart in 0..session.document().charts().len() {
            check(cancelled)?;
            if let Some(grid) = session.loaded_grid(chart) {
                for _ in grid.iter() {
                    resident_heads = resident_heads
                        .checked_add(1)
                        .ok_or_else(|| invalid("Export note count overflow"))?;
                    if resident_heads > limits.notes {
                        return Err(invalid("Export resident note limit exceeded"));
                    }
                    if resident_heads % 1024 == 0 {
                        check(cancelled)?;
                    }
                }
            }
        }
        check(cancelled)?;
        let bytes = session.serialize()?;
        check(cancelled)?;
        if bytes.len() > limits.bytes {
            return Err(invalid("Export source byte limit exceeded"));
        }
        let mut plan = Self {
            token: session.content_token(),
            source: Arc::from(bytes),
            source_format: session.document().format,
            target,
            timing_chart,
            options,
            ids: session.chart_ids().to_vec(),
            issues: Vec::new(),
        };
        // Serialize pending grids first, then use the single existing Document
        // tokenizer. The original live Document/ranges/history stay untouched.
        let snapshot = Document::parse(plan.source.clone(), plan.source_format)?;
        let mut staged = Session::new(snapshot);
        let mut signatures = Vec::new();
        let mut count = 0usize;
        for chart in 0..staged.document().charts().len() {
            check(cancelled)?;
            let layout = staged.document().charts()[chart].steps_type.clone();
            if crate::notes::layout_columns(&layout).is_none() {
                plan.add(
                    Some(chart),
                    Severity::Error,
                    IssueKind::UnsupportedChart,
                    "STEPSTYPE",
                    None,
                    "No lossless native exporter for this chart layout",
                    limits,
                )?;
                signatures.push(None);
                continue;
            }
            match inspect_timing(&staged, chart) {
                Ok(signature) => signatures.push(Some(signature)),
                Err(error) => {
                    plan.add(
                        Some(chart),
                        Severity::Error,
                        IssueKind::InvalidChart,
                        "TIMING",
                        None,
                        &error.to_string(),
                        limits,
                    )?;
                    signatures.push(None);
                }
            }
            let grid = match staged.grid(chart) {
                Ok(grid) => grid,
                Err(error) => {
                    plan.add(
                        Some(chart),
                        Severity::Error,
                        IssueKind::InvalidChart,
                        "NOTES",
                        None,
                        &error.to_string(),
                        limits,
                    )?;
                    continue;
                }
            };
            plan.inspect_grid(chart, &layout, grid, &mut count, limits, cancelled)?;
            plan.inspect_properties(staged.document(), chart, limits)?;
        }
        // EditorSong's writer deliberately omits song ATTACKS for SSC. Once
        // chart timing is omitted too, StepMania ignores these chart attacks
        // even though the editor's own reader can still recover the tags.
        if target == Format::Ssc && options.omit_chart_timing {
            for (chart, signature) in signatures.iter().enumerate() {
                if signature.as_ref().is_some_and(|fields|fields.iter().any(|(name,value)|*name=="ATTACKS"&&!value.is_empty())) {
                    plan.add(Some(chart),Severity::Warning,IssueKind::ChartAttacksWithoutSplitTiming,"ATTACKS",None,"Chart attacks are ignored by StepMania without split timing; the editor's tag round-trip does not preserve their effect in game",limits)?;
                }
            }
        }
        if let Some(Some(base)) = signatures.get(timing_chart) {
            for (chart, signature) in signatures.iter().enumerate() {
                check(cancelled)?;
                if chart == timing_chart {
                    continue;
                }
                let Some(signature) = signature else {
                    continue;
                };
                for ((field, value), (_, expected)) in signature.iter().zip(base) {
                    if value == expected {
                        continue;
                    }
                    let timing = TIMING.contains(field);
                    let shared = if target == Format::Sm {
                        timing || matches!(*field, "ATTACKS" | "TICKCOUNTS")
                    } else {
                        options.omit_chart_timing && *field != "ATTACKS"
                    };
                    if !shared {
                        continue;
                    }
                    let severity = if target == Format::Ssc
                        || (timing && options.require_identical_timing_in_sm)
                    {
                        Severity::Error
                    } else {
                        Severity::Warning
                    };
                    plan.add(
                        Some(chart),
                        severity,
                        if timing {
                            IssueKind::UnequalTiming
                        } else {
                            IssueKind::UnequalSharedEvent
                        },
                        field,
                        None,
                        &format!(
                            "Differs from timing chart {}; output would use its value",
                            timing_chart + 1
                        ),
                        limits,
                    )?;
                }
            }
        }
        if options.anonymize {
            for field in SONG_ANON {
                if staged
                    .document()
                    .text(None, field)
                    .is_some_and(|v| !v.is_empty())
                {
                    plan.add(
                        None,
                        Severity::Warning,
                        IssueKind::AnonymizedField,
                        field,
                        None,
                        "Removed by explicit anonymize option",
                        limits,
                    )?;
                }
            }
        }
        plan.inspect_unknown(staged.document(), limits)?;
        check(cancelled)?;
        Ok(plan)
    }
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }
    pub fn source_bytes(&self) -> &[u8] {
        &self.source
    }
    pub fn source_format(&self) -> Format {
        self.source_format
    }
    pub fn target_format(&self) -> Format {
        self.target
    }
    pub fn options(&self) -> Options {
        self.options
    }
    pub fn timing_chart(&self) -> usize {
        self.timing_chart
    }
    pub fn is_current(&self, session: &Session) -> bool {
        session.accepts_content_token(&self.token) && session.chart_ids() == self.ids.as_slice()
    }
    /// Generation is permitted only after every concrete warning is reviewed.
    /// This does not publish anything or mark the source saved.
    pub fn validate_review(
        &self,
        session: &Session,
        acknowledged: &BTreeSet<usize>,
    ) -> Result<(), Error> {
        if !self.is_current(session) {
            return Err(invalid(
                "Export review belongs to another session or content state",
            ));
        }
        if self.issues.iter().any(|i| i.severity == Severity::Error) {
            return Err(invalid("Export has blocking compatibility errors"));
        }
        if acknowledged.iter().any(|i| *i >= self.issues.len())
            || self
                .issues
                .iter()
                .enumerate()
                .any(|(i, issue)| issue.severity == Severity::Warning && !acknowledged.contains(&i))
        {
            return Err(invalid(
                "Review every reported output loss before generation",
            ));
        }
        Ok(())
    }
    fn add(
        &mut self,
        chart: Option<usize>,
        severity: Severity,
        kind: IssueKind,
        field: &str,
        position: Option<Position>,
        detail: &str,
        limits: Limits,
    ) -> Result<(), Error> {
        if self.issues.len() >= limits.issues {
            return Err(invalid("Export review issue limit exceeded"));
        }
        self.issues.push(Issue {
            chart,
            identity: chart.map(|i| self.ids[i].clone()),
            severity,
            kind,
            field: field.into(),
            position,
            detail: detail.into(),
        });
        Ok(())
    }
    fn inspect_grid(
        &mut self,
        chart: usize,
        layout: &str,
        grid: &Grid,
        count: &mut usize,
        limits: Limits,
        cancelled: &AtomicBool,
    ) -> Result<(), Error> {
        let step = self.options.stepf2_pump_multiplayer
            && layout.starts_with("pump-")
            && grid.players() > 1;
        if step && grid.players() > 4 {
            self.add(
                Some(chart),
                Severity::Error,
                IssueKind::StepF2Players,
                "NOTES",
                None,
                "StepF2 supports at most four owners",
                limits,
            )?;
        }
        if step && layout != "pump-routine" {
            self.add(Some(chart),Severity::Error,IssueKind::StepF2Layout,"STEPSTYPE",None,"Target readers do not safely decode StepF2 in this Pump layout; normalize explicitly to Routine first",limits)?;
        }
        let mut occupied =
            std::collections::BTreeMap::<u16, std::collections::BTreeMap<u64, u64>>::new();
        for (position, note) in grid.iter() {
            *count = count
                .checked_add(1)
                .ok_or_else(|| invalid("Export note count overflow"))?;
            if *count > limits.notes {
                return Err(invalid("Export note limit exceeded"));
            }
            if *count % 1024 == 0 {
                check(cancelled)?;
            }
            if !step {
                if let Err(error) = crate::stepf2::require_standard(note) {
                    self.add(
                        Some(chart),
                        Severity::Error,
                        IssueKind::StepF2Attributes,
                        "NOTES",
                        Some(position),
                        &error.to_string(),
                        limits,
                    )?;
                }
                continue;
            }
            if matches!(
                note.kind,
                Kind::Roll | Kind::Lift | Kind::Keysound | Kind::Attack
            ) {
                self.add(
                    Some(chart),
                    Severity::Error,
                    IssueKind::StepF2NoteKind,
                    "NOTES",
                    Some(position),
                    "This note kind is not representable in StepF2",
                    limits,
                )?;
            }
            if note.kind == Kind::Mine && position.player > 0 {
                self.add(
                    Some(chart),
                    Severity::Warning,
                    IssueKind::StepF2MineOwner,
                    "NOTES",
                    Some(position),
                    "StepF2 would assign this mine to P1",
                    limits,
                )?;
            }
            if let Err(error) = crate::stepf2::validate_attributes(note) {
                self.add(
                    Some(chart),
                    Severity::Error,
                    IssueKind::StepF2Attributes,
                    "NOTES",
                    Some(position),
                    &error.to_string(),
                    limits,
                )?;
            }
            let end = note.end.unwrap_or(position.row);
            let lane = occupied.entry(position.lane).or_default();
            let intersects = lane
                .range(..=position.row)
                .next_back()
                .is_some_and(|(_, last)| *last >= position.row)
                || lane
                    .range(position.row..)
                    .next()
                    .is_some_and(|(first, _)| *first <= end);
            if intersects {
                self.add(
                    Some(chart),
                    Severity::Error,
                    IssueKind::StepF2Collision,
                    "NOTES",
                    Some(position),
                    "Owners overlap in the shared StepF2 lane stream",
                    limits,
                )?;
            }
            lane.entry(position.row)
                .and_modify(|old| *old = (*old).max(end))
                .or_insert(end);
        }
        Ok(())
    }
    fn inspect_properties(
        &mut self,
        document: &Document,
        chart: usize,
        limits: Limits,
    ) -> Result<(), Error> {
        self.inspect_chart_metadata(document, chart, limits)?;
        let origin = timing_edit::uses_chart_timing(document, chart).then_some(chart);
        let raw = |field: &str| document.text(origin, field).unwrap_or_default();
        if self.target == Format::Sm {
            let warps = timing_edit::region::Regions::parse(
                timing_edit::region::Kind::Warp,
                &raw("WARPS"),
            )?;
            if !warps.values().is_empty() {
                self.add(
                    Some(chart),
                    Severity::Error,
                    IssueKind::OmittedEvent,
                    "WARPS",
                    None,
                    "SM ignores Warps; convert timing explicitly before export",
                    limits,
                )?;
            }
            let fake = timing_edit::region::Regions::parse(
                timing_edit::region::Kind::Fake,
                &raw("FAKES"),
            )?;
            let labels = timing_edit::label::Labels::parse(&raw("LABELS"))?;
            let combo = timing_edit::combo::Combos::parse(&raw("COMBOS"))?;
            let scroll = timing_edit::scroll::Scrolls::parse(&raw("SCROLLS"))?;
            let speed = timing_edit::speed::Speeds::parse(&raw("SPEEDS"))?;
            let combo_loss = combo
                .values()
                .iter()
                .any(|(&row, v)| row != 0 || *v != Default::default());
            let scroll_loss = scroll
                .values()
                .iter()
                .any(|(&row, &v)| row != 0 || v != 1.0);
            let speed_loss = speed
                .values()
                .iter()
                .any(|(&row, v)| row != 0 || *v != Default::default());
            for (field, loss) in [
                ("FAKES", !fake.values().is_empty()),
                ("LABELS", !labels.values().is_empty()),
                ("COMBOS", combo_loss),
                ("SCROLLS", scroll_loss),
                ("SPEEDS", speed_loss),
            ] {
                if loss {
                    self.add(
                        Some(chart),
                        Severity::Warning,
                        IssueKind::OmittedEvent,
                        field,
                        None,
                        "SM output cannot represent these events",
                        limits,
                    )?;
                }
            }
            if chart == self.timing_chart
                && document
                    .text(None, "LASTSECONDHINT")
                    .and_then(|v| v.parse::<f64>().ok())
                    .is_some_and(|v| v > 0.0)
            {
                self.add(
                    None,
                    Severity::Warning,
                    IssueKind::OmittedEvent,
                    "LASTSECONDHINT",
                    None,
                    "SM omits the last second hint",
                    limits,
                )?;
            }
        } else {
            let stops =
                timing_edit::pause::Pauses::parse(timing_edit::pause::Kind::Stop, &raw("STOPS"))?;
            if stops.values().values().any(|v| *v < 0.0) {
                self.add(
                    Some(chart),
                    Severity::Error,
                    IssueKind::OmittedEvent,
                    "STOPS",
                    None,
                    "SSC ignores negative stops; convert timing explicitly before export",
                    limits,
                )?;
            }
        }
        if self.options.omit_custom_data {
            if let Some(data) = ChartData::read(document, chart)? {
                if data.patterns.as_ref().is_some_and(|p| !p.is_empty()) {
                    self.add(
                        Some(chart),
                        Severity::Warning,
                        IssueKind::OmittedCustomData,
                        "Patterns",
                        None,
                        "Patterns are stored only in custom chart metadata",
                        limits,
                    )?;
                }
            }
            // The entire custom payload is omitted, not just known Patterns.
            if document
                .tags()
                .iter()
                .any(|t| t.chart == Some(chart) && is_custom(&t.name))
            {
                self.add(
                    Some(chart),
                    Severity::Warning,
                    IssueKind::OmittedCustomData,
                    "GA metadata",
                    None,
                    "All GrooveAuthor chart custom metadata will be omitted",
                    limits,
                )?;
            }
        }
        if self.options.anonymize {
            for field in ["DESCRIPTION", "CREDIT", "CHARTNAME", "CHARTSTYLE"] {
                if document
                    .text(Some(chart), field)
                    .is_some_and(|v| !v.is_empty())
                    || field == "DESCRIPTION" && !document.charts()[chart].description.is_empty()
                {
                    self.add(
                        Some(chart),
                        Severity::Warning,
                        IssueKind::AnonymizedField,
                        field,
                        None,
                        "Removed by explicit anonymize option",
                        limits,
                    )?;
                }
            }
        }
        Ok(())
    }
    fn inspect_unknown(&mut self, document: &Document, limits: Limits) -> Result<(), Error> {
        let mut seen = BTreeSet::new();
        for tag in document.tags() {
            if KNOWN.contains(&tag.name.as_str())
                || timing_edit::TIMING_TAGS.contains(&tag.name.as_str())
            {
                continue;
            }
            if !seen.insert((tag.chart, tag.name.clone())) {
                continue;
            }
            if is_custom(&tag.name) {
                if self.options.omit_custom_data {
                    self.add(
                        tag.chart,
                        Severity::Warning,
                        IssueKind::OmittedCustomData,
                        &tag.name,
                        None,
                        "Custom editor metadata will be omitted",
                        limits,
                    )?;
                }
            } else {
                self.add(tag.chart,Severity::Error,IssueKind::UnknownProperty,&tag.name,None,"Unknown property has no proven cross-format mapping; preserve original Save or implement an explicit mapping",limits)?;
            }
        }
        Ok(())
    }
}
const TIMING: &[&str] = &[
    "BPMS",
    "STOPS",
    "DELAYS",
    "TIMESIGNATURES",
    "WARPS",
    "SCROLLS",
    "SPEEDS",
    "OFFSET",
    "DISPLAYBPM",
];
const SONG_ANON: &[&str] = &[
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
];
const KNOWN: &[&str] = &[
    "VERSION",
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
    "DISPLAYBPM",
    "ATTACKS",
    "NOTEDATA",
    "STEPSTYPE",
    "DESCRIPTION",
    "CHARTNAME",
    "CHARTSTYLE",
    "DIFFICULTY",
    "METER",
    "RADARVALUES",
    "NOTES",
    "NOTES2",
];
fn inspect_timing(session: &Session, chart: usize) -> Result<Vec<(&'static str, String)>, Error> {
    use timing_edit::{combo, label, pause, region, scroll, signature, speed, tick};
    // Validate the existing reference decision clock in addition to typed fields.
    crate::generation::timing::GenerationTiming::from_document(session.document(), chart)?;
    let scope = if timing_edit::supports_chart_timing(session.document()) {
        Scope::Chart
    } else {
        Scope::Song
    };
    let origin = timing_edit::uses_chart_timing(session.document(), chart).then_some(chart);
    let text = |name: &str| session.document().text(origin, name).unwrap_or_default();
    let properties = crate::chart_properties::Draft::capture(session, chart)?;
    let attack = crate::attacks::Draft::capture(session, chart, scope)?
        .attacks()
        .serialize()?;
    Ok(vec![
        (
            "BPMS",
            timing_edit::TempoDraft::capture(session, chart, scope)?
                .tempos()
                .serialize(),
        ),
        (
            "STOPS",
            pause::Pauses::parse(pause::Kind::Stop, &text("STOPS"))?.serialize(),
        ),
        (
            "DELAYS",
            pause::Pauses::parse(pause::Kind::Delay, &text("DELAYS"))?.serialize(),
        ),
        (
            "TIMESIGNATURES",
            signature::Signatures::parse(&text("TIMESIGNATURES"))?.serialize(),
        ),
        (
            "WARPS",
            region::Regions::parse(region::Kind::Warp, &text("WARPS"))?.serialize(),
        ),
        (
            "SCROLLS",
            scroll::Scrolls::parse(&text("SCROLLS"))?.serialize(),
        ),
        ("SPEEDS", speed::Speeds::parse(&text("SPEEDS"))?.serialize()),
        ("OFFSET", properties.effective_music_offset().to_string()),
        ("DISPLAYBPM", properties.display_tempo().serialize()),
        (
            "TICKCOUNTS",
            tick::TickCounts::parse(&text("TICKCOUNTS"))?.serialize(),
        ),
        ("COMBOS", combo::Combos::parse(&text("COMBOS"))?.serialize()),
        (
            "FAKES",
            region::Regions::parse(region::Kind::Fake, &text("FAKES"))?.serialize(),
        ),
        ("LABELS", label::Labels::parse(&text("LABELS"))?.serialize()),
        (
            "ATTACKS",
            String::from_utf8(attack).map_err(|_| invalid("Non UTF-8 attacks"))?,
        ),
    ])
}
fn check(cancelled: &AtomicBool) -> Result<(), Error> {
    if cancelled.load(Ordering::Relaxed) {
        Err(invalid("Export preparation cancelled"))
    } else {
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}

fn is_custom(name: &str) -> bool {
    [
        "GASONGDATA",
        "GASONGDATAVERSION",
        "GACHARTDATA",
        "GACHARTDATAVERSION",
    ]
    .into_iter()
    .any(|prefix| {
        name.strip_prefix(prefix).is_some_and(|suffix| {
            suffix.is_empty()
                || suffix == "SONG"
                || suffix.strip_prefix("CHART").is_some_and(|index| {
                    !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit())
                })
        })
    })
}
