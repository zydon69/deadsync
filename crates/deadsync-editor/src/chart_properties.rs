//! Chart-wide editor properties, independent of the existence of pattern markers.
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
        let display_tempo = crate::display_tempo::Value::parse(
            &session
                .document()
                .text(
                    (session.document().format == Format::Ssc).then_some(chart),
                    "DISPLAYBPM",
                )
                .or_else(|| session.document().text(None, "DISPLAYBPM"))
                .unwrap_or_default(),
        );
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
        session.edit_chart_atomic(chart, "Change chart properties", |staged| {
            if metadata_changed {
                data.save_with_offset(staged, chart, exported, "Change chart properties")?;
            }
            self.identity.apply(staged, chart)?;
            self.text.apply(staged, chart)?;
            if self.display_tempo_changed() {
                staged.set_property_bytes(
                    (self.format == Format::Ssc).then_some(chart),
                    "DISPLAYBPM",
                    std::sync::Arc::from(self.display_tempo.serialize().into_bytes()),
                )?;
            }
            Ok(())
        })
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
