//! The original application-lifetime DPI decision, separate from raw preferences.
use super::General;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Capture {
    scale: f64,
    monitor: f64,
}
impl Default for Capture {
    fn default() -> Self {
        Self {
            scale: 1.,
            monitor: 1.,
        }
    }
}
impl Capture {
    pub fn new(preferences: &General, monitor: f64) -> Self {
        let mut scale = if preferences.custom_dpi {
            preferences.dpi_scale
        } else {
            0.
        };
        if crate::number::reference_equal(scale, 0.) {
            scale = monitor;
        }
        if scale <= 0. {
            scale = 1.;
        }
        Self { scale, monitor }
    }
    pub fn scale(self) -> f64 {
        self.scale
    }
    /// Exact source UiScaled conversion for representable native integer metrics.
    pub fn ui_scaled(self, value: i32) -> Result<i32, crate::Error> {
        let n = (f64::from(value) * self.scale).trunc();
        if !n.is_finite() || n < f64::from(i32::MIN) || n > f64::from(i32::MAX) {
            return Err(crate::Error::InvalidEdit(
                "DPI metric exceeds native integer range".into(),
            ));
        }
        Ok(n as i32)
    }
    pub fn font_pixels(self) -> Result<i32, crate::Error> {
        self.ui_scaled(15)
    }
    /// The monitor is intentionally captured with the source's application cache.
    pub fn pending_restart(self, preferences: &General) -> bool {
        !crate::number::reference_equal(Self::new(preferences, self.monitor).scale, self.scale)
    }
}
/// Native keyboard UI pages preserve the requested font scale rather than
/// shrinking controls to fit. Every selected row belongs to exactly one page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Page {
    pub first: usize,
    pub end: usize,
    pub translation: f32,
    pub number: usize,
    pub count: usize,
}
impl Page {
    pub fn rows(centers: &[f32], selected: usize, pitch: f32, height: f32) -> Option<Self> {
        if centers.is_empty()
            || selected >= centers.len()
            || !pitch.is_finite()
            || pitch <= 0.
            || !height.is_finite()
            || height <= 0.
            || centers.iter().any(|v| !v.is_finite())
        {
            return None;
        }
        let padding = 16_f32.min(height * 0.25);
        let capacity = (((height - 2. * padding) / pitch).floor() as usize).max(1);
        let first = selected / capacity * capacity;
        let end = (first + capacity).min(centers.len());
        let count = centers.len().div_ceil(capacity);
        let fits = centers[0] >= padding && centers[centers.len() - 1] <= height - padding;
        let translation = if first == 0 && end == centers.len() && fits {
            0.
        } else {
            (if capacity == 1 { height * 0.5 } else { padding }) - centers[first]
        };
        Some(Self {
            first,
            end,
            translation,
            number: selected / capacity + 1,
            count,
        })
    }
}
/// Native bitmap-widget baseline adaptation. Source ImGui at DPI1 measures a
/// 23px row; the existing native Miso widget row is20px. Normalize the original
/// style/font rules to the supplied native baseline rather than claiming equal
/// font metrics. The same resulting rectangles must drive rendering/selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WidgetMetrics {
    pub width: f64,
    pub height: f64,
    pub padding: f64,
    pub font_scale: f64,
    pub inset_x: f64,
    pub inset_y: f64,
}
impl Capture {
    pub fn native_widget_metrics(
        self,
        width: f64,
        height: f64,
    ) -> Result<WidgetMetrics, crate::Error> {
        if !width.is_finite() || width < 18. || !height.is_finite() || height <= 0. {
            return Err(crate::Error::InvalidEdit(
                "Invalid native widget baseline".into(),
            ));
        }
        let pixels = self.font_pixels()?;
        if pixels <= 0 {
            return Err(crate::Error::InvalidEdit(
                "DPI font size is not drawable".into(),
            ));
        }
        // Real pinned cimgui ScaleAllSizes truncates each f32 style dimension.
        let frame_padding = (3_f32 * (self.scale as f32)).trunc();
        if !frame_padding.is_finite() {
            return Err(crate::Error::InvalidEdit(
                "DPI style exceeds native range".into(),
            ));
        }
        let font_scale = f64::from(pixels) / 15.;
        let source_height = f64::from(frame_padding) * 2. + f64::from(pixels) + 2.;
        let result = WidgetMetrics {
            width: (width - 18.) * font_scale + f64::from(self.ui_scaled(18)?),
            height: height * source_height / 23.,
            padding: f64::from(self.ui_scaled(2)?),
            font_scale,
            inset_x: f64::from(self.ui_scaled(2)?),
            inset_y: f64::from(self.ui_scaled(1)?),
        };
        if !result.width.is_finite() || !result.height.is_finite() {
            return Err(crate::Error::InvalidEdit(
                "DPI widget exceeds native range".into(),
            ));
        }
        Ok(result)
    }
}
