//! Validated native waveform controls from GrooveAuthor PreferencesWaveForm.
//! Defaults/ranges follow UIWaveFormPreferences at pinned commit 48e18f71 (MIT).
use crate::{Error, waveform_palette::Palette, waveform_spacing::ScrollMode};
use serde_json::{Map, Value};

static RUNTIME_LOAD_PARALLELISM: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(8);
pub fn initialize_runtime_defaults(logical_processors: usize) {
    let value = runtime_default_for_processors(logical_processors);
    RUNTIME_LOAD_PARALLELISM.store(value, std::sync::atomic::Ordering::Release);
}
fn runtime_default_for_processors(logical_processors: usize) -> i32 {
    logical_processors
        .saturating_sub(6)
        .max(2)
        .min(i32::MAX as usize) as i32
}
fn default_load_parallelism() -> i32 {
    RUNTIME_LOAD_PARALLELISM.load(std::sync::atomic::Ordering::Acquire)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawLocation {
    FocusedChart,
    AllCharts,
    AllChartsWithSameMusic,
    Unknown(i32),
}
impl DrawLocation {
    pub const ALL: [Self; 3] = [
        Self::FocusedChart,
        Self::AllCharts,
        Self::AllChartsWithSameMusic,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::FocusedChart => "FocusedChart",
            Self::AllCharts => "AllCharts",
            Self::AllChartsWithSameMusic => "AllChartsWithSameMusic",
            Self::Unknown(_) => "Unknown",
        }
    }
    pub fn draws_chart(
        self,
        focused: bool,
        same_music: bool,
        offset: f64,
        focused_offset: f64,
    ) -> bool {
        match self {
            Self::FocusedChart => focused,
            Self::AllCharts => true,
            Self::Unknown(_) => false,
            Self::AllChartsWithSameMusic => {
                focused
                    || (same_music
                        && if offset.is_sign_negative() != focused_offset.is_sign_negative() {
                            offset == focused_offset
                        } else {
                            (offset.to_bits() as i64).abs_diff(focused_offset.to_bits() as i64) <= 1
                        })
            }
        }
    }
    pub fn cycle(self, back: bool) -> Self {
        let index = Self::ALL.iter().position(|v| *v == self).unwrap_or(0);
        Self::ALL[(index + if back { 2 } else { 1 }) % 3]
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preferences {
    antialias: bool,
    subpix: f64,
    edge_threshold: f64,
    edge_threshold_min: f64,
    loading_parallelism: i32,
    show: bool,
    scale_with_zoom: bool,
    size_to_chart: bool,
    location: DrawLocation,
    enabled: bool,
    channel_width: f64,
    dense_scale: f64,
    palette: Palette,
    scroll_mode: ScrollMode,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            antialias: true,
            subpix: 0.2,
            edge_threshold: 0.166,
            edge_threshold_min: 0.0833,
            loading_parallelism: default_load_parallelism(),
            show: true,
            scale_with_zoom: false,
            size_to_chart: true,
            location: DrawLocation::AllChartsWithSameMusic,
            enabled: true,
            channel_width: 0.9,
            dense_scale: 6.0,
            palette: Palette::default(),
            scroll_mode: ScrollMode::MostCommonTempo,
        }
    }
}
impl Preferences {
    pub fn antialias(self) -> bool {
        self.antialias
    }
    pub fn set_antialias(&mut self, value: bool) {
        self.antialias = value;
    }
    pub fn subpix(self) -> f64 {
        self.subpix
    }
    pub fn edge_threshold(self) -> f64 {
        self.edge_threshold
    }
    pub fn edge_threshold_min(self) -> f64 {
        self.edge_threshold_min
    }
    pub fn set_subpix(&mut self, value: f64) -> Result<(), Error> {
        self.subpix = finite_fxaa(value)?;
        Ok(())
    }
    pub fn set_edge_threshold(&mut self, value: f64) -> Result<(), Error> {
        self.edge_threshold = finite_fxaa(value)?;
        Ok(())
    }
    pub fn set_edge_threshold_min(&mut self, value: f64) -> Result<(), Error> {
        self.edge_threshold_min = finite_fxaa(value)?;
        Ok(())
    }

