//! Source ToggleRowSelection completes a note selection, independently of
//! the current fractional chart focus and without editing document history.
use super::*;
impl Workspace {
    pub fn toggle_source_row_selection_at(
        &mut self,
        position: f64,
    ) -> Result<Option<crate::event_clipboard::Selection>, Error> {
        if !self.has_chart() {
            return Err(Error::InvalidEdit("No active chart".into()));
        }
        let row = nearest_source_row(position)?;
        if let Some(anchor) = self.anchor {
            let anchor = self.source_row_anchor.unwrap_or(
                i32::try_from(anchor)
                    .map_err(|_| Error::InvalidEdit("Invalid row selection anchor".into()))?,
            );
            let start = anchor.min(row);
            let end = anchor.max(row);
            let mut selection =
                self.event_selection_between(start.max(0) as u64, end.max(0) as u64)?;
            let chosen = selection
                .events()
                .iter()
                .map(|event| end >= 0 && matches!(event, crate::event_clipboard::Event::Note(_)))
                .collect::<Vec<_>>();
            selection.select_all(false);
            for (index, selected) in chosen.into_iter().enumerate() {
                selection.set_selected(index, selected)?;
            }
            self.anchor = None;
            self.source_row_anchor = None;
            self.selection_end = None;
            Ok(Some(selection))
        } else {
            // The legacy range overlay uses unsigned rows; retain the signed
            // source anchor separately so an entirely negative range is empty.
            self.anchor = Some(row.max(0) as u64);
            self.source_row_anchor = Some(row);
            self.selection_end = None;
            Ok(None)
        }
    }
}

fn nearest_source_row(position: f64) -> Result<i32, Error> {
    if !position.is_finite() || position < i32::MIN as f64 || position > i32::MAX as f64 {
        return Err(Error::InvalidEdit("Invalid row selection position".into()));
    }
    let truncated = position as i32;
    truncated
        .checked_add(i32::from(position - f64::from(truncated) > 0.5))
        .ok_or_else(|| Error::InvalidEdit("Row selection position overflow".into()))
}
impl Workspace {
    /// Signed transient widget bounds, using source nearest-row rounding for
    /// the live fractional focus. Completed note selection has no widget.
    pub fn source_row_selection_at(&self, position: f64) -> Result<Option<(i32, i32)>, Error> {
        if let Some(anchor) = self.source_row_anchor {
            let row = nearest_source_row(position)?;
            return Ok(Some((anchor.min(row), anchor.max(row))));
        }
        self.selection()
            .map(|(start, end)| {
                Ok((
                    i32::try_from(start)
                        .map_err(|_| Error::InvalidEdit("Row selection start overflow".into()))?,
                    i32::try_from(end)
                        .map_err(|_| Error::InvalidEdit("Row selection end overflow".into()))?,
                ))
            })
            .transpose()
    }
    pub fn source_chart_end_position_at(&mut self, position: f64) -> Result<f64, Error> {
        let selection = self.source_row_selection_at(position)?;
        crate::navigation::chart_end_position_with_selection(
            &mut self.file.session,
            self.chart,
            selection,
        )
    }
}
