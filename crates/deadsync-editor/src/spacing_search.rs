//! ActiveEditorChart.FindChartTimeAndRowForScreenSpaceY traversal (MIT).
//! The caller supplies the active rate index selected by the chart's event tree.
use crate::{
    Error,
    generation::editor_timing::SpacingSection,
    spacing::{Anchor, Helper, Speeds},
    zoom::SpacingMode,
};
#[derive(Clone, Copy)]
pub struct Settings {
    pub mode: SpacingMode,
    pub reverse: bool,
    pub speeds: Speeds,
    pub interpolated_scroll: f64,
    pub zoom: f64,
}
/// Sections must be in the original chart event order.
/// Follows the original directional search. For folded variable spacing this
/// chooses the first crossing along that search, not every row sharing a pixel.
/// Empty charts return the original (0,0) fallback. Collapsed/nonfinite inverse
/// rates return an explicit error instead of a nonfinite screen location.
pub fn find_location(
    sections: &[SpacingSection],
    active: usize,
    focal: Anchor,
    target_y: f64,
    settings: Settings,
) -> Result<(f64, f64), Error> {
    if ![focal.y, focal.time, focal.row, target_y]
        .iter()
        .all(|v| v.is_finite())
    {
        return Err(Error::InvalidEdit(
            "Nonfinite spacing search position".into(),
        ));
    }
    if sections.is_empty() {
        return Ok((0.0, 0.0));
    }
    let Some(section) = sections.get(active) else {
        return Err(Error::InvalidEdit("Invalid active spacing section".into()));
    };
    let mut index = active;
    let mut helper = Helper::new(settings.mode, settings.reverse);
    helper.update(
        section.spacing_rate(),
        settings.speeds,
        settings.interpolated_scroll,
        settings.zoom,
    )?;
    let mut anchor = Anchor {
        y: helper.y(section.mapping.time(), section.mapping.row(), focal)?,
        time: section.mapping.time(),
        row: section.mapping.row(),
    };
    let direction = if settings.reverse { -1.0 } else { 1.0 };
    let relative = (target_y - focal.y) * direction;
    if relative == 0.0 {
        return Ok((focal.time, focal.row));
    }
    loop {
        if relative < 0.0 {
            if (anchor.y - target_y) * direction <= 0.0 || index == 0 {
                let mapping = sections[index].mapping;
                return helper.location(
                    target_y,
                    anchor,
                    false,
                    |time| mapping.position_at(time),
                    |row| mapping.time_at(row),
                );
            }
            index -= 1;
            let next = sections[index];
            helper.update(
                next.spacing_rate(),
                settings.speeds,
                settings.interpolated_scroll,
                settings.zoom,
            )?;
            anchor = Anchor {
                y: helper.y(next.mapping.time(), next.mapping.row(), anchor)?,
                time: next.mapping.time(),
                row: next.mapping.row(),
            };
        } else {
            if index + 1 == sections.len() {
                let mapping = sections[index].mapping;
                return helper.location(
                    target_y,
                    anchor,
                    false,
                    |time| mapping.position_at(time),
                    |row| mapping.time_at(row),
                );
            }
            index += 1;
            let next = sections[index];
            let y = helper.y(next.mapping.time(), next.mapping.row(), anchor)?;
            helper.update(
                next.spacing_rate(),
                settings.speeds,
                settings.interpolated_scroll,
                settings.zoom,
            )?;
            anchor = Anchor {
                y,
                time: next.mapping.time(),
                row: next.mapping.row(),
            };
            if (anchor.y - target_y) * direction >= 0.0 {
                let mapping = next.mapping;
                return helper.location(
                    target_y,
                    anchor,
                    true,
                    |time| mapping.position_at(time),
                    |row| mapping.time_at(row),
                );
            }
        }
    }
}