    pub fn loading_parallelism(self) -> i32 {
        self.loading_parallelism
    }
    pub fn set_loading_parallelism(&mut self, value: i32) {
        self.loading_parallelism = value;
    }

    pub fn scale_with_zoom(self) -> bool {
        self.scale_with_zoom
    }
    pub fn size_to_chart(self) -> bool {
        self.size_to_chart
    }
    pub fn location(self) -> DrawLocation {
        self.location
    }
    pub fn set_scale_with_zoom(&mut self, value: bool) {
        self.scale_with_zoom = value;
    }
    pub fn set_size_to_chart(&mut self, value: bool) {
        self.size_to_chart = value;
    }
    pub fn set_location(&mut self, value: DrawLocation) {
        self.location = value;
    }
    /// Editor.GetWaveFormWidth truncates the nonnegative width to uint.
    pub fn render_width(self, chart_width: f64, size_zoom: f64) -> f64 {
        let width = if self.size_to_chart {
            chart_width
        } else {
            1024.
        };
        (width * if self.scale_with_zoom { size_zoom } else { 1. })
            .max(0.)
            .min(u32::MAX as f64)
            .trunc()
    }
    pub fn scroll_mode(&self) -> ScrollMode {
        self.scroll_mode
    }
    pub fn set_scroll_mode(&mut self, mode: ScrollMode) {
        self.scroll_mode = mode;
    }
    pub fn palette(&self) -> Palette {
        self.palette
    }
    pub fn set_palette(&mut self, palette: Palette) {
        self.palette = palette;
    }
    pub fn show(&self) -> bool {
        self.show
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn channel_width(&self) -> f64 {
        self.channel_width
    }
    /// WaveFormRenderer.SetXPerChannelScale clamps the consumer, never the persisted preference.
    pub fn effective_channel_width(self) -> f64 {
        f64::from((self.channel_width as f32).clamp(0.0, 1.0))
    }
    pub fn dense_scale(&self) -> f64 {
        self.dense_scale
    }
    pub fn set_show(&mut self, value: bool) {
        self.show = value;
    }
    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
    }
    pub fn set_channel_width(&mut self, value: f64) -> Result<(), Error> {
        self.channel_width = source_scalar(value)?;
        Ok(())
    }
    pub fn set_dense_scale(&mut self, value: f64) -> Result<(), Error> {
        self.dense_scale = source_scalar(value)?;
        Ok(())
    }
    pub(crate) fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut value = Self::default();
        for (key, target) in [
            ("AntiAlias", &mut value.antialias),
            ("ShowWaveForm", &mut value.show),
            ("WaveFormScaleXWhenZooming", &mut value.scale_with_zoom),
            ("WaveFormScaleWidthToChart", &mut value.size_to_chart),
            ("EnableWaveForm", &mut value.enabled),
        ] {
            if let Some(field) = fields.get(key) {
                *target = field
                    .as_bool()
                    .ok_or_else(|| Error::InvalidFormat(format!("Invalid {key}")))?;
            }
        }
        if let Some(field) = fields.get("WaveFormMaxXPercentagePerChannel") {
            value.set_channel_width(
                field
                    .as_f64()
                    .ok_or_else(|| Error::InvalidFormat("Invalid waveform channel width".into()))?,
            )?;
        }
        if let Some(field) = fields.get("DenseScale") {
            value.set_dense_scale(
                field
                    .as_f64()
                    .ok_or_else(|| Error::InvalidFormat("Invalid waveform dense scale".into()))?,
            )?;
        }
        if let Some(field) = fields.get("RowBasedWaveFormScrollMode") {
            value.scroll_mode = match (field.as_str(), field.as_u64()) {
                (Some("CurrentTempo"), _) | (_, Some(0)) => ScrollMode::CurrentTempo,
                (Some("CurrentTempoAndRate"), _) | (_, Some(1)) => ScrollMode::CurrentTempoAndRate,
                (Some("MostCommonTempo"), _) | (_, Some(2)) => ScrollMode::MostCommonTempo,
                _ => return Err(Error::InvalidFormat("Invalid waveform scroll mode".into())),
            };
        }
        if let Some(field) = fields.get("WaveFormDrawLocation") {
            value.location = match (
                field.as_str(),
                field
                    .as_i64()
                    .or_else(|| field.as_str().and_then(|value| value.parse::<i64>().ok())),
            ) {
                (Some("FocusedChart"), _) | (_, Some(0)) => DrawLocation::FocusedChart,
                (Some("AllCharts"), _) | (_, Some(1)) => DrawLocation::AllCharts,
                (Some("AllChartsWithSameMusic"), _) | (_, Some(2)) => {
                    DrawLocation::AllChartsWithSameMusic
                }
                (_, Some(n)) if i32::try_from(n).is_ok() => DrawLocation::Unknown(n as i32),
                _ => {
                    return Err(Error::InvalidFormat(
                        "Invalid waveform draw location".into(),
                    ));
                }
            };
        }
        if let Some(field) = fields.get("WaveFormLoadingMaxParallelism") {
            let parsed = field
                .as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| {
                    Error::InvalidFormat("Invalid waveform loading parallelism".into())
                })?;
            value.loading_parallelism = if parsed == 0 {
                default_load_parallelism()
            } else {
                parsed
            };
        }
        for (key, target) in [
            ("AntiAliasSubpix", &mut value.subpix),
            ("AntiAliasEdgeThreshold", &mut value.edge_threshold),
            ("AntiAliasEdgeThresholdMin", &mut value.edge_threshold_min),
        ] {
            if let Some(raw) = fields.get(key) {
                *target = finite_fxaa(
                    raw.as_f64()
                        .ok_or_else(|| Error::InvalidFormat(format!("Invalid {key}")))?,
                )?;
            }
        }
        value.palette = Palette::read(fields)?;
        Ok(value)
    }
    pub(crate) fn write(&self, fields: &mut Map<String, Value>) {
        fields.insert(
            "WaveFormLoadingMaxParallelism".into(),
            self.loading_parallelism.into(),
        );
        fields.insert("ShowWaveForm".into(), self.show.into());
        fields.insert(
            "WaveFormScaleXWhenZooming".into(),
            self.scale_with_zoom.into(),
        );
        fields.insert(
            "WaveFormScaleWidthToChart".into(),
            self.size_to_chart.into(),
        );
        fields.insert(
            "WaveFormDrawLocation".into(),
            match self.location {
                DrawLocation::Unknown(n) => {
                    crate::preference_enum::unknown_ordinal(n, fields.get("WaveFormDrawLocation"))
                }
                known => known.name().into(),
            },
        );
        fields.insert("EnableWaveForm".into(), self.enabled.into());
        fields.insert(
            "WaveFormMaxXPercentagePerChannel".into(),
            self.channel_width.into(),
        );
        fields.insert("DenseScale".into(), self.dense_scale.into());
        fields.insert(
            "RowBasedWaveFormScrollMode".into(),
            self.scroll_mode.name().into(),
        );
        fields.insert("AntiAlias".into(), self.antialias.into());
        fields.insert("AntiAliasSubpix".into(), self.subpix.into());
        fields.insert("AntiAliasEdgeThreshold".into(), self.edge_threshold.into());
        fields.insert(
            "AntiAliasEdgeThresholdMin".into(),
            self.edge_threshold_min.into(),
        );
        self.palette.write(fields);
    }
}
fn source_scalar(value: f64) -> Result<f64, Error> {
    if value.is_finite() && (value as f32).is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidEdit(
            "Waveform value must be a finite 32-bit number".into(),
        ))
    }
}

