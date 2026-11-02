use super::*;
use deadlib_present::actors::SizeSpec;
use deadlib_render_core::{BlendMode, MeshVertex};

const CONTROLS: [&str; 8] = [
    "PerformancePaused",
    "PerformanceScale",
    "PerformanceExplicit",
    "PerformanceFrames",
    "PerformanceRestore",
    "PerformanceApply",
    "PerformancePlayback",
    "PerformanceClose",
];
const TIMING_KEYS: [&str; 9] = [
    "PerformanceEntireFrame",
    "PerformanceCpu",
    "PerformanceUpdate",
    "PerformanceChart",
    "PerformanceMinimap",
    "PerformanceWaveform",
    "PerformanceDraw",
    "PerformancePresent",
    "PerformanceWait",
];

fn scalar(value: f64) -> String {
    if value.is_finite() {
        format!("{value:.6}")
    } else {
        i18n::tr("Edition", "PerformanceUnavailable").to_string()
    }
}

impl State {
    pub(in crate::screens::edition) fn performance_actors(
        &self,
        index: usize,
        actors: &mut Vec<Actor>,
    ) {
        let width = (screen_width() - 40.0).clamp(1.0, 720.0);
        let x = (screen_width() - width) * 0.5;
        let summary = self.performance_summary();
        let draft = self
            .performance
            .draft
            .unwrap_or(self.performance.preferences);
        actors.push(act!(quad:xy(screen_width()*0.5,screen_height()*0.5):zoomto(width+20.0,(screen_height()-40.0).max(1.0)):diffuse(0.035,0.05,0.085,0.99):z(8050)));
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","PerformanceTitle")):align(0.0,0.0):xy(x,25.0):zoom(0.8):maxwidth(width):z(8051)));
        if index < 8 {
            for (row, key) in CONTROLS.iter().enumerate() {
                let start = actors.len();
                let value = match row {
                    0 => i18n::tr(
                        "Edition",
                        if draft.paused {
                            "WaveformYes"
                        } else {
                            "WaveformNo"
                        },
                    )
                    .to_string(),
                    1 => match draft.scale {
                        Scale::Independent => {
                            i18n::tr("Edition", "PerformanceIndependent").to_string()
                        }
                        Scale::Shared => i18n::tr("Edition", "PerformanceShared").to_string(),
                        Scale::Explicit => i18n::tr("Edition", "PerformanceFixed").to_string(),
                        Scale::Unknown(value) => value.to_string(),
                    },
                    2 => scalar(draft.explicit_seconds),
                    3 => draft.max_frames.to_string(),
                    _ => String::new(),
                };
                let label = if let Some(label) = self.preference_numeric_label(row) {
                    label
                } else if row < 4 {
                    i18n::tr_fmt("Edition", key, &[("value", &value)])
                } else {
                    i18n::tr("Edition", key)
                };
                actors.push(act!(text:font("miso"):settext(label):align(0.0,0.0):xy(x,65.0+row as f32*29.0):zoom(0.62):maxwidth(width):diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0):z(8051)));
                self.ui_row(actors, start, row == index, 31.0);
            }
            let overview = summary.as_ref().map_or_else(
                || i18n::tr("Edition", "PerformanceEmpty"),
                |summary| {
                    i18n::tr_fmt(
                        "Edition",
                        "PerformanceOverview",
                        &[
                            ("average", &scalar(summary.frame_average * 1000.0)),
                            ("fps", &scalar(1.0 / summary.frame_average)),
                            ("current", &scalar(f64::from(summary.current[0]) * 1000.0)),
                            ("current_fps", &scalar(1.0 / f64::from(summary.current[0]))),
                        ],
                    )
                },
            );
            actors.push(act!(text:font("miso"):settext(overview):align(0.0,0.0):xy(x,310.0):zoom(0.58):wrapwidthpixels(width/0.58):maxwidth(width):z(8051)));
        } else if let Some(summary) = &summary {
            let page = (index - 8) / 3;
            for timing in page * 3..(page * 3 + 3).min(TIMING_KEYS.len()) {
                let start = actors.len();
                let top = 65.0 + (timing - page * 3) as f32 * 120.0;
                let label = i18n::tr_fmt(
                    "Edition",
                    "PerformanceTiming",
                    &[
                        ("name", &i18n::tr("Edition", TIMING_KEYS[timing])),
                        (
                            "average",
                            &scalar(f64::from(summary.averages[timing]) * 1000.0),
                        ),
                        (
                            "current",
                            &scalar(f64::from(summary.current[timing]) * 1000.0),
                        ),
                    ],
                );
                actors.push(act!(text:font("miso"):settext(label):align(0.0,0.0):xy(x,top):zoom(0.53):maxwidth(width):diffuse(1.0,if timing+8==index{0.8}else{1.0},if timing+8==index{0.3}else{1.0},1.0):z(8051)));
                let maximum = if summary.plot_scale == f32::MAX {
                    summary.maxima[timing]
                } else {
                    summary.plot_scale
                };
                let scale = i18n::tr_fmt(
                    "Edition",
                    "PerformancePlotMaximum",
                    &[("value", &scalar(f64::from(maximum) * 1000.0))],
                );
                actors.push(act!(text:font("miso"):settext(scale):align(0.0,0.0):xy(x,top+15.0):zoom(0.42):maxwidth(width):z(8051)));
                actors.push(act!(quad:xy(x+width*0.5,top+70.0):zoomto(width,80.0):diffuse(if timing<6{0.12}else{0.24},0.18,0.2,1.0):z(8051)));
                actors.push(plot(summary, timing, x, top + 30.0, width, 80.0));
                self.ui_row(actors, start, timing + 8 == index, 120.0);
            }
        } else {
            actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","PerformanceEmpty")):align(0.0,0.0):xy(x,90.0):zoom(0.65):maxwidth(width):z(8051)));
        }
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","PerformanceHelp")):align(0.0,1.0):xy(x,screen_height()-18.0):zoom(0.43):wrapwidthpixels(width/0.43):maxwidth(width):z(8052)));
    }
}

