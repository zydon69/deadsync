//! About is presentation only: no document, preference, or updater action is committed.
use super::*;
use deadlib_present::{
    actors::Actor,
    space::{screen_height, screen_width},
};

const LICENSES: [&str; 4] = [
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../LICENSE")),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/editor/licenses/GrooveAuthor-MIT.txt"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/editor/licenses/StepManiaLibrary-MIT.txt"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/editor/licenses/Fumen-MIT.txt"
    )),
];
const ROWS: [&str; 6] = [
    "EditorAboutNativeLicense",
    "EditorAboutUpstreamLicense",
    "StepManiaLibrary (MIT)",
    "Fumen (MIT)",
    "",
    "EditorAboutBack",
];
const PAGE_LINES: usize = 12;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LicenseKind {
    DeadSync,
    GrooveAuthor,
    StepManiaLibrary,
    Fumen,
}
impl LicenseKind {
    const ALL: [Self; 4] = [
        Self::DeadSync,
        Self::GrooveAuthor,
        Self::StepManiaLibrary,
        Self::Fumen,
    ];
    fn index(self) -> usize {
        match self {
            Self::DeadSync => 0,
            Self::GrooveAuthor => 1,
            Self::StepManiaLibrary => 2,
            Self::Fumen => 3,
        }
    }
    fn text(self) -> &'static str {
        LICENSES[self.index()]
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AboutView {
    Menu { selected: usize },
    License { kind: LicenseKind, page: usize },
}
#[cfg(test)]
fn last_page(kind: LicenseKind) -> usize { last_page_with_lines(kind, PAGE_LINES) }
fn last_page_with_lines(kind: LicenseKind, lines: usize) -> usize {
    kind.text().lines().count().saturating_sub(1) / lines
}
#[cfg(test)]
fn license_text(kind: LicenseKind, page: usize) -> String { license_text_with_lines(kind, page, PAGE_LINES) }
fn license_text_with_lines(kind: LicenseKind, page: usize, lines: usize) -> String {
    kind.text()
        .lines()
        .skip(page.min(last_page_with_lines(kind, lines)) * lines)
        .take(lines)
        .collect::<Vec<_>>()
        .join("\n")
}
impl State {
    fn editor_license_page_lines(&self) -> usize {
        // The UI host renders read-only pages in this exact virtual viewport.
        // Recompute for input too so Home/End/PageDown address the rendered page.
        let height = (self.ui_dpi_window[1] / self.ui_dpi_render_scale - 30.).min(510.);
        let ratio = (self.ui_dpi.filter(|_| self.ui_dpi_supported).unwrap_or_default().font_pixels().unwrap_or(15) as f32 / 15. / self.ui_dpi_render_scale).max(f32::MIN_POSITIVE);
        ((height - 145.).max(0.) / (24. * 0.45 * ratio)).floor().max(1.).min(PAGE_LINES as f32) as usize
    }
    /// Host-provided updater knowledge is independent of notification suppression/once latch.
    /// An absent or unclassified result must never be presented as “up to date”.
    pub fn observe_editor_about_update(
        &mut self,
        checked: bool,
        available: Option<(String, String)>,
    ) {
        self.notifications.about_update_checked = checked;
        self.notifications.about_update = available.filter(|(tag, url)| {
            !tag.is_empty() && url.starts_with("https://") && !url.contains(['\r', '\n', '\0'])
        });
    }
    fn editor_about_update_label(&self) -> Arc<str> {
        if let Some((tag, _)) = &self.notifications.about_update {
            i18n::tr_fmt("Edition", "EditorAboutAvailable", &[("version", tag)])
        } else {
            i18n::tr(
                "Edition",
                if self.notifications.about_update_checked {
                    "EditorAboutUpdated"
                } else {
                    "EditorAboutUnknown"
                },
            )
        }
    }
    pub(super) fn editor_about_key(&mut self, view: AboutView, code: KeyCode) {
        if let AboutView::License { kind, page } = view {
            let lines = self.editor_license_page_lines();
            let last_page = |kind| last_page_with_lines(kind, lines);
            let page = page.min(last_page(kind));
            let next = match code {
                KeyCode::Escape => {
                    self.panel = Some(Panel::EditorAbout(AboutView::Menu {
                        selected: kind.index(),
                    }));
                    return;
                }
                KeyCode::ArrowDown | KeyCode::PageDown => {
                    page.saturating_add(1).min(last_page(kind))
                }
                KeyCode::ArrowUp | KeyCode::PageUp => page.saturating_sub(1),
                KeyCode::Home => 0,
                KeyCode::End => last_page(kind),
                _ => return,
            };
            self.panel = Some(Panel::EditorAbout(AboutView::License { kind, page: next }));
            return;
        }
        let AboutView::Menu { selected } = view else {
            return;
        };
        let row = selected.min(ROWS.len() - 1);
        if code == KeyCode::Escape {
            self.panel = Some(Panel::EditorHelp(8));
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::EditorAbout(AboutView::Menu {
                selected: (row + if back { ROWS.len() - 1 } else { 1 }) % ROWS.len(),
            }));
            return;
        }
        if !matches!(code, KeyCode::Enter | KeyCode::NumpadEnter) {
            return;
        }
        match row {
            0..=3 => {
                self.panel = Some(Panel::EditorAbout(AboutView::License {
                    kind: LicenseKind::ALL[row],
                    page: 0,
                }))
            }
            4 => {
                if let Some((_, url)) = &self.notifications.about_update {
                    self.notifications.open_url = Some(url.clone());
                }
            }
            _ => self.panel = Some(Panel::EditorHelp(8)),
        }
    }
    pub(super) fn editor_about_actors(&self, actors: &mut Vec<Actor>, view: AboutView) {
        let width = (screen_width() - 30.).min(790.);
        let height = (screen_height() - 30.).min(510.);
        let x = screen_width() * 0.5;
        let top = (screen_height() - height) * 0.5;
        actors.push(act!(quad:xy(x,top+height*0.5):setsize(width,height):diffuse(0.04,0.06,0.1,0.97):z(8070)));
        let version = deadsync_version::current().to_string();
        let title = i18n::tr_fmt("Edition", "EditorAboutVersion", &[("version", &version)]);
        actors.push(act!(text:font("miso"):settext(title):xy(x,top+24.):zoom(0.6):z(8071)));
        if let AboutView::License { kind, page } = view {
            let lines = self.editor_license_page_lines();
            let last_page = |kind| last_page_with_lines(kind, lines);
            let page = page.min(last_page(kind));
            let text = license_text_with_lines(kind, page, lines);
            actors.push(act!(text:font("miso"):settext(text):align(0.,0.):xy(x-width*0.5+15.,top+65.):zoom(0.45):maxwidth(width-30.):maxheight(height-145.):z(8071)));
            let hint = format!(
                "{}/{} — {}",
                page + 1,
                last_page(kind) + 1,
                i18n::tr("Edition", "EditorAboutLicenseHint")
            );
            actors.push(act!(text:font("miso"):settext(hint):xy(x,top+height-35.):zoom(0.4):maxwidth(width-30.):z(8071)));
            return;
        }
        actors.push(act!(text:font("miso"):settext(i18n::tr("Edition","EditorAboutAttribution")):xy(x,top+65.):zoom(0.45):wrapwidthpixels((width-30.)/0.45):maxwidth(width-30.):z(8071)));
        for (row, key) in ROWS.iter().enumerate() {
            let label = if row == 4 {
                self.editor_about_update_label()
            } else if row == 2 || row == 3 {
                Arc::from(*key)
            } else {
                i18n::tr("Edition", key)
            };
            let selected = matches!(view,AboutView::Menu{selected} if selected==row);
            let start = actors.len();
            actors.push(act!(text:font("miso"):settext(format!("{} {}",if selected{">"}else{" "},label)):align(0.,0.):xy(x-width*0.5+15.,top+130.+row as f32*35.):zoom(0.5):maxwidth(width-30.):z(8071)));
            self.ui_row(actors, start, selected, 35.);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn press(state: &mut State, code: KeyCode) {
        for pressed in [true, false] {
            state.raw_key(&deadlib_platform::input::RawKeyboardEvent {
                code,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    fn view(state: &State) -> AboutView {
        match state.panel {
            Some(Panel::EditorAbout(view)) => view,
            _ => panic!("About expected"),
        }
    }
    #[test]
    fn raw_keyboard_reads_all_license_pages_and_returns_without_document_mutation_en_fr() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let mut state = State::without_song(root.path()).unwrap();
            let before = state.workspace.file().session.serialize().unwrap();
            state.panel = Some(Panel::EditorHelp(8));
            press(&mut state, KeyCode::Enter);
            assert_eq!(view(&state), AboutView::Menu { selected: 0 });
            let version = deadsync_version::current().to_string();
            assert!(
                i18n::tr_fmt("Edition", "EditorAboutVersion", &[("version", &version)])
                    .contains(&version)
            );
            assert!(!i18n::tr("Edition", "EditorAboutAttribution").contains("MonoGame"));
            for kind in LicenseKind::ALL {
                assert_eq!(
                    view(&state),
                    AboutView::Menu {
                        selected: kind.index()
                    }
                );
                press(&mut state, KeyCode::Enter);
                let mut pages = Vec::new();
                for page in 0..=last_page(kind) {
                    assert_eq!(view(&state), AboutView::License { kind, page });
                    pages.push(license_text(kind, page));
                    let mut actors = Vec::new();
                    state.panel_actors_unscaled(state.panel.unwrap(), &mut actors);
                    assert!(!actors.is_empty());
                    press(&mut state, KeyCode::PageDown);
                }
                assert_eq!(
                    pages.join("\n"),
                    kind.text().lines().collect::<Vec<_>>().join("\n")
                );
                assert_eq!(
                    view(&state),
                    AboutView::License {
                        kind,
                        page: last_page(kind)
                    }
                );
                press(&mut state, KeyCode::Home);
                assert_eq!(view(&state), AboutView::License { kind, page: 0 });
                press(&mut state, KeyCode::PageUp);
                assert_eq!(view(&state), AboutView::License { kind, page: 0 });
                press(&mut state, KeyCode::End);
                assert_eq!(
                    view(&state),
                    AboutView::License {
                        kind,
                        page: last_page(kind)
                    }
                );
                press(&mut state, KeyCode::Escape);
                assert_eq!(
                    view(&state),
                    AboutView::Menu {
                        selected: kind.index()
                    }
                );
                press(&mut state, KeyCode::ArrowDown);
            }
            assert!(LICENSES[0].contains("GNU GENERAL PUBLIC LICENSE"));
            for text in &LICENSES[1..] {
                assert!(text.contains("Permission is hereby granted"));
            }
            press(&mut state, KeyCode::ArrowDown);
            assert_eq!(view(&state), AboutView::Menu { selected: 5 });
            press(&mut state, KeyCode::ArrowDown);
            assert_eq!(view(&state), AboutView::Menu { selected: 0 });
            press(&mut state, KeyCode::Escape);
            assert!(matches!(state.panel, Some(Panel::EditorHelp(8))));
            assert!(state.take_notification_url().is_none());
            assert!(state.take_editor_external_open().is_none());
            assert_eq!(state.workspace.file().session.serialize().unwrap(), before);
            assert!(!state.has_song());
        }
    }
    #[test]
    fn raw_update_action_requires_real_knowledge_and_explicit_enter() {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root = tempfile::tempdir().unwrap();
            let mut state = State::without_song(root.path()).unwrap();
            state.panel = Some(Panel::EditorAbout(AboutView::Menu { selected: 4 }));
            state.observe_editor_about_update(false, None);
            assert_eq!(
                state.editor_about_update_label(),
                i18n::tr("Edition", "EditorAboutUnknown")
            );
            press(&mut state, KeyCode::Enter);
            assert!(state.take_notification_url().is_none());
            state.observe_editor_about_update(true, None);
            assert_eq!(
                state.editor_about_update_label(),
                i18n::tr("Edition", "EditorAboutUpdated")
            );
            let url = "https://github.com/pnn64/deadsync/releases/tag/v9.0.0";
            state.observe_editor_about_update(false, Some(("v9.0.0".into(), url.into())));
            assert!(state.editor_about_update_label().contains("v9.0.0"));
            assert!(state.take_notification_url().is_none());
            press(&mut state, KeyCode::Enter);
            assert_eq!(state.take_notification_url().as_deref(), Some(url));
            state.observe_editor_about_update(false, Some(("bad".into(), "file:///tmp/a".into())));
            press(&mut state, KeyCode::Enter);
            assert!(state.take_notification_url().is_none());
        }
    }
}

#[cfg(test)]
mod license_layout_tests {
    use super::*;
    use deadlib_present::{actors::SizeSpec, space::{self, Metrics}};
    use deadsync_editor::options_preferences::{General, dpi::Capture};
    fn px(size: SizeSpec) -> f32 { match size {SizeSpec::Px(value)=>value,SizeSpec::Fill=>panic!("fixed modal")}}
    fn close(actual:f32,expected:f32) {assert!((actual-expected).abs()<0.03,"{actual} != {expected}");}
    #[test]
    fn all_license_pages_stay_centered_and_inside_real_viewport_en_fr_dpi_one_and_two() {
        for language in ["en","fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let root=tempfile::tempdir().unwrap();
            let mut state=State::without_song(root.path()).unwrap();
            for (width,height) in [(854.,480.),(1100.,760.),(3200.,1860.),(3440.,1440.)] { for dpi in [1.,2.] {
                let metrics=Metrics::centered(width/height*480.,480.);
                space::with_metrics(metrics,||{
                    state.set_ui_dpi_viewport(width as u32,height as u32);
                    state.ui_dpi=Some(Capture::new(&General::default(),dpi));
                    state.ui_dpi_render_scale=dpi as f32;
                    state.ui_dpi_supported=true;
                    let before=state.workspace.file().session.serialize().unwrap();
                    let lines=state.editor_license_page_lines();
                    assert!((1..=PAGE_LINES).contains(&lines));
                    for kind in LicenseKind::ALL {
                        let last=last_page_with_lines(kind,lines);
                        let mut contents=Vec::new();
                        for page in 0..=last {
                            let view=AboutView::License{kind,page};let panel=Panel::EditorAbout(view);
                            state.panel=Some(panel);let mut actors=Vec::new();state.panel_actors(panel,&mut actors);
                            let (transform,origin,children)=if let Actor::SharedTransform{transform,children,..}=&actors[0] {
                                let Actor::Frame{align,offset,size,children,..}=&children[0] else{panic!("UI frame")};
                                (*transform,[offset[0]-align[0]*px(size[0]),offset[1]-align[1]*px(size[1])],children.as_slice())
                            }else{(glam::Mat4::IDENTITY,[0.,0.],actors.as_slice())};
                            let Actor::Sprite{offset:modal,size,..}=&children[0] else{panic!("modal background")};
                            let center=transform.transform_point3(glam::Vec3::new(metrics.left+origin[0]+modal[0],metrics.top-origin[1]-modal[1],0.));
                            let center_px=[(center.x-metrics.left)/space::screen_width()*width,(metrics.top-center.y)/space::screen_height()*height];
                            close(center_px[0],width*0.5);close(center_px[1],height*0.5);
                            let physical=[px(size[0])*transform.x_axis.x*width/space::screen_width(),px(size[1])*transform.y_axis.y*height/space::screen_height()];
                            assert!(center_px[0]-physical[0]*0.5>=-0.03&&center_px[0]+physical[0]*0.5<=width+0.03,"width{width} dpi{dpi}");
                            assert!(center_px[1]-physical[1]*0.5>=-0.03&&center_px[1]+physical[1]*0.5<=height+0.03,"height{height} dpi{dpi}");
                            let Actor::Text{content:text,offset,scale,max_height,..}=&children[2] else{panic!("license body")};
                            contents.push(text.to_string());
                            assert!(offset[1]>=modal[1]-px(size[1])*0.5);
                            assert!(offset[1]+max_height.unwrap()<=modal[1]+px(size[1])*0.5);
                            assert!(text.lines().count() as f32*24.*scale[1]<=max_height.unwrap()+0.03,"body pagination fits at {width}x{height} dpi{dpi}");
                            let Actor::Text{offset:hint,..}=&children[3] else{panic!("license navigation hint")};
                            assert!(hint[1]>=offset[1]+max_height.unwrap());
                            assert!(hint[1]<modal[1]+px(size[1])*0.5);
                        }
                        assert_eq!(contents.join("\n"),kind.text().lines().collect::<Vec<_>>().join("\n"));
                        state.editor_about_key(AboutView::License{kind,page:0},KeyCode::End);
                        assert!(matches!(state.panel,Some(Panel::EditorAbout(AboutView::License{page,..}))if page==last));
                        state.editor_about_key(AboutView::License{kind,page:last},KeyCode::Home);
                        assert!(matches!(state.panel,Some(Panel::EditorAbout(AboutView::License{page:0,..}))));
                    }
                    assert_eq!(state.workspace.file().session.serialize().unwrap(),before);
                    assert!(state.workspace.file().session.undo_label().is_none());
                });
            }}
        }
    }
}
