//! Original SelectMiscEvents two directional scans, including its asymmetric
//! time-only end tests inside rate chunks and its layout-dependent early-outs.
use crate::{
    Error,
    spacing::{Anchor, Helper},
    spacing_index::Index,
    spacing_search::Settings,
    zoom::SpacingMode,
};
#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub row: f64,
    pub time: f64,
    pub phase: u8,
}
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub start_time: f64,
    pub end_time: f64,
    pub start_row: f64,
    pub end_row: f64,
}
pub struct ResultSet {
    pub order: Vec<usize>,
    pub start_y: Option<f64>,
    pub end_y: Option<f64>,
}
fn invalid() -> Error {
    Error::InvalidEdit("Invalid miscellaneous scan inventory".into())
}
fn rate_anchor(index: &Index, i: usize, y: f64) -> Anchor {
    let m = index.sections()[i].mapping;
    Anchor {
        time: m.time(),
        row: m.row(),
        y,
    }
}
fn helper(index: &Index, i: usize, s: Settings) -> Result<Helper, Error> {
    let mut h = Helper::new(s.mode, s.reverse);
    h.update(
        index.sections()[i].spacing_rate(),
        s.speeds,
        s.interpolated_scroll,
        s.zoom,
    )?;
    Ok(h)
}
/// `rate_items` maps each concrete rate identity into the ordered misc array;
/// virtual/non-Misc sections are None. `position` must perform the actual
/// shared Layout.position and return its current GetMaxYForSingleRow value.
/// No rendering quota is applied to selection discovery.
pub fn scan(
    index: &Index,
    focal: Anchor,
    settings: Settings,
    b: Bounds,
    items: &[Item],
    rate_items: &[Option<usize>],
    mut position: impl FnMut(usize, f64) -> Result<f64, Error>,
) -> Result<ResultSet, Error> {
    if rate_items.len() != index.sections().len()
        || items
            .iter()
            .any(|e| ![e.row, e.time].iter().all(|x| x.is_finite()))
        || items
            .windows(2)
            .any(|e| (e[0].row, e[0].phase) > (e[1].row, e[1].phase))
        || rate_items.iter().flatten().any(|&i| i >= items.len())
    {
        return Err(invalid());
    }
    let mut result = ResultSet {
        order: vec![],
        start_y: None,
        end_y: None,
    };
    let Some(active) = index.active(settings.mode, focal.time, focal.row)? else {
        return Ok(result);
    };
    let mut seen = std::collections::BTreeSet::new();
    let time = settings.mode == SpacingMode::ConstantTime;
    let direction = if settings.reverse { -1.0 } else { 1.0 };
    let active_rate = index.sections()[active];
    let active_key = (active_rate.mapping.row(), active_rate.order);
    let mut emit = |id, y, result: &mut ResultSet| -> Result<f64, Error> {
        let max = position(id, y)?;
        if seen.insert(id) {
            result.order.push(id);
        }
        Ok(max)
    };
    // Up uses the concrete current identity or greatest preceding Misc event.
    let mut i = active;
    let mut h = helper(index, i, settings)?;
    let mut anchor = rate_anchor(
        index,
        i,
        h.y(active_rate.mapping.time(), active_rate.mapping.row(), focal)?,
    );
    let first_y = anchor.y;
    let start_in_range = if time {
        b.start_time <= anchor.time
    } else {
        b.start_row <= anchor.row
    };
    let mut misc = rate_items[active].or_else(|| {
        items
            .partition_point(|e| (e.row, e.phase) < active_key)
            .checked_sub(1)
    });
    let mut beyond = false;
    loop {
        if beyond {
            break;
        }
        if i > 0 {
            i -= 1;
            h = helper(index, i, settings)?;
            let m = index.sections()[i].mapping;
            let previous = rate_anchor(index, i, h.y(m.time(), m.row(), anchor)?);
            if result.start_y.is_none()
                && if time {
                    b.start_time >= previous.time && b.start_time <= anchor.time
                } else {
                    b.start_row >= previous.row && b.start_row <= anchor.row
                }
            {
                result.start_y = Some(h.y(b.start_time, b.start_row, previous)?);
            }
            // Deliberately time-only in original, including row/variable mode.
            if result.end_y.is_none() && b.end_time >= previous.time && b.end_time <= anchor.time {
                result.end_y = Some(h.y(b.end_time, b.end_row, previous)?);
            }
            while !beyond {
                let Some(id) = misc else {
                    break;
                };
                let e = items[id];
                if !(if time {
                    e.time >= previous.time && e.time <= anchor.time
                } else {
                    e.row >= previous.row && e.row <= anchor.row
                }) {
                    break;
                }
                let y = h.y(e.time, e.row, previous)?;
                let max = emit(id, y, &mut result)?;
                misc = id.checked_sub(1);
                beyond = match result.start_y {
                    Some(start) => (start - y) * direction > max,
                    None => !start_in_range && (first_y - y) * direction > max,
                };
            }
            anchor = previous;
        } else {
            if result.start_y.is_none()
                && if time {
                    b.start_time <= anchor.time
                } else {
                    b.start_row <= anchor.row
                }
            {
                result.start_y = Some(h.y(b.start_time, b.start_row, anchor)?);
            }
            if result.end_y.is_none()
                && if time {
                    b.end_time <= anchor.time
                } else {
                    b.end_row <= anchor.row
                }
            {
                result.end_y = Some(h.y(b.end_time, b.end_row, anchor)?);
            }
            while !beyond {
                let Some(id) = misc else {
                    break;
                };
                let e = items[id];
                if if time {
                    e.time <= anchor.time
                } else {
                    e.row <= anchor.row
                } {
                    let y = h.y(e.time, e.row, anchor)?;
                    let max = emit(id, y, &mut result)?;
                    beyond = match result.start_y {
                        Some(start) => (start - y) * direction > max,
                        None => !start_in_range && (first_y - y) * direction > max,
                    };
                }
                misc = id.checked_sub(1);
            }
            break;
        }
    }
    // Reset coefficients and anchor: the down scan clones the original active
    // rate enumerator, not the exhausted up-scan state.
    i = active;
    h = helper(index, i, settings)?;
    anchor = rate_anchor(
        index,
        i,
        h.y(active_rate.mapping.time(), active_rate.mapping.row(), focal)?,
    );
    let first_y = anchor.y;
    let end_in_range = if time {
        b.end_time >= anchor.time
    } else {
        b.end_row >= anchor.row
    };
    let first = rate_items[active]
        .unwrap_or_else(|| items.partition_point(|e| (e.row, e.phase) <= active_key));
    misc = (first < items.len()).then_some(first);
    beyond = false;
    loop {
        if beyond {
            break;
        }
        if i + 1 < index.sections().len() {
            let next = i + 1;
            let m = index.sections()[next].mapping;
            let following = rate_anchor(index, next, h.y(m.time(), m.row(), anchor)?);
            if result.start_y.is_none()
                && if time {
                    b.start_time >= anchor.time && b.start_time <= following.time
                } else {
                    b.start_row >= anchor.row && b.start_row <= following.row
                }
            {
                result.start_y = Some(h.y(b.start_time, b.start_row, anchor)?);
            }
            if result.end_y.is_none() && b.end_time >= anchor.time && b.end_time <= following.time {
                result.end_y = Some(h.y(b.end_time, b.end_row, anchor)?);
            }
            while !beyond {
                let Some(id) = misc else {
                    break;
                };
                let e = items[id];
                if !(if time {
                    e.time >= anchor.time && e.time <= following.time
                } else {
                    e.row >= anchor.row && e.row <= following.row
                }) {
                    break;
                }
                let y = h.y(e.time, e.row, anchor)?;
                let max = emit(id, y, &mut result)?;
                misc = (id + 1 < items.len()).then_some(id + 1);
                beyond = match result.end_y {
                    Some(end) => (y - end) * direction > max,
                    None => !end_in_range && (y - first_y) * direction > max,
                };
            }
            i = next;
            h = helper(index, i, settings)?;
            anchor = following;
        } else {
            if result.start_y.is_none()
                && if time {
                    b.start_time >= anchor.time
                } else {
                    b.start_row >= anchor.row
                }
            {
                result.start_y = Some(h.y(b.start_time, b.start_row, anchor)?);
            }
            if result.end_y.is_none()
                && if time {
                    b.end_time >= anchor.time
                } else {
                    b.end_row >= anchor.row
                }
            {
                result.end_y = Some(h.y(b.end_time, b.end_row, anchor)?);
            }
            while !beyond {
                let Some(id) = misc else {
                    break;
                };
                let e = items[id];
                if if time {
                    e.time >= anchor.time
                } else {
                    e.row >= anchor.row
                } {
                    let y = h.y(e.time, e.row, anchor)?;
                    let max = emit(id, y, &mut result)?;
                    beyond = match result.end_y {
                        Some(end) => (y - end) * direction > max,
                        None => !end_in_range && (y - first_y) * direction > max,
                    };
                }
                misc = (id + 1 < items.len()).then_some(id + 1);
            }
            break;
        }
    }
    Ok(result)
}
