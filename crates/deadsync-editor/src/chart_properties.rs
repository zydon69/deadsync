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
        Ok(Self {
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
        })
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
        self.offset_changed() || self.expression != self.original_expression
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
        data.save_with_offset(session, chart, exported, "Change chart properties")
    }
}
fn invalid(message: &str) -> Error {
    Error::InvalidEdit(message.into())
}
