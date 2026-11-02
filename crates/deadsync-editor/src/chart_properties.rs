//! Chart-wide editor properties, independent of the existence of pattern markers.
pub use crate::history::song_presentation::{Application, PresentationReceipt};
use crate::{
    Error,
    document::Format,
    generation::{
        metadata::{ChartData, Guid, music_offsets_equal},
        profile_catalog::{self, Catalog, ExpressionSelection},
    },
    history::{RevisionToken, Session},
};
use std::path::Path;
pub struct Draft {
    token: RevisionToken,
    chart: usize,
    original_expression: Guid,
    original_offset: f64,
    original_use_offset: bool,
    expression: Guid,
    music_offset: f64,
    use_chart_offset: bool,
    song_offset: f64,
    selection: Option<ExpressionSelection>,
    format: Format,
    identity: crate::chart_identity::Identity,
    text: crate::chart_text::Text,
    original_display_tempo: crate::display_tempo::Value,
    display_tempo: crate::display_tempo::Value,
}
impl Draft {
    /// Preserve pending values across the immediately following disk-only copy creation.
    pub fn new_profile_plan(
        &self,
        session: &Session,
    ) -> Result<crate::history::profile_creation::Plan, Error> {
        self.check(session, self.chart)?;
        if let Some(selection) = &self.selection {
            selection.check(&self.expression)?;
        }
        crate::history::profile_creation::Plan::chart(session, self.chart)
    }
    pub fn after_inline_profile_creation(
        &mut self,
        session: &Session,
        receipt: &crate::history::profile_creation::Committed,
    ) -> Result<(), Error> {
        let guid = receipt.rebind(session, &self.token, self.chart, None)?;
        self.original_expression = guid.clone();
        self.expression = guid;
        if let Some(selection) = self.selection.as_mut() {
            selection.after_inline_creation(receipt);
        }
        self.token = session.revision_token();
        Ok(())
    }
    pub fn after_profile_history(&mut self, session: &Session, redo: bool) -> Result<(), Error> {
        self.token = session.after_profile_history(&self.token, redo)?;
        Ok(())
    }

    pub fn after_profile_creation(&mut self, session: &Session) -> Result<(), Error> {
        self.token = session.after_profile_creation(&self.token)?;
        Ok(())
    }

