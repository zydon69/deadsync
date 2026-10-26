//! Pinned GrooveAuthor MiniMap projection and scrollbar math, without rendering.
//! The viewport clamps within the full range; editor overscroll remains unbounded.
#[derive(Clone, Copy, Debug)]
pub struct Projection {
    pub full_start: f64,
    pub full_end: f64,
    pub mini_start: f64,
    pub mini_unquantized: f64,
    pub mini_range: f64,
    pub editor_start: f64,
    pub editor_end: f64,
    pub bounds_y: i32,
    pub visible_height: i32,
    pub grabbed_percentage: f64,
    pub reverse: bool,
    pub quantize: bool,
}
impl Projection {
    /// Reject malformed geometry before arithmetic. Keyboard focus adapters can
    /// use the same projection while choosing their own explicit movement step.
    pub fn initialized(mut self) -> Option<Self> {
        if ![
            self.full_start,
            self.full_end,
            self.mini_start,
            self.mini_range,
            self.editor_start,
            self.editor_end,
            self.grabbed_percentage,
        ]
        .into_iter()
        .all(f64::is_finite)
            || self.full_end <= self.full_start
            || self.mini_range <= 0.0
            || self.editor_end < self.editor_start
            || self.visible_height <= 0
        {
            return None;
        }
        self.set_mini_start(self.mini_start)?;
        Some(self)
    }
    fn set_mini_start(&mut self, start: f64) -> Option<()> {
        if !start.is_finite() {
            return None;
        }
        let mut projected = start;
        if self.quantize {
            let chart_space_per_pixel = self.mini_range / f64::from(self.visible_height);
            let pixels = start / chart_space_per_pixel;
            if !pixels.is_finite() || pixels < f64::from(i32::MIN) || pixels >= f64::from(i32::MAX)
            {
                return None;
            }
            let mut num_pixels = pixels as i32;
            let remainder = start / (f64::from(num_pixels) * chart_space_per_pixel);
            if remainder >= 0.5 {
                num_pixels += 1;
            }
            projected = f64::from(num_pixels) * chart_space_per_pixel;
        }
        self.mini_unquantized = start;
        self.mini_start = projected;
        Some(())
    }
    pub fn reflect_y(self, screen_y: i32) -> Option<i32> {
        if self.reverse {
            self.bounds_y
                .checked_mul(2)?
                .checked_add(self.visible_height)?
                .checked_sub(screen_y)
        } else {
            Some(screen_y)
        }
    }
    /// Unquantized editor rectangles always use the unquantized viewport start.
    pub fn project_editor(self, position: f64) -> f64 {
        (position - self.mini_unquantized) * (f64::from(self.visible_height) / self.mini_range)
    }
    pub fn position_at_pixel(self, screen_y: f64) -> f64 {
        self.mini_start
            + (screen_y - f64::from(self.bounds_y)) / f64::from(self.visible_height)
                * self.mini_range
    }
    /// Screen input is reflected once. Disabled wide viewports leave state intact.
    /// Original drag arithmetic deliberately permits editor overscroll.
    pub fn drag(&mut self, screen_y: i32) -> Option<()> {
        let screen_y = self.reflect_y(screen_y)?;
        if self.editor_end - self.editor_start >= self.mini_range {
            return Some(());
        }
        let selected_pixel = f64::from(screen_y) - f64::from(self.bounds_y);
        let editor_pixel_range =
            self.project_editor(self.editor_end) - self.project_editor(self.editor_start);
        let editor_start_pixel = selected_pixel - editor_pixel_range * self.grabbed_percentage;
        let editor_start_range = f64::from(self.visible_height) - editor_pixel_range;
        let percentage = editor_start_pixel / editor_start_range;
        let mut next = *self;
        if self.mini_range < self.full_end - self.full_start {
            let mini_start_range = self.full_end - self.full_start - self.mini_range;
            next.set_mini_start(self.full_start + percentage.clamp(0.0, 1.0) * mini_start_range)?;
        }
        let editor_range = self.editor_end - self.editor_start;
        next.editor_start = next.mini_start + percentage * (self.mini_range - editor_range);
        next.editor_end = next.editor_start + editor_range;
        if !next.editor_start.is_finite() || !next.editor_end.is_finite() {
            return None;
        }
        *self = next;
        Some(())
    }
}
