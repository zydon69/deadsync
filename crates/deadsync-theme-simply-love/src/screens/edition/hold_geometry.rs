//! GrooveAuthor EditorHoldNoteEvent.Draw/DrawBody geometry (48e18f71, MIT).
//! Coordinates are source sprite top edges, before actor-center conversion.
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub head_top: f64,
    /// Source H: extended tail edge minus head top. ActiveEditorChart adds
    /// the cap extent in the scroll direction before computing this value.
    pub height: f64,
    pub scale: f64,
    pub arrow_height: u32,
    pub body_height: u32,
    pub cap_height: u32,
    pub start_height: u32,
    pub cap_centered: bool,
    pub reverse: bool,
    pub screen_height: f64,
    pub cutoff: Option<f64>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Strip {
    pub top: f64,
    pub height: f64,
    pub source_top: u32,
    pub source_height: u32,
}
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub head_top: f64,
    pub body: Vec<Strip>,
    pub start: Option<Strip>,
    pub cap: Option<Strip>,
    pub partial: bool,
}
impl Input {
    pub fn plan(self, body_budget: usize) -> Result<Plan, &'static str> {
        if ![self.head_top, self.height, self.scale, self.screen_height]
            .iter()
            .all(|n| n.is_finite())
            || self.cutoff.is_some_and(|n| !n.is_finite())
            || self.scale <= 0.0
            || self.screen_height < 0.0
            || self.arrow_height == 0
            || self.body_height == 0
        {
            return Err("Invalid hold geometry");
        }
        let half_arrow = f64::from(self.arrow_height) * 0.5 * self.scale;
        let pivot = self.head_top + half_arrow;
        let reflect = |top: f64, height: f64| {
            if self.reverse {
                2.0 * pivot - (top + height)
            } else {
                top
            }
        };
        let mut body_y = pivot;
        let mut note_h = if self.reverse {
            -self.height + half_arrow
        } else {
            self.height - half_arrow
        };
        if let Some(cutoff) = self.cutoff {
            let new_y = if self.reverse {
                2.0 * pivot - cutoff
            } else {
                cutoff
            };
            note_h -= new_y - body_y;
            body_y = new_y;
        }
        let cap_h = f64::from(self.cap_height) * self.scale;
        let cap_y = body_y + note_h - cap_h;
        let mut y = cap_y;
        let mut min_y = body_y;
        let minimum_cap_y = if self.cap_centered {
            y += cap_h * 0.5;
            self.head_top
        } else {
            body_y
        };
        let start_h = f64::from(self.start_height) * self.scale;
        let draw_start = !(self.reverse && start_h > 0.0);
        if !draw_start {
            min_y = y.min(min_y + start_h);
        }
        let screen_top = if self.reverse {
            2.0 * pivot - self.screen_height
        } else {
            0.0
        };
        let screen_bottom = if self.reverse {
            2.0 * pivot
        } else {
            self.screen_height
        };
        let tile_h = f64::from(self.body_height) * self.scale;
        if y > screen_bottom + tile_h {
            y -= ((y - (screen_bottom + tile_h)) / tile_h).trunc() * tile_h;
        }
        let mut body = Vec::new();
        let mut partial = false;
        while y >= min_y {
            let h = tile_h.min(y - min_y);
            if h == 0.0 {
                break;
            }
            y -= h;
            if y < screen_top - tile_h {
                break;
            }
            if body.len() == body_budget {
                partial = true;
                break;
            }
            let source_h = if h < tile_h {
                (f64::from(self.body_height) * (h / tile_h)) as u32
            } else {
                self.body_height
            };
            body.push(Strip {
                top: reflect(y, h),
                height: h,
                source_top: self.body_height - source_h,
                source_height: source_h,
            });
        }
        let start = (draw_start && self.start_height > 0).then(|| Strip {
            top: reflect(min_y - start_h, start_h),
            height: start_h,
            source_top: 0,
            source_height: self.start_height,
        });
        let cap = (cap_y > screen_top - cap_h && cap_y < screen_bottom && cap_y >= minimum_cap_y)
            .then(|| Strip {
                top: reflect(cap_y, cap_h),
                height: cap_h,
                source_top: 0,
                source_height: self.cap_height,
            });
        let head_top = (if self.reverse {
            2.0 * pivot - body_y
        } else {
            body_y
        }) - half_arrow;
        Ok(Plan {
            head_top,
            body,
            start,
            cap,
            partial,
        })
    }
}
