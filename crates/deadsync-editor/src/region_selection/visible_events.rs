//! Bounded, ordered UpdateChartEvents viewport admission. Selection uses its
//! separate two-way Misc scan and never inherits this drawing quota.
use super::{
    admission::{Admission, Kind},
    inventory::Entry,
};
use crate::{
    Error,
    event_clipboard::{Event, TimingKind},
    notes::{Grid, Position},
    playback::Clock,
    spacing::{Anchor, Helper},
    spacing_index::Index,
    spacing_search::Settings,
};
use std::cmp::Ordering;
#[derive(Clone, Copy)]
struct Key {
    row: f64,
    rank: u8,
    phase: u8,
    lane: u16,
    player: u32,
    edited: bool,
}
fn key(e: Event, phase: u8) -> Key {
    let (rank, lane, player) = match e {
        Event::SongWidget(w) => (
            if w.kind() == crate::event_clipboard::WidgetKind::Preview {
                0
            } else {
                1
            },
            0,
            0,
        ),
        Event::Note(p) => (2, p.lane, p.player),
        _ => (2, 0, 0),
    };
    Key {
        row: e.chart_position(),
        rank,
        phase,
        lane,
        player,
        edited:false,
    }
}
fn cmp(a: Key, b: Key) -> Ordering {
    a.row
        .total_cmp(&b.row)
        .then(a.rank.cmp(&b.rank))
        .then(a.phase.cmp(&b.phase))
        .then(a.lane.cmp(&b.lane))
        .then(a.player.cmp(&b.player))
        .then(a.edited.cmp(&b.edited))
}
fn head_key(h:&crate::lane_entry::Head)->Key{let mut k=key(Event::Note(h.position),h.note.kind.editor_phase());k.edited=h.edited;k}
fn coefficients(index: &Index, i: usize, s: Settings) -> Result<Helper, Error> {
    let mut h = Helper::new(s.mode, s.reverse);
    h.update(
        index.sections()[i].spacing_rate(),
        s.speeds,
        s.interpolated_scroll,
        s.zoom,
    )?;
    Ok(h)
}
fn concrete_rate(e: Event, order: u8, row: f64) -> bool {
    let k = match order {
        0 => TimingKind::Signature,
        1 => TimingKind::Tempo,
        7 => TimingKind::Delay,
        13 => TimingKind::Scroll,
        15 | 16 => TimingKind::Stop,
        17 => TimingKind::Warp,
        _ => return false,
    };
    matches!(e,Event::Timing{row:r,kind}if r as f64==row&&kind==k)
}
/// Source active selection inputs: the start is a chart position while the
/// current endpoint is a screen-space pointer coordinate. A keyboard adapter
/// may supply that Y from its authenticated, explicitly entered endpoint.
#[derive(Clone, Copy)]
pub struct Region {
    pub start_time: f64,
    pub start_row: f64,
    pub current_y: f64,
}
pub struct RegionCompletion {
    input: Region,
    start_y: Option<f64>,
    current: Option<Result<(f64, f64), Error>>,
    processed_rates: usize,
    extra_steps: usize,
    fallback: bool,
}
impl RegionCompletion {
    fn new(input: Region) -> Result<Self, Error> {
        if ![input.start_time, input.start_row, input.current_y]
            .iter()
            .all(|v| v.is_finite())
        {
            return Err(Error::InvalidEdit(
                "Nonfinite selected region geometry".into(),
            ));
        }
        Ok(Self {
            input,
            start_y: None,
            current: None,
            processed_rates: 1,
            extra_steps: 0,
            fallback: false,
        })
    }
    fn complete(&self) -> bool {
        self.start_y.is_some() && self.current.is_some()
    }
    fn update(
        &mut self,
        h: Helper,
        index: &Index,
        section: usize,
        anchor: Anchor,
        next: Option<usize>,
        settings: Settings,
    ) -> Result<(), Error> {
        let rate = index.sections()[section];
        let next = next.map(|i| index.sections()[i]);
        if self.start_y.is_none() {
            let before = if settings.mode == crate::zoom::SpacingMode::ConstantTime {
                self.input.start_time < rate.mapping.time()
                    || next.is_none_or(|n| self.input.start_time < n.mapping.time())
            } else {
                self.input.start_row < rate.mapping.row()
                    || next.is_none_or(|n| self.input.start_row < n.mapping.row())
            };
            if before {
                self.start_y = Some(h.y(self.input.start_time, self.input.start_row, anchor)?);
            }
        }
        if self.current.is_none() {
            let sign = if settings.reverse { -1.0 } else { 1.0 };
            let before = (self.input.current_y - anchor.y) * sign < 0.0
                || match next {
                    None => true,
                    Some(n) => {
                        (self.input.current_y - h.y(n.mapping.time(), n.mapping.row(), anchor)?)
                            * sign
                            < 0.0
                    }
                };
            if before {
                // Unlike the original nonfinite double result, an undefined
                // inverse is retained as a presentation diagnostic. Admission
                // and the keyboard adapter's authenticated points stay valid.
                self.current = Some(h.location(
                    self.input.current_y,
                    anchor,
                    false,
                    |t| rate.mapping.position_at(t),
                    |r| rate.mapping.time_at(r),
                ));
            }
        }
        Ok(())
    }
    pub fn start_y(&self) -> Option<f64> {
        self.start_y
    }
    pub fn current_y(&self) -> f64 {
        self.input.current_y
    }
    pub fn current(&self) -> Option<&Result<(f64, f64), Error>> {
        self.current.as_ref()
    }
    pub fn processed_rates(&self) -> usize {
        self.processed_rates
    }
    pub fn extra_steps(&self) -> usize {
        self.extra_steps
    }
    pub fn used_fallback(&self) -> bool {
        self.fallback
    }
}
#[derive(Default)]
pub struct Plan {
    events: Vec<Event>,
    heads:Vec<crate::lane_entry::Head>,
    misc: Vec<(usize, f64)>,
    pub stopped_by_quota: bool,
    completion: Option<RegionCompletion>,
}
impl Plan {
    pub fn heads(&self)->&[crate::lane_entry::Head]{&self.heads}

