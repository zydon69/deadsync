//! Keyboard Hotbar adaptation: exact focused coordinates and direct zoom/snap.
//! Context is prepared on UI open/content refresh; rendering performs no parse/IO.
use super::*;
use deadsync_editor::{
    chart_order::Binding, playback::Clock, snap::Level, timing_edit::signature::Signatures,
};
use std::{cell::RefCell, time::Instant};
#[derive(Default)]
pub(super) struct Runtime {
    context: Option<Context>,
    frame: RefCell<Option<Readout>>,
}
struct Context {
    binding: Binding,
    chart: usize,
    clock: Clock,
    signatures: Signatures,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Readout {
    pub row: f64,
    pub beat: f64,
    pub measure: f64,
    pub music: f64,
    pub chart: f64,
    pub screen: [f32; 2],
}
fn fractional_measure(signatures: &Signatures, row: f64) -> Result<f64, deadsync_editor::Error> {
    if !row.is_finite() {
        return Err(deadsync_editor::Error::InvalidEdit(
            "Nonfinite focus position".into(),
        ));
    }
    let departure = row.max(0.).floor().min(i32::MAX as f64) as u64;
    let start = *signatures
        .values()
        .range(..=departure)
        .next_back()
        .expect("initial signature")
        .0;
    let base = signatures.measure_position(start)?;
    // Source EditorChart.GetMeasure: (row-timeSig.row)/rowsPerMeasure +timeSig.Measure.
    Ok((row - start as f64) / base.rows_per_measure as f64 + base.measure as f64)
}
impl State {
    pub(super) fn refresh_hotbar_context(&mut self) {
        if !self.workspace.has_chart() {
            self.hotbar.context = None;
            return;
        }
        if !matches!(self.panel, Some(Panel::Hotbar(_))) {
            return;
        }
        let session = &self.workspace.file().session;
        let chart = self.workspace.chart();
        if self
            .hotbar
            .context
            .as_ref()
            .is_some_and(|c| c.binding.is_current(session, chart))
        {
            return;
        }
        let result = (|| -> Result<Context, deadsync_editor::Error> {
            Ok(Context {
                binding: Binding::capture(session, chart)?,
                chart,
                clock: Clock::capture(session, chart)?,
                signatures: Signatures::from_document(session.document(), chart)?,
            })
        })();
        match result {
            Ok(context) => self.hotbar.context = Some(context),
            Err(error) => {
                self.hotbar.context = None;
                self.status = i18n::editor_error(&error).into();
            }
        }
    }
    pub(super) fn open_hotbar(&mut self) {
        self.finish_preference_numeric();
        self.minimap_release_input();
        self.receptor_views.release_input();
        self.lane_entry_release_input();
        self.panel = Some(Panel::Hotbar(0));
        self.refresh_hotbar_context();
        self.status = i18n::tr("Edition", "HotbarHint");
    }
    /// Capture the actual chart canvas before UI DPI switches to virtual form metrics.
    pub(super) fn capture_hotbar_frame(&self) {
        let value = (|| -> Option<Readout> {
            let c = self.hotbar.context.as_ref()?;
            if !self.workspace.has_chart()
                || !c
                    .binding
                    .is_current(&self.workspace.file().session, self.workspace.chart())
                || c.chart != self.workspace.chart()
            {
                return None;
            }
            let (chart, row) = self.playback_position().or_else(|| {
                let row = self.workspace.row() as f64;
                c.clock
                    .fractional_chart_time(row)
                    .ok()
                    .map(|time| (time, row))
            })?;
            let focus = self
                .focus_position
                .as_ref()
                .filter(|p| p.is_current(&self.workspace));
            let music = self
                .transport_music_time()
                .or_else(|| focus.map(|p| p.music_time()))
                .or_else(|| c.clock.chart_time_to_music(chart).ok())?;
            let (x, y) = self.receptor_canvas_position();
            Some(Readout {
                row,
                beat: row / 48.,
                measure: fractional_measure(&c.signatures, row).ok()?,
                music,
                chart,
                screen: [x, y],
            })
        })();
        *self.hotbar.frame.borrow_mut() = value;
    }
    #[cfg(test)]
    pub(super) fn hotbar_readout(&self) -> Option<Readout> {
        *self.hotbar.frame.borrow()
    }
    pub(super) fn set_hotbar_zoom(&mut self, value: f64) -> Result<(), deadsync_editor::Error> {
        // UIHotbar casts to float before Editor.SetSpacingZoom. Keep that boundary.
        let value = value as f32;
        if !value.is_finite() {
            return Err(deadsync_editor::Error::InvalidEdit(
                "Invalid preference numeric value".into(),
            ));
        }
        self.zoom.set_zoom(f64::from(value).clamp(
            deadsync_editor::zoom::MIN_ZOOM,
            deadsync_editor::zoom::MAX_ZOOM,
        ))
    }
    pub(super) fn hotbar_key(&mut self, index: usize, code: KeyCode) {
        if code == KeyCode::Escape
            || index == 10 && matches!(code, KeyCode::Enter | KeyCode::NumpadEnter)
        {
            self.panel = Some(Panel::EditorHelp(11));
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || code == KeyCode::Tab && self.shift;
            self.panel = Some(Panel::Hotbar((index + if back { 10 } else { 1 }) % 11));
            return;
        }
        let forward = code != KeyCode::ArrowLeft;
        if index == 7 {
            if code == KeyCode::Home {
                self.command_zoom(None, Instant::now());
            } else if matches!(code, KeyCode::ArrowLeft | KeyCode::ArrowRight) {
                self.command_zoom(Some(if forward { 1. } else { -1. }), Instant::now());
            }
        } else if index == 8
            && matches!(
                code,
                KeyCode::ArrowLeft | KeyCode::ArrowRight | KeyCode::Enter | KeyCode::NumpadEnter
            )
        {
            self.cycle_snap(forward);
        } else if index == 9
            && matches!(
                code,
                KeyCode::ArrowLeft
                    | KeyCode::ArrowRight
                    | KeyCode::Home
                    | KeyCode::Enter
                    | KeyCode::NumpadEnter
            )
        {
            let old = self
                .application_preferences_owner()
                .map(|f| f.snap().lock)
                .unwrap_or_default()
                .source_index();
            let next = if code == KeyCode::Home {
                0
            } else {
                (old + if forward { 1 } else { 9 }) % 10
            };
            if let Err(error) =
                self.set_hotbar_snap_lock(Level::source(next).expect("bounded source lock index"))
            {
                self.status = i18n::editor_error(&error).into();
            }
        }
    }
    fn hotbar_labels(&self) -> Vec<Arc<str>> {
        let value = *self.hotbar.frame.borrow();
        let missing = i18n::tr("Edition", "HotbarUnavailable");
        let numeric = |key, number: Option<f64>| {
            i18n::tr_fmt(
                "Edition",
                key,
                &[(
                    "value",
                    &number
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| missing.to_string()),
                )],
            )
        };
        let screen = value
            .map(|p| format!("{}, {}", p.screen[0], p.screen[1]))
            .unwrap_or_else(|| missing.to_string());
        let snap = Level::native(self.workspace.snap()).ok();
        let lock = self
            .application_preferences_owner()
            .map(|f| f.snap().lock)
            .unwrap_or_default();
        let level = |v: Level| {
            v.denominator()
                .map(|n| format!("1/{n}"))
                .unwrap_or_else(|| i18n::tr("Edition", "SnapOff").to_string())
        };
        vec![
            numeric("HotbarSong", value.map(|p| p.music)),
            numeric("HotbarChart", value.map(|p| p.chart)),
            numeric("HotbarMeasure", value.map(|p| p.measure)),
            numeric("HotbarBeat", value.map(|p| p.beat)),
            numeric("HotbarRow", value.map(|p| p.row)),
            i18n::tr_fmt("Edition", "HotbarScreen", &[("value", &screen)]),
            i18n::tr("Edition", "HotbarPointer"),
            self.preference_numeric_label(7)
                .unwrap_or_else(|| numeric("HotbarZoom", Some(self.spacing_zoom()))),
            i18n::tr_fmt(
                "Edition",
                "HotbarSnap",
                &[(
                    "value",
                    &snap.map(level).unwrap_or_else(|| missing.to_string()),
                )],
            ),
            i18n::tr_fmt("Edition", "HotbarLock", &[("value", &level(lock))]),
            i18n::tr("Edition", "Close"),
        ]
    }
    pub(super) fn hotbar_actors(&self, actors: &mut Vec<Actor>, selected: usize) {
        let width = (screen_width() - 24.).min(760.).max(1.);
        let height = (screen_height() - 24.).min(460.).max(1.);
        let x = screen_width() * 0.5;
        let top = (screen_height() - height) * 0.5;
        let pitch = 25.;
        let capacity = (((height - 100.) / pitch).floor() as usize).max(1).min(11);
        let first = selected.min(10) / capacity * capacity;
        let end = (first + capacity).min(11);
        actors.push(act!(quad:xy(x,top+height*0.5):zoomto(width,height):diffuse(0.04,0.06,0.1,0.98):z(8070)));
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","HotbarTitle")):xy(x,top+19.):zoom(0.65):maxwidth(width-24.):z(8071)));
        for (row, label) in self
            .hotbar_labels()
            .into_iter()
            .enumerate()
            .skip(first)
            .take(end - first)
        {
            let start = actors.len();
            actors.push(act!(text:font("miso"):settext(label):align(0.,0.):xy(x-width*0.5+12.,top+48.+(row-first)as f32*pitch):zoom(0.55):maxwidth(width-24.):diffuse(1.,if row==selected{0.85}else{1.},if row==selected{0.3}else{1.},1.):z(8071)));
            self.ui_row(actors, start, row == selected, pitch);
        }
        let hint = i18n::tr_fmt(
            "Edition",
            "HotbarPage",
            &[
                ("first", &(first + 1).to_string()),
                ("last", &end.to_string()),
            ],
        );
        actors.push(act!(text:font("miso"):settext(hint):xy(x,top+height-22.):zoom(0.4):maxwidth(width-24.):z(8071)));
    }
}

#[cfg(test)]
mod source_measure_tests {
    use super::*;
    #[test]
    fn fractional_measure_uses_active_signature_anchor_and_preserves_negative_rows() {
        let signatures = Signatures::parse("0=4=4,4=3=8").unwrap();
        for row in [-192.5, -12.75, 0., 60.75, 191.5, 192., 200.5, 264.25] {
            let expected = if row < 192. {
                row / 192.
            } else {
                (row - 192.) / 72. + 1.
            };
            assert_eq!(
                fractional_measure(&signatures, row).unwrap().to_bits(),
                expected.to_bits()
            );
        }
    }
}