/// One bounded mesh per plot; no actor or allocation per sample.
fn plot(summary: &Summary, timing: usize, x: f32, y: f32, width: f32, height: f32) -> Actor {
    let max = if summary.plot_scale == f32::MAX {
        summary
            .frames
            .iter()
            .map(|frame| frame[timing])
            .fold(0.0_f32, f32::max)
    } else {
        summary.plot_scale
    };
    let mut vertices = Vec::with_capacity(summary.frames.len().saturating_sub(1) * 6);
    let point = |index: usize| {
        let value = summary.frames[index][timing];
        let percent = if max > 0.0 && max.is_finite() && value.is_finite() {
            (value / max).clamp(0.0, 1.0)
        } else {
            0.0
        };
        [
            index as f32 / summary.frames.len().saturating_sub(1).max(1) as f32 * width,
            height * (1.0 - percent),
        ]
    };
    for index in 1..summary.frames.len() {
        let a = point(index - 1);
        let b = point(index);
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let length = dx.hypot(dy).max(f32::MIN_POSITIVE);
        let normal = [-dy / length * 0.5, dx / length * 0.5];
        let vertex = |p: [f32; 2], sign: f32| MeshVertex {
            pos: [p[0] + normal[0] * sign, p[1] + normal[1] * sign],
            color: [0.95, 0.85, 0.3, 1.0],
        };
        vertices.extend([
            vertex(a, 1.0),
            vertex(a, -1.0),
            vertex(b, 1.0),
            vertex(b, 1.0),
            vertex(a, -1.0),
            vertex(b, -1.0),
        ]);
    }
    Actor::Mesh {
        align: [0.0, 0.0],
        offset: [x, y],
        size: [SizeSpec::Px(width), SizeSpec::Px(height)],
        tint: [1.0; 4],
        vertices: vertices.into(),
        visible: true,
        blend: BlendMode::Alpha,
        z: 8052,
    }
}