#[cfg(test)]
mod source_layout_tests {
    use super::*;
    #[test]
    fn source_layout_defaults_unknown_enum_and_width_contract_round_trip() {
        let p = Preferences::default();
        assert!(!p.scale_with_zoom());
        assert!(p.size_to_chart());
        assert_eq!(p.location(), DrawLocation::AllChartsWithSameMusic);
        for (to_chart, zoom, expected) in [
            (true, false, 192.),
            (true, true, 288.),
            (false, false, 1024.),
            (false, true, 1536.),
        ] {
            let mut p = p;
            p.set_size_to_chart(to_chart);
            p.set_scale_with_zoom(zoom);
            assert_eq!(p.render_width(192., 1.5), expected);
            p.set_location(DrawLocation::Unknown(99));
            let mut fields = Map::new();
            p.write(&mut fields);
            assert_eq!(Preferences::read(&fields).unwrap(), p);
        }
        let location = DrawLocation::AllChartsWithSameMusic;
        assert!(location.draws_chart(false, true, 1., f64::from_bits(1f64.to_bits() + 1)));
        assert!(!location.draws_chart(false, true, 1., f64::from_bits(1f64.to_bits() + 2)));
        assert!(location.draws_chart(false, true, -0., 0.));
        assert!(!location.draws_chart(false, false, 0., 0.));
        let mut p = p;
        p.set_scale_with_zoom(true);
        assert_eq!(p.render_width(191.9, 0.5), 95.);
        assert_eq!(p.render_width(192., 0.), 0.);
    }
}