    pub fn completion(&self) -> Option<&RegionCompletion> {
        self.completion.as_ref()
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }
    pub fn misc_calls(&self) -> &[(usize, f64)] {
        &self.misc
    }
    pub fn notes(&self) -> impl Iterator<Item = Position> + '_ {
        self.events.iter().filter_map(|e| match e {
            Event::Note(p) => Some(*p),
            _ => None,
        })
    }
}
#[derive(Clone, Copy)]
pub struct Viewport {
    pub top: f64,
    pub bottom: f64,
    pub half_arrow: f64,
    pub hold_cap: f64,
}
/// Misc entries must be in the source event order and carry actual clock times.
/// Notes are merged lazily with O(tracks) auxiliary memory. Source counts lane
/// events even when RenderNotes will hide them at the final actor stage.
pub fn prepare(
    grid: &Grid,
    clock: &Clock,
    index: &Index,
    misc: &[Entry],
    focal: Anchor,
    settings: Settings,
    viewport: Viewport,
    max: i32,
    render_misc: bool,
) -> Result<Plan, Error> {
    prepare_with_region(
        grid,
        clock,
        index,
        misc,
        focal,
        settings,
        viewport,
        max,
        render_misc,
        None,
        0,
    )
}
pub fn prepare_with_region(
    grid: &Grid,
    clock: &Clock,
    index: &Index,
    misc: &[Entry],
    focal: Anchor,
    settings: Settings,
    viewport: Viewport,
    max: i32,
    render_misc: bool,
    region: Option<Region>,
    max_rates: i32,
) -> Result<Plan, Error> {
    prepare_with_input(grid,clock,index,misc,focal,settings,viewport,max,render_misc,region,max_rates,None)
}
pub fn prepare_with_input(
    grid: &Grid,
    clock: &Clock,
    index: &Index,
    misc: &[Entry],
    focal: Anchor,
    settings: Settings,
    viewport: Viewport,
    max: i32,
    render_misc: bool,
    region: Option<Region>,
    max_rates: i32,
    input:Option<&crate::lane_entry::Inputs>,
) -> Result<Plan, Error> {
    let held=crate::lane_entry::Held::new(grid,input);
    if ![
        viewport.top,
        viewport.bottom,
        viewport.half_arrow,
        viewport.hold_cap,
    ]
    .iter()
    .all(|v| v.is_finite())
        || viewport.top > viewport.bottom
        || viewport.half_arrow <= 0.0
        || viewport.hold_cap < 0.0
    {
        return Err(Error::InvalidEdit(
            "Invalid viewport event admission geometry".into(),
        ));
    }
    let mut plan = Plan::default();
    let Some(mut section) = index.active(settings.mode, focal.time, focal.row)? else {
        return Ok(plan);
    };
    let sign = if settings.reverse { -1.0 } else { 1.0 };
    let margin = viewport.hold_cap.max(viewport.half_arrow);
    let start_y = if sign > 0.0 {
        viewport.top - margin
    } else {
        viewport.bottom + margin
    };
    let mut h = coefficients(index, section, settings)?;
    let mut m = index.sections()[section].mapping;
    let mut anchor = Anchor {
        y: h.y(m.time(), m.row(), focal)?,
        time: m.time(),
        row: m.row(),
    };
    while (anchor.y - start_y) * sign >= 0.0 && section > 0 {
        section -= 1;
        h = coefficients(index, section, settings)?;
        m = index.sections()[section].mapping;
        anchor = Anchor {
            y: h.y(m.time(), m.row(), anchor)?,
            time: m.time(),
            row: m.row(),
        };
    }
    // Source IEEE infinity is a search sentinel, never a chart Position.
    // Fumen FindBest chooses the first event for -Infinity, last for +Infinity.
    let start_row =
        if settings.mode != crate::zoom::SpacingMode::ConstantTime && h.pixels_per_row() == 0.0 {
            anchor.row + (start_y - anchor.y) / (h.pixels_per_row() * sign)
        } else {
            h.location(
                start_y,
                anchor,
                false,
                |t| m.position_at(t),
                |r| m.time_at(r),
            )?
            .1
        };
    // FindBestByPosition begins at the greatest single event strictly before
    // the query, or the first following event when no predecessor exists.
    let threshold = start_row.ceil().clamp(0.0, u64::MAX as f64) as u64;
    let previous_note = held.last_head_before(threshold).map(|h|(Event::Note(h.position),head_key(&h)));
    let previous_misc = misc
        .partition_point(|e| e.event.chart_position() < start_row)
        .checked_sub(1)
        .map(|i| (misc[i].event, key(misc[i].event,misc[i].phase)));
    let previous = match (previous_note, previous_misc) {
        (Some(a), Some(b)) => Some(if cmp(a.1,b.1).is_gt() {
            a
        } else {
            b
        }),
        (a, b) => a.or(b),
    };
    let from = previous.map(|(_, k)|k);
    let first_row = from.map_or(0, |k| k.row.floor().max(0.0) as u64);
    // Source overlapping holds are pre-added in lane order, before the quota
    // loop. A head can also occur in the loop: its count is intentionally not
    // deduplicated here. Actor consumers receive that same admitted sequence.
    if start_row >= 0.0 {
        for head in held.crossing(start_row,threshold){plan.events.push(Event::Note(head.position));plan.heads.push(head);}
    }
    let mut admission = Admission::new(max, plan.events.len(), render_misc);
    let mut notes = held.heads_from(first_row).filter(|h|from.is_none_or(|f|!cmp(head_key(h),f).is_lt())).peekable();
    let mut mi = from.map_or(0, |f| {
        misc.partition_point(|e| cmp(key(e.event, e.phase), f).is_lt())
    });
    let mut next_rate = section + 1;
    let mut completion = region.map(RegionCompletion::new).transpose()?;
    if let Some(c) = &mut completion {
        c.update(
            h,
            index,
            section,
            anchor,
            (next_rate < index.sections().len()).then_some(next_rate),
            settings,
        )?;
    }
    let mut reached_end = false;
    loop {
        let note = notes.peek();
        let widget = misc.get(mi).copied();
        let take_note = match (note, widget) {
            (Some(h), Some(e)) => cmp(
                head_key(h),
                key(e.event, e.phase),
            )
            .is_lt(),
            (Some(_), None) => true,
            _ => false,
        };
        let mut picked_head=None;
        let (event, _phase, time, widget_index) = if take_note {
            let h=notes.next().unwrap();let p=h.position;let phase=h.note.kind.editor_phase();picked_head=Some(h);
            (
                Event::Note(p),
                phase,
                clock.event_phase_time(p.row, phase)?,
                None,
            )
        } else if let Some(e) = widget {
            let i = mi;
            mi += 1;
            (e.event, e.phase, e.chart_time, Some(i))
        } else {
            break;
        };
        let is_next = next_rate < index.sections().len()
            && concrete_rate(
                event,
                index.sections()[next_rate].order,
                index.sections()[next_rate].mapping.row(),
            );
        if is_next {
            let rate = index.sections()[next_rate];
            let y = h.y(time, event.chart_position(), anchor)?;
            let decision = admission.offer(Kind::NextRate);
            if decision.admit {
                plan.events.push(event);
                if let Some(i) = widget_index {
                    plan.misc.push((i, y));
                }
            }
            section = next_rate;
            next_rate += 1;
            h = coefficients(index, section, settings)?;
            anchor = Anchor {
                y,
                time: rate.mapping.time(),
                row: rate.mapping.row(),
            };
            if let Some(c) = &mut completion {
                c.processed_rates += 1;
                c.update(
                    h,
                    index,
                    section,
                    anchor,
                    (next_rate < index.sections().len()).then_some(next_rate),
                    settings,
                )?;
            }
            continue;
        }
        let y = h.y(time, event.chart_position(), anchor)?;
        let trailing = y - viewport.half_arrow * sign;
        let beyond = if sign > 0.0 {
            trailing > viewport.bottom
        } else {
            trailing < viewport.top
        };
        if beyond && !h.negative_scroll() {
            reached_end = true;
            break;
        }
        let decision = admission.offer(if matches!(event, Event::Note(_)) {
            Kind::Lane
        } else {
            Kind::Misc
        });
        if decision.admit {
            plan.events.push(event);
            if let Some(head)=picked_head{plan.heads.push(head);}
            if let Some(i) = widget_index {
                plan.misc.push((i, y));
            }
        }
        if decision.stop {
            plan.stopped_by_quota = true;
            break;
        }
    }
    if let Some(c) = &mut completion {
        finish_region(
            c,
            index,
            &mut section,
            &mut anchor,
            &mut h,
            settings,
            max_rates,
            reached_end,
        )?;
    }
    plan.completion = completion;
    Ok(plan)
}

