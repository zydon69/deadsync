//! Reviewed activation of legacy SSC chart timing. The version and all required
//! tempo materialization are committed together, without touching note data.
pub use crate::history::song_presentation::PresentationOrigin;
use super::{inherited_fields_for_write, reference_uses_chart_timing, supports_chart_timing};
use crate::{
    Error,
    document::{Document, Format, escape_text},
    generation::timing::{EventKind, GenerationTiming},
    history::{RevisionToken, Session},
};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChartChange {
    pub chart: usize,
    pub previously_local: bool,
    pub materializes_tempo: bool,
}
pub fn needed(document: &Document) -> bool {
    document.format == Format::Ssc && !supports_chart_timing(document)
}
pub fn has_legacy_chart_timing(document: &Document) -> bool {
    needed(document)
        && (0..document.charts().len()).any(|chart| reference_uses_chart_timing(document, chart))
}
pub struct Draft {
    token: RevisionToken,
    previous: Option<String>,
    version: String,
    charts: Vec<ChartChange>,
    properties: Vec<(Option<usize>, String, Arc<[u8]>)>,
}
impl Draft {
    pub fn capture(session: &Session) -> Result<Self, Error> {
        let document = session.document();
        if document.format != Format::Ssc {
            return Err(Error::Unsupported(
                "SSC timing migration requires an SSC file".into(),
            ));
        }
        let parsed = rssp::parse::parse_version(
            document.raw_property(None, "VERSION"),
            rssp::timing::TimingFormat::Ssc,
        );
        // Do not cross the 0.74 chart-name interpretation boundary for known
        // old versions. Unknown/missing versions already use modern name rules.
        let version = if !needed(document) {
            document
                .text(None, "VERSION")
                .unwrap_or_else(|| "0.83".into())
        } else if parsed.is_finite() {
            "0.70".into()
        } else {
            "0.83".into()
        };
        let mut draft = Self {
            token: session.revision_token(),
            previous: document.text(None, "VERSION"),
            version,
            charts: Vec::new(),
            properties: Vec::new(),
        };
        if !needed(document) {
            return Ok(draft);
        }
        draft
            .properties
            .push((None, "VERSION".into(), Arc::from(draft.version.as_bytes())));
        for chart in 0..document.charts().len() {
            let timing = GenerationTiming::from_document(document, chart)?;
            let tempos = |events: &[crate::generation::timing::Event]| {
                events
                    .iter()
                    .filter_map(|event| {
                        if let EventKind::Tempo(value) = event.kind {
                            Some((event.row, value))
                        } else {
                            None
                        }
                    })
                    .collect::<BTreeMap<_, _>>()
            };
            let editor = tempos(timing.editor_events());
            let file = tempos(timing.file_events());
            let own = reference_uses_chart_timing(document, chart);
            let materialize = own || editor != file;
            let mut tempo_changed = false;
            if materialize {
                if !own {
                    for (tag, value) in inherited_fields_for_write(document, chart, &["BPMS"])? {
                        draft.properties.push((Some(chart), tag, value));
                    }
                }
                let value = editor
                    .iter()
                    .map(|(row, bpm)| format!("{}={bpm}", *row as f64 / 48.0))
                    .collect::<Vec<_>>()
                    .join(",");
                let raw = escape_text(&value);
                if document.raw_property(Some(chart), "BPMS") != Some(raw.as_slice()) {
                    tempo_changed = true;
                    draft
                        .properties
                        .push((Some(chart), "BPMS".into(), Arc::from(raw)));
                }
            }
            if own || materialize {
                draft.charts.push(ChartChange {
                    chart,
                    previously_local: own,
                    materializes_tempo: tempo_changed,
                });
            }
        }
        Ok(draft)
    }
    pub fn previous_version(&self) -> Option<&str> {
        self.previous.as_deref()
    }
    pub fn target_version(&self) -> &str {
        &self.version
    }
    pub fn charts(&self) -> &[ChartChange] {
        &self.charts
    }
    pub fn has_changes(&self) -> bool {
        !self.properties.is_empty()
    }
    pub fn apply(&self, session: &mut Session) -> Result<bool, Error> {
        if !session.accepts_token(&self.token) {
            return Err(Error::InvalidEdit(
                "SSC migration belongs to another document or revision".into(),
            ));
        }
        self.validate_presentation(session)?;
        session.set_timing_migration_properties(&self.properties)
    }
    pub fn apply_with_presentation_receipt(&self, session: &mut Session)
        -> Result<crate::song_properties::Application, Error>
    {
        crate::history::song_presentation::apply_timing_migration(self, session)
    }
    // Migration activates the editor's already-effective legacy clock in the
    // file reader. Validate that invariant before touching the real history.
    fn validate_presentation(&self, session: &Session) -> Result<(), Error> {
        let mut document = session.document().clone();
        for (scope, name, value) in &self.properties {
            if let Some(tag) = document.tag(*scope, name) {
                document.replace(tag.value.clone(), Some(Arc::clone(value)))?;
            } else {
                document.set_added_property(*scope, name, Some(Arc::clone(value)))?;
            }
        }
        let candidate = Session::new(document);
        for chart in 0..session.document().charts().len() {
            let before = GenerationTiming::from_document(session.document(), chart)?;
            let after = GenerationTiming::from_document(candidate.document(), chart)?;
            let before_offset = crate::chart_properties::Draft::capture(session, chart)?.effective_music_offset();
            let after_offset = crate::chart_properties::Draft::capture(&candidate, chart)?.effective_music_offset();
            if before.editor_events() != after.editor_events()
                || before_offset.to_bits() != after_offset.to_bits()
                || crate::song_properties::sync_offset(session.document())?.to_bits()
                    != crate::song_properties::sync_offset(candidate.document())?.to_bits()
            {
                return Err(Error::InvalidEdit("SSC migration would change the editor's effective clock".into()));
            }
        }
        Ok(())
    }
}
/// Generation must not emit new chart timing that gameplay would still ignore.
pub(crate) fn require_compatible(document: &Document) -> Result<(), Error> {
    if has_legacy_chart_timing(document) {
        return Err(Error::Unsupported(
            "Legacy SSC chart timing must be converted in F10 before editing or generation".into(),
        ));
    }
    Ok(())
}
