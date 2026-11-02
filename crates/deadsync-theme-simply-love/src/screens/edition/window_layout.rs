//! Host monitor capabilities only; resolution requests never replace editor state.
use super::*;
#[derive(Default)]
pub(super) struct WindowLayout {
    modes: Vec<(u32, u32)>,
    request: Option<(u32, u32)>,
}
impl State {
    pub fn set_editor_resolutions(&mut self, mut modes: Vec<(u32, u32)>) {
        modes.retain(|&(w, h)| w > 0 && h > 0);
        modes.sort_unstable_by(|a, b| b.cmp(a));
        modes.dedup();
        if self.window_layout.modes != modes {
            self.window_layout.modes = modes;
            if let Some(Panel::WindowLayout(index)) = self.panel {
                self.panel = Some(Panel::WindowLayout(
                    index.min(self.window_layout.modes.len()),
                ));
            }
        }
    }
    pub fn editor_resolution_menu_open(&self) -> bool {
        matches!(self.panel, Some(Panel::WindowLayout(_)))
    }
    pub fn editor_resolution_requested(&self) -> bool {
        self.window_layout.request.is_some()
    }
    pub fn take_editor_resolution(&mut self) -> Option<(u32, u32)> {
        self.window_layout.request.take()
    }
    pub fn editor_resolution_result(&mut self, result: Result<(), String>) {
        self.status = match result {
            Ok(()) => i18n::tr("Edition", "LayoutResolutionApplied"),
            Err(error) => i18n::tr_fmt("Edition", "LayoutResolutionFailed", &[("error", &error)]),
        };
    }
    pub(super) fn open_window_layout(&mut self) {
        if !matches!(
            self.panel,
            None | Some(Panel::EditorHelp(_) | Panel::Menu(_) | Panel::EmptyMenu(_))
        ) {
            return;
        }
        self.lane_entry_release_input();
        self.panel = Some(Panel::WindowLayout(0));
        self.status = i18n::tr("Edition", "LayoutAdaptation");
    }
    pub(super) fn window_layout_key(&mut self, index: usize, code: KeyCode) {
        let len = self.window_layout.modes.len() + 1;
        if code == KeyCode::Escape {
            self.panel = None;
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::WindowLayout(
                (index + if back { len - 1 } else { 1 }) % len,
            ));
            return;
        }
        if matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            if let Some(mode) = self.window_layout.modes.get(index) {
                if self.window_layout.request.is_none() {
                    self.window_layout.request = Some(*mode);
                }
            } else {
                self.panel = None;
            }
        }
    }
    pub(super) fn window_layout_actors(&self, actors: &mut Vec<Actor>, index: usize) {
        let w = (screen_width() - 30.).min(800.);
        let h = (screen_height() - 30.).min(510.);
        let x = screen_width() * 0.5;
        let y = (screen_height() - h) * 0.5;
        actors.push(act!(quad:xy(x,y+h*0.5):setsize(w,h):diffuse(0.04,0.06,0.1,0.98):z(8070)));
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","LayoutOpen")):xy(x,y+25.):zoom(0.65):maxwidth(w-30.):z(8071)));
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","LayoutAdaptation")):align(0.,0.):xy(x-w*0.5+15.,y+60.):zoom(0.45):wrapwidthpixels((w-30.)/0.45):maxheight(110.):z(8071)));
        let visible = ((h - 220.) / 26.).max(1.) as usize;
        let first = index
            .saturating_sub(visible / 2)
            .min((self.window_layout.modes.len() + 1).saturating_sub(visible));
        for (i, row) in self
            .window_layout
            .modes
            .iter()
            .map(|(w, h)| format!("{w} x {h}"))
            .chain(std::iter::once(i18n::tr("Edition", "Close").to_string()))
            .enumerate()
            .skip(first)
            .take(visible)
        {
            let start = actors.len();
            actors.push(act!(text:font("miso"):settext(format!("{} {}",if i==index{">"}else{" "},row)):align(0.,0.):xy(x-w*0.5+15.,y+175.+(i-first)as f32*26.):zoom(0.55):maxwidth(w-30.):z(8071)));
            self.ui_row(actors, start, i == index, 26.);
        }
        actors.push(act!(text:font("miso"):settext(self.status.clone()):xy(x,y+h-22.):zoom(0.4):maxwidth(w-30.):z(8071)));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_modes_only_explicit_request_preserves_document() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::without_song(dir.path()).unwrap();
        state.panel = None;
        let before = state.workspace.file().session.serialize().unwrap();
        state.set_editor_resolutions(vec![(0, 0), (800, 600), (1920, 1080), (800, 600)]);
        assert_eq!(state.window_layout.modes, vec![(1920, 1080), (800, 600)]);
        state.open_window_layout();
        assert!(state.take_editor_resolution().is_none());
        state.window_layout_key(1, KeyCode::Enter);
        assert_eq!(state.take_editor_resolution(), Some((800, 600)));
        state.editor_resolution_result(Err("unavailable".into()));
        assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
        assert!(!state.has_song());
        state.set_editor_resolutions(vec![]);
        assert!(matches!(state.panel, Some(Panel::WindowLayout(0))));
        state.window_layout_key(0, KeyCode::Enter);
        assert!(state.panel.is_none());
        assert!(state.take_editor_resolution().is_none());
    }
}