fn finish_region(
    c: &mut RegionCompletion,
    index: &Index,
    section: &mut usize,
    anchor: &mut Anchor,
    h: &mut Helper,
    settings: Settings,
    max_rates: i32,
    reached_end: bool,
) -> Result<(), Error> {
    let mut next_rate = *section + 1;
    if !reached_end {
        while next_rate < index.sections().len()
            && !c.complete()
            && max_rates > 0
            && c.processed_rates < max_rates as usize
        {
            let rate = index.sections()[next_rate];
            let y = h.y(rate.mapping.time(), rate.mapping.row(), *anchor)?;
            *section = next_rate;
            next_rate += 1;
            *h = coefficients(index, *section, settings)?;
            *anchor = Anchor {
                y,
                time: rate.mapping.time(),
                row: rate.mapping.row(),
            };
            c.processed_rates += 1;
            c.extra_steps += 1;
            c.update(
                *h,
                index,
                *section,
                *anchor,
                (next_rate < index.sections().len()).then_some(next_rate),
                settings,
            )?;
        }
    }
    c.fallback = !c.complete();
    c.update(*h, index, *section, *anchor, None, settings)?;
    Ok(())
}
#[cfg(test)]
mod region_completion_tests {
    use super::*;
    use crate::{
        generation::editor_timing::SpacingSection, rate_mapping::RateMapping, spacing::Speeds,
        zoom::SpacingMode,
    };
    #[test]
    fn original_complementary_loop_helpers_and_end_fallback_match_7560_cases() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/key_commands/region-completion-source.json"
        ))
        .unwrap();
        let index = Index::new(
            [0.0, 48.0, 96.0]
                .into_iter()
                .map(|row| SpacingSection {
                    order: 0,
                    mapping: RateMapping::new(
                        row,
                        if row == 96.0 { 0.75 } else { row / 96.0 },
                        0.0,
                        0.0,
                        if row == 48.0 { 1.0 / 192.0 } else { 1.0 / 96.0 },
                    )
                    .unwrap(),
                    tempo: if row == 48.0 { 240.0 } else { 120.0 },
                    scroll: if row == 96.0 { -1.0 } else { 1.0 },
                })
                .collect(),
        )
        .unwrap();
        for case in cases.as_array().unwrap() {
            let n = |key: &str| case[key].as_f64().unwrap();
            let mut section = case["initial"].as_u64().unwrap() as usize;
            let m = index.sections()[section].mapping;
            let mut anchor = Anchor {
                y: 200.0,
                time: m.time(),
                row: m.row(),
            };
            let settings = Settings {
                mode: match case["spacing"].as_u64().unwrap() {
                    0 => SpacingMode::ConstantTime,
                    1 => SpacingMode::ConstantRow,
                    _ => SpacingMode::Variable,
                },
                reverse: case["reverse"].as_bool().unwrap(),
                speeds: Speeds::default(),
                interpolated_scroll: 1.0,
                zoom: 1.0,
            };
            let mut c = RegionCompletion::new(Region {
                start_time: n("start") / 96.0,
                start_row: n("start"),
                current_y: n("current"),
            })
            .unwrap();
            let mut h = coefficients(&index, section, settings).unwrap();
            c.update(
                h,
                &index,
                section,
                anchor,
                (section + 1 < index.sections().len()).then_some(section + 1),
                settings,
            )
            .unwrap();
            finish_region(
                &mut c,
                &index,
                &mut section,
                &mut anchor,
                &mut h,
                settings,
                case["max"].as_i64().unwrap() as i32,
                case["reached"].as_bool().unwrap(),
            )
            .unwrap();
            let current = c.current().unwrap().as_ref().unwrap();
            assert_eq!(
                (
                    c.used_fallback(),
                    c.processed_rates(),
                    c.extra_steps(),
                    c.start_y().unwrap(),
                    current.0,
                    current.1
                ),
                (
                    case["fallback"].as_bool().unwrap(),
                    case["processed"].as_u64().unwrap() as usize,
                    case["extra"].as_u64().unwrap() as usize,
                    n("startY"),
                    n("time"),
                    n("row")
                ),
                "{case}"
            );
        }
    }

    #[test]
    fn signed_zero_scroll_uses_original_fumen_search_sentinels_without_nonfinite_positions() {
        use crate::{document::{Document, Format}, history::Session, notes::Note};
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/key_commands/collapsed-viewport-source.json"
        ))
        .unwrap();
        for case in cases.as_array().unwrap() {
            let zero = if case["negativeZero"].as_bool().unwrap() {
                -0.0
            } else {
                0.0
            };
            let index = Index::new(vec![SpacingSection {
                order: 0,
                mapping: RateMapping::new(0.0, 0.0, 0.0, 0.0, 1.0 / 96.0).unwrap(),
                tempo: 120.0,
                scroll: zero,
            }])
            .unwrap();
            let mut grid = Grid::new(4, 1).unwrap();
            for row in [12, 96] {
                grid.set(
                    Position {
                        row,
                        lane: 0,
                        player: 0,
                    },
                    Some(Note::tap()),
                )
                .unwrap();
            }
            let session = Session::new(
                Document::parse(
                    b"#VERSION:0.83;#BPMS:0=120;#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:0000;"
                        .to_vec(),
                    Format::Ssc,
                )
                .unwrap(),
            );
            let clock = Clock::capture(&session, 0).unwrap();
            let misc =
                [(0, TimingKind::Signature), (1, TimingKind::Tempo)].map(|(phase, kind)| Entry {
                    event: Event::Timing { row: 0, kind },
                    phase,
                    chart_time: 0.0,
                    widget: None,
                });
            let plan = prepare_with_region(
                &grid,
                &clock,
                &index,
                &misc,
                Anchor {
                    row: 0.0,
                    time: 0.0,
                    y: 160.0,
                },
                Settings {
                    mode: SpacingMode::Variable,
                    reverse: case["reverse"].as_bool().unwrap(),
                    speeds: Speeds::default(),
                    interpolated_scroll: 1.0,
                    zoom: 1.0,
                },
                Viewport {
                    top: 0.0,
                    bottom: 480.0,
                    half_arrow: 32.0,
                    hold_cap: 32.0,
                },
                0,
                true,
                Some(Region {
                    start_time: 0.125,
                    start_row: 12.0,
                    current_y: 160.0,
                }),
                0,
            )
            .unwrap();
            assert_eq!(plan.events().len(), 1);
            let event = plan.events()[0];
            assert!(event.chart_position().is_finite());
            let id = match event {
                Event::Timing {
                    kind: TimingKind::Signature,
                    ..
                } => 0,
                Event::Note(p) if p.row == 96 => 3,
                _ => panic!("unexpected {event:?}"),
            };
            assert_eq!(id, case["index"].as_u64().unwrap(), "{case}");
            let report = plan.completion().unwrap();
            assert_eq!(report.start_y(), Some(160.0));
            assert!(report.current().is_some_and(|result| result.is_err()));
        }
    }
}
