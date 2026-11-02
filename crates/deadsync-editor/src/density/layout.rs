//! UpdateDensityGraphBounds source geometry, using native chart/misc anchors.
use crate::{
    Error,
    density_preferences::{Position, Preferences},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    Vertical,
    Horizontal,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Focus {
    pub lanes_start: i32,
    pub lanes_end: i32,
    pub misc_end: i32,
    pub misc_scaled_end: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub chart_area: Rect,
    pub header: i32,
    pub focus: Option<Focus>,
    pub dpi_scale: f64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub rect: Rect,
    pub orientation: Orientation,
}
pub fn bounds(p: Preferences, input: Input) -> Result<Bounds, Error> {
    p.validate()?;
    let scale = input.dpi_scale;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(invalid("Invalid density DPI scale"));
    }
    use std::num::Wrapping as W;
    let ui = |v: i32| W((v as f64 * scale) as i32);
    let area = input.chart_area;
    let thick = ui(p.height);
    let offset = ui(p.position_offsets[p.position.index()]);
    let width_offset = ui(p.width_offsets[p.position.index()]);
    let (mut x, mut w) = (W(0), W(0));
    let (y, h);
    let mut orientation = Orientation::Vertical;
    match p.position {
        Position::RightSideOfWindow => {
            w = thick;
            x = W(area.x) + W(area.width) - w - offset;
            h = W(area.height) - W(input.header) + width_offset * W(2);
            y = W(area.y) + W(input.header) - width_offset;
        }
        Position::LeftSideOfWindow => {
            w = thick;
            x = W(area.x) + offset;
            h = W(area.height) - W(input.header) + width_offset * W(2);
            y = W(area.y) + W(input.header) - width_offset;
        }
        Position::FocusedChartWithoutScaling | Position::FocusedChartWithScaling => {
            if let Some(focus) = input.focus {
                x = W(if p.position == Position::FocusedChartWithoutScaling {
                    focus.misc_end
                } else {
                    focus.misc_scaled_end
                }) + offset;
            }
            w = thick;
            h = W(area.height) - W(input.header) + width_offset * W(2);
            y = W(area.y) + W(input.header) - width_offset;
        }
        Position::TopOfFocusedChart | Position::BottomOfFocusedChart => {
            if let Some(focus) = input.focus {
                x = W(focus.lanes_start) - width_offset;
                w = W(focus.lanes_end) - W(focus.lanes_start) + width_offset * W(2);
            }
            h = thick;
            y = if p.position == Position::TopOfFocusedChart {
                W(area.y) + W(input.header) + offset
            } else {
                W(area.y) + W(area.height) - thick - offset
            };
            orientation = Orientation::Horizontal;
        }
    }
    Ok(Bounds {
        rect: Rect {
            x: x.0,
            y: y.0,
            width: w.0,
            height: h.0,
        },
        orientation,
    })
}
fn invalid(s: &str) -> Error {
    Error::InvalidEdit(s.into())
}
