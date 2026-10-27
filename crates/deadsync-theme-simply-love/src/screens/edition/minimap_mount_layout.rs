//! Native simplified-editor allocation, distinct from GrooveAuthor's XNA layout.
//! The renderer must consume `misc_start`/`misc_end` as its label allocation too.
//! Waveform is currently clipped to the lane field; no absent wider region is added.
#[derive(Clone, Copy, Debug)]
pub(super) struct Input {
    pub left: f32,
    pub lane_width: f32,
    pub columns: u16,
    pub size_zoom: f32,
    pub clip_right: f32,
    /// Actual allocated misc text width, in screen pixels, not a GA constant.
    pub misc_width: f32,
    /// Participant indicator actually rendered to the right of multiplayer lanes.
    pub multiplayer: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Layout {
    pub field_right: f32,
    pub misc_start: f32,
    pub misc_end: f32,
    pub without_scale: f32,
    pub with_scale: f32,
}
pub(super) fn layout(input: Input) -> Option<Layout> {
    let Input {
        left,
        lane_width,
        columns,
        size_zoom,
        clip_right,
        misc_width,
        multiplayer,
    } = input;
    if columns == 0
        || ![left, lane_width, size_zoom, clip_right, misc_width]
            .iter()
            .all(|x| x.is_finite())
        || lane_width <= 0.0
        || size_zoom <= 0.0
        || misc_width < 0.0
    {
        return None;
    }
    let field_right = left + lane_width * f32::from(columns);
    let center_last = field_right - lane_width * 0.5;
    // Centers are fixed in the native renderer; only sprite widths scale.
    let scaled_note_right = center_last + lane_width * size_zoom * 0.5;
    let player_width = if multiplayer {
        40.0 * lane_width / 128.0
    } else {
        0.0
    };
    let player_right = field_right + player_width * size_zoom;
    let misc_start = field_right.max(player_right) + 12.0;
    let available = (clip_right - 12.0 - misc_start).max(0.0);
    // label_actors enforces a 20px minimum text width. Omit its allocation
    // when less space exists, rather than claiming a smaller rendered edge.
    let width = if available >= 20.0 && misc_width >= 20.0 {
        misc_width.min(available)
    } else {
        0.0
    };
    let misc_end = if width > 0.0 {
        misc_start + width
    } else {
        field_right
    };
    let result = Layout {
        field_right,
        misc_start,
        misc_end,
        without_scale: (field_right + player_width).max(misc_end),
        with_scale: field_right
            .max(scaled_note_right)
            .max(player_right)
            .max(misc_end),
    };
    [
        result.field_right,
        result.misc_start,
        result.misc_end,
        result.without_scale,
        result.with_scale,
    ]
    .iter()
    .all(|x| x.is_finite())
    .then_some(result)
}