    pub fn capture(session: &Session, chart: usize) -> Result<Self, Error> {
        let data = ChartData::existing_or_initial(session.document(), chart)?;
        let song_offset = session
            .document()
            .text(None, "OFFSET")
            .map(|text| {
                text.trim()
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite())
                    .ok_or_else(|| invalid("Invalid song music offset"))
            })
            .transpose()?
            .unwrap_or(0.0);
        let document = session.document();
        let display_text = if document.format == Format::Sm
            && crate::timing_edit::resident::has_local(document, chart)
        {
            // The same resident snapshot owns performance timing and display
            // tempo. An empty local DISPLAYBPM is explicit Actual, not fallback.
            crate::timing_edit::resident::Properties::capture(document, Some(chart))?
                .text("DISPLAYBPM")
        } else {
            document
                .text(
                    (document.format == Format::Ssc).then_some(chart),
                    "DISPLAYBPM",
                )
                .or_else(|| document.text(None, "DISPLAYBPM"))
                .unwrap_or_default()
        };
        let display_tempo = crate::display_tempo::Value::parse(&display_text);
        Ok(Self {
            original_display_tempo: display_tempo.clone(),
            display_tempo,
            token: session.revision_token(),
            chart,
            original_expression: data.expressed_chart_config.clone(),
            original_offset: data.music_offset,
            original_use_offset: data.should_use_chart_music_offset,
            expression: data.expressed_chart_config,
            music_offset: data.music_offset,
            use_chart_offset: data.should_use_chart_music_offset,
            song_offset,
            selection: None,
            format: session.document().format,
            identity: crate::chart_identity::Identity::read(session.document(), chart)?,
            text: crate::chart_text::Text::read(session.document(), chart)?,
        })
    }
    pub fn actual_display_tempo_range(
        &self,
        session: &Session,
        chart: usize,
    ) -> Result<(f64, f64), Error> {
        self.check(session, chart)?;
        let timing =
            crate::generation::timing::GenerationTiming::from_document(session.document(), chart)?;
        let mut range: Option<(f64, f64)> = None;
        for event in timing.editor_events() {
            if let crate::generation::timing::EventKind::Tempo(value) = event.kind {
                range = Some(range.map_or((value, value), |(min, max)| {
                    (min.min(value), max.max(value))
                }));
            }
        }
        range.ok_or_else(|| invalid("Chart has no tempo events"))
    }
    pub fn display_tempo(&self) -> &crate::display_tempo::Value {
        &self.display_tempo
    }
    pub fn set_display_tempo(&mut self, value: crate::display_tempo::Value) {
        self.display_tempo = value;
    }
    fn display_tempo_changed(&self) -> bool {
        !self
            .display_tempo
            .same_serialized_value(&self.original_display_tempo)
    }
    pub fn text(&self, field: crate::chart_text::Field) -> &str {
        self.text.get(field)
    }
    pub fn supports_text(&self, field: crate::chart_text::Field) -> bool {
        self.text.supports(field)
    }
    pub fn set_text(&mut self, field: crate::chart_text::Field, value: &str) -> Result<(), Error> {
        self.text.set(field, value)
    }
    pub fn difficulty(&self) -> Option<crate::chart_identity::Difficulty> {
        self.identity.difficulty()
    }
    pub fn rating(&self) -> i32 {
        self.identity.rating()
    }
    pub fn set_difficulty(&mut self, value: crate::chart_identity::Difficulty) {
        self.identity.set_difficulty(value);
    }
    pub fn set_rating(&mut self, value: i32) -> Result<(), Error> {
        self.identity.set_rating(value)
    }
    pub fn format(&self) -> Format {
        self.format
    }
    pub fn expression(&self) -> &Guid {
        &self.expression
    }
    pub fn music_offset(&self) -> f64 {
        self.music_offset
    }
    pub fn uses_chart_offset(&self) -> bool {
        self.use_chart_offset
    }
    pub fn effective_music_offset(&self) -> f64 {
        if self.use_chart_offset {
            self.music_offset
        } else {
            self.song_offset
        }
    }
    pub fn set_music_offset(&mut self, value: f64) -> Result<(), Error> {
        if !value.is_finite() {
            return Err(invalid("Music offset must be finite"));
        }
        if !music_offsets_equal(value, self.music_offset) {
            self.music_offset = value;
        }
        Ok(())
    }
    pub fn set_use_chart_offset(&mut self, value: bool) {
        self.use_chart_offset = value;
    }
    fn offset_changed(&self) -> bool {
        self.music_offset != self.original_offset
            || self.use_chart_offset != self.original_use_offset
    }
    pub fn has_changes(&self) -> bool {
        self.offset_changed()
            || self.display_tempo_changed()
            || self.expression != self.original_expression
            || self.identity.has_changes()
            || self.text.has_changes()
    }
    fn check(&self, session: &Session, chart: usize) -> Result<(), Error> {
        if chart != self.chart || !session.accepts_token(&self.token) {
            return Err(invalid(
                "Chart properties belong to another chart or revision",
            ));
        }
        Ok(())
    }
    pub fn profile_task(
        &self,
        session: &Session,
        chart: usize,
        directory: Option<&Path>,
    ) -> Result<profile_catalog::Task, Error> {
        self.check(session, chart)?;
        profile_catalog::Task::capture(session, chart, directory)
    }
    pub fn select_expression(
        &mut self,
        session: &Session,
        chart: usize,
        catalog: &Catalog,
        expression: &Guid,
    ) -> Result<(), Error> {
        self.check(session, chart)?;
        let selection = catalog.select_expression(session, chart, expression)?;
        self.expression = expression.clone();
        self.selection = Some(selection);
        Ok(())
    }
    pub fn apply_with_presentation_receipt(
        &self,
        session: &mut Session,
        chart: usize,
    ) -> Result<Application, Error> {
        crate::history::song_presentation::apply_chart(self, session, chart)
    }
    pub(crate) fn preserves_presentation_clock(&self) -> bool {
        let old = if self.original_use_offset {
            self.original_offset
        } else {
            self.song_offset
        };
        old.to_bits() == self.effective_music_offset().to_bits()
    }
    /// Whether the effective chart MUSIC binding remains unchanged.
    pub fn preserves_presentation_audio(&self, document: &crate::document::Document) -> bool {
        let inherited = document.text(None, "MUSIC").unwrap_or_default();
        let old = self.text.original(crate::chart_text::Field::Music);
        let new = self.text.get(crate::chart_text::Field::Music);
        let old = if old.is_empty() {
            inherited.as_str()
        } else {
            old
        };
        let new = if new.is_empty() {
            inherited.as_str()
        } else {
            new
        };
        old == new
    }
    pub fn apply(&self, session: &mut Session, chart: usize) -> Result<bool, Error> {
        self.check(session, chart)?;
        if !self.has_changes() {
            return Ok(false);
        }
        let mut data = ChartData::existing_or_initial(session.document(), chart)?;
        data.expressed_chart_config = self.expression.clone();
        data.music_offset = self.music_offset;
        data.should_use_chart_music_offset = self.use_chart_offset;
        data.validate()?;
        if let Some(selection) = &self.selection {
            selection.check(&self.expression)?;
        }
        // SM's chart offset lives in editor metadata. SSC exports the effective
        // override as well. Never change another chart or the song OFFSET.
        let exported = (self.format == Format::Ssc && self.offset_changed())
            .then(|| self.effective_music_offset());
        let metadata_changed = self.offset_changed() || self.expression != self.original_expression;
        let clock = self.preserves_presentation_clock();
        let audio = self.preserves_presentation_audio(session.document());
        let changed = session.edit_chart_atomic(chart, "Change chart properties", |staged| {
            if metadata_changed {
                data.save_with_offset(staged, chart, exported, "Change chart properties")?;
            }
            self.identity.apply(staged, chart)?;
            self.text.apply(staged, chart)?;
            if self.display_tempo_changed() {
                if self.format == Format::Sm {
                    // Match the source EditorChart's independent DisplayTempo;
                    // native lossless SM keeps the original song tag untouched.
                    crate::timing_edit::resident::apply(
                        staged,
                        chart,
                        crate::timing_edit::Scope::Chart,
                        &[("DISPLAYBPM", self.display_tempo.serialize())],
                        "Change chart properties",
                    )?;
                } else {
                    staged.set_property_bytes(
                        Some(chart),
                        "DISPLAYBPM",
                        std::sync::Arc::from(self.display_tempo.serialize().into_bytes()),
                    )?;
                }
            }
            Ok(())
        })?;
        if changed {
            session.mark_chart_properties_history(chart, clock, audio);
        }
        Ok(changed)
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
