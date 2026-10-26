//! Natural-width chart geometry around the focused view (GrooveAuthor
//! Editor.cs UpdateChartPositions). Off-screen views are culled before drawing.
use deadsync_editor::Error;
#[derive(Clone, Copy, Debug)]
pub(super) struct Geometry {
    pub source: usize,
    pub order: usize,
    pub focused: bool,
    pub left: f32,
    pub field_width: f32,
    pub lane_width: f32,
    pub clip_left: f32,
    pub clip_right: f32,
}
impl Geometry {
    pub fn right(self) -> f32 {
        self.left + self.field_width
    }
    pub fn visible_x(self, x: f32) -> bool {
        x >= self.clip_left && x <= self.clip_right
    }
}
/// Columns are live grid widths, including a focused generation preview.
/// At most viewport-visible natural-width fields are returned; tabs are not
/// compressed to fit all open charts in the viewport.
pub(super) fn layout(
    charts: &[(usize, u16)],
    focused: usize,
    center: f32,
    available: f32,
    clip_left: f32,
    clip_right: f32,
) -> Result<Vec<Geometry>, Error> {
    if ![center, available, clip_left, clip_right]
        .iter()
        .all(|value| value.is_finite())
        || clip_left >= clip_right
        || charts.iter().any(|(_, columns)| *columns == 0)
    {
        return Err(Error::InvalidEdit("Invalid chart-view geometry".into()));
    }
    let focus = charts
        .iter()
        .position(|(chart, _)| *chart == focused)
        .ok_or_else(|| Error::InvalidEdit("Focused chart is absent from open views".into()))?;
    let geometry = |index: usize, left: f32| {
        let (source, columns) = charts[index];
        let lane_width = (available.max(80.0) / f32::from(columns)).min(48.0);
        let field_width = lane_width * f32::from(columns);
        Geometry {
            source,
            order: index,
            focused: index == focus,
            left,
            field_width,
            lane_width,
            clip_left,
            clip_right,
        }
    };
    let width = geometry(focus, 0.0).field_width;
    let focused_geometry = geometry(focus, center - width * 0.5);
    let mut result = Vec::new();
    let keep = |item: Geometry| item.right() + 30.0 >= clip_left && item.left - 42.0 <= clip_right;
    if keep(focused_geometry) {
        result.push(focused_geometry);
    }
    let mut edge = focused_geometry.left - 42.0;
    for index in (0..focus).rev() {
        if edge < clip_left {
            break;
        }
        let width = geometry(index, 0.0).field_width;
        let item = geometry(index, edge - 30.0 - width);
        edge = item.left - 42.0;
        if keep(item) {
            result.push(item);
        }
    }
    let mut edge = focused_geometry.right() + 30.0;
    for index in focus + 1..charts.len() {
        if edge > clip_right {
            break;
        }
        let item = geometry(index, edge + 42.0);
        edge = item.right() + 30.0;
        if keep(item) {
            result.push(item);
        }
    }
    result.sort_by_key(|item| item.order);
    Ok(result)
}
