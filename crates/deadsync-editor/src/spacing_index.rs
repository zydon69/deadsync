//! GrooveAuthor RateAlteringEventTree.FindBest policy (48e18f71, MIT).
//! Active spacing lookup preserves the original strict-boundary semantics.
use crate::{
    Error,
    generation::editor_timing::SpacingSection,
    spacing::Anchor,
    spacing_search::{self, Settings},
    zoom::SpacingMode,
};
pub struct Index {
    sections: Vec<SpacingSection>,
}
impl Index {
    pub fn new(sections: Vec<SpacingSection>) -> Result<Self, Error> {
        if sections.windows(2).any(|pair| {
            (pair[0].mapping.row(), pair[0].order) > (pair[1].mapping.row(), pair[1].order)
        }) {
            return Err(Error::InvalidEdit(
                "Spacing sections are not in chart order".into(),
            ));
        }
        // The original editor clock clamps each event time to its predecessor,
        // including through negative stops and warps.
        if sections
            .windows(2)
            .any(|pair| pair[0].mapping.time() > pair[1].mapping.time())
        {
            return Err(Error::InvalidEdit(
                "Spacing section times are not in editor clock order".into(),
            ));
        }
        if sections.iter().any(|section| {
            !section.tempo.is_finite() || section.tempo <= 0.0 || !section.scroll.is_finite()
        }) {
            return Err(Error::InvalidEdit(
                "Invalid initialized spacing section".into(),
            ));
        }
        Ok(Self { sections })
    }
    pub fn sections(&self) -> &[SpacingSection] {
        &self.sections
    }
    /// Greatest section strictly preceding the query; if none precedes it,
    /// choose the first section. Exact ties remain on the preceding section.
    pub fn active(&self, mode: SpacingMode, time: f64, row: f64) -> Result<Option<usize>, Error> {
        if !time.is_finite() || !row.is_finite() {
            return Err(Error::InvalidEdit(
                "Nonfinite active spacing position".into(),
            ));
        }
        if self.sections.is_empty() {
            return Ok(None);
        }
        let index = if mode != SpacingMode::ConstantTime {
            self.sections
                .partition_point(|section| section.mapping.row() < row)
                .checked_sub(1)
        } else {
            self.sections
                .partition_point(|section| section.mapping.time() < time)
                .checked_sub(1)
        };
        Ok(Some(index.unwrap_or(0)))
    }
    /// Active coefficients immediately before a concrete event's phase.
    pub fn event_active(&self, row: f64, order: u8) -> Result<Option<usize>, Error> {
        if !row.is_finite() {
            return Err(Error::InvalidEdit("Nonfinite event position".into()));
        }
        if self.sections.is_empty() {
            return Ok(None);
        }
        let index = self
            .sections
            .partition_point(|section| (section.mapping.row(), section.order) < (row, order));
        Ok(Some(index.saturating_sub(1)))
    }
    pub fn find_location(
        &self,
        focal: Anchor,
        target_y: f64,
        settings: Settings,
    ) -> Result<(f64, f64), Error> {
        let active = self
            .active(settings.mode, focal.time, focal.row)?
            .unwrap_or(0);
        spacing_search::find_location(&self.sections, active, focal, target_y, settings)
    }
}
