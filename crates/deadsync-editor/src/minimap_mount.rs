//! Editor.UpdateMiniMapBounds arithmetic. Inputs explicitly separate native UI
//! geometry from original ImGui dimensions; no guessed chart/misc-event edges.
use crate::{
    Error,
    minimap_position_preferences::{Mount, Preferences},
};
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct FocusEdges {
    pub without_scale: i32,
    pub with_scale: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Padding {
    pub top_screen: i32,
    pub top_chart: i32,
    pub bottom: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub chart_area: Rect,
    pub viewport_height: i32,
    pub focus: Option<FocusEdges>,
    pub padding: Padding,
    pub dpi_scale: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub texture_height: i32,
    pub visible_height: u32,
}
pub fn ui_scaled(value: i32, scale: f32) -> Result<i32, Error> {
    let result = value as f32 * scale;
    if !scale.is_finite()
        || scale <= 0.0
        || !result.is_finite()
        || result < (i32::MIN as f64) as f32
        || result as f64 >= i32::MAX as f64
    {
        return Err(Error::InvalidEdit(
            "Invalid minimap UI scale or overflow".into(),
        ));
    }
    Ok(result as i32)
}
impl Padding {
    /// ImGui source constants: menu20, chart header21, scene padding10.
    /// Native integration must instead supply its own actual UI insets.
    pub fn original(scale: f32) -> Result<Self, Error> {
        Ok(Self {
            top_screen: ui_scaled(51, scale)?,
            top_chart: ui_scaled(31, scale)?,
            bottom: ui_scaled(10, scale)?,
        })
    }
}
pub fn bounds(preferences: Preferences, width: u32, input: Input) -> Result<Bounds, Error> {
    if input.chart_area.width < 0
        || input.chart_area.height < 0
        || input.viewport_height < 0
        || input.padding.top_screen < 0
        || input.padding.top_chart < 0
        || input.padding.bottom < 0
        || width > 1024
    {
        return Err(Error::InvalidEdit("Invalid minimap mount geometry".into()));
    }
    let width = ui_scaled(width as i32, input.dpi_scale)?;
    let offset = ui_scaled(preferences.offset(preferences.mount), input.dpi_scale)?;
    let checked = |value: i64| {
        i32::try_from(value).map_err(|_| Error::InvalidEdit("Minimap bounds overflow".into()))
    };
    let x = checked(match preferences.mount {
        Mount::RightSideOfWindow => {
            input.chart_area.x as i64 + input.chart_area.width as i64 - offset as i64 - width as i64
        }
        Mount::LeftSideOfWindow => input.chart_area.x as i64 + offset as i64,
        Mount::FocusedChartWithoutScaling => input
            .focus
            .map_or(0, |edges| edges.without_scale as i64 + offset as i64),
        Mount::FocusedChartWithScaling => input
            .focus
            .map_or(0, |edges| edges.with_scale as i64 + offset as i64),
    })?;
    let y = checked(input.chart_area.y as i64 + input.padding.top_chart as i64)?;
    let texture_height = checked(
        (input.viewport_height as i64
            - input.padding.top_screen as i64
            - input.padding.bottom as i64)
            .max(0),
    )?;
    let visible = (input.chart_area.height as i64
        - input.padding.top_chart as i64
        - input.padding.bottom as i64)
        .max(0)
        .min(texture_height as i64);
    Ok(Bounds {
        x,
        y,
        width,
        texture_height,
        visible_height: visible as u32,
    })
}