#[cfg(test)]
mod parallelism_tests {
    use super::*;
    #[test]
    fn runtime_default_postload_zero_and_raw_values_are_distinct_from_ui_limits() {
        for (processors, expected) in [(1, 2), (6, 2), (8, 2), (14, 8), (256, 250)] {
            assert_eq!(runtime_default_for_processors(processors), expected);
        }
        for raw in [-7, 0, 1, 128, 513, i32::MAX] {
            let fields = Map::from_iter([("WaveFormLoadingMaxParallelism".into(), raw.into())]);
            let p = Preferences::read(&fields).unwrap();
            assert_eq!(
                p.loading_parallelism(),
                if raw == 0 {
                    default_load_parallelism()
                } else {
                    raw
                }
            );
            let mut written = Map::new();
            p.write(&mut written);
            assert_eq!(Preferences::read(&written).unwrap(), p);
        }
    }
}

fn finite_fxaa(value: f64) -> Result<f64, Error> {
    if value.is_finite() && (value as f32).is_finite() {
        Ok(value)
    } else {
        Err(Error::InvalidFormat(
            "Invalid waveform antialias parameter".into(),
        ))
    }
}
#[cfg(test)]
mod fxaa_tests {
    use super::*;
    #[test]
    fn source_fxaa_defaults_and_non_ui_raw_round_trip() {
        let defaults = Preferences::default();
        assert!(defaults.antialias());
        assert_eq!(
            (
                defaults.subpix(),
                defaults.edge_threshold(),
                defaults.edge_threshold_min()
            ),
            (0.2, 0.166, 0.0833)
        );
        for raw in [-2.0, 0.0, 0.75, 2.0] {
            let fields = Map::from_iter([
                ("AntiAliasSubpix".into(), raw.into()),
                ("AntiAliasEdgeThreshold".into(), raw.into()),
                ("AntiAliasEdgeThresholdMin".into(), raw.into()),
            ]);
            let p = Preferences::read(&fields).unwrap();
            assert_eq!(
                (p.subpix(), p.edge_threshold(), p.edge_threshold_min()),
                (raw, raw, raw)
            );
            let mut output = Map::new();
            p.write(&mut output);
            assert_eq!(Preferences::read(&output).unwrap(), p);
        }
    }
}

#[cfg(test)]
mod source_scalar_tests {
    use super::*;
    #[test]
    fn source_preferences_retain_raw_and_only_width_consumer_clamps() {
        for raw in [-10.0, -0.1, 0.0, 0.25, 1.0, 10.01, 512.0] {
            let fields = Map::from_iter([
                ("WaveFormMaxXPercentagePerChannel".into(), raw.into()),
                ("DenseScale".into(), raw.into()),
            ]);
            let p = Preferences::read(&fields).unwrap();
            assert_eq!(p.channel_width(), raw);
            assert_eq!(p.dense_scale(), raw);
            assert_eq!(
                p.effective_channel_width(),
                f64::from((raw as f32).clamp(0.0, 1.0))
            );
            let mut written = Map::new();
            p.write(&mut written);
            assert_eq!(Preferences::read(&written).unwrap(), p);
        }
    }
}
