//! Optional keyboard inspector and background overlay; chart sprites and UI remain above it.
use super::*;
use deadsync_editor::dark_preferences::{Order, Preferences, Size};
const KEYS: [&str; 10] = [
    "DarkShow",
    "DarkSize",
    "DarkOrder",
    "DarkRed",
    "DarkGreen",
    "DarkBlue",
    "DarkAlpha",
    "DarkSettingsDefaults",
    "DarkSettingsSave",
    "ScrollSettingsCancel",
];
impl State {
    pub fn dark_preferences(&self) -> Preferences {
        self.dark_preferences
    }
    pub fn configure_dark_preferences(&mut self, preferences: Preferences) {
        self.dark_preferences = preferences;
    }
    pub(super) fn open_dark_settings(&mut self) {
        self.minimap_release_input();
        self.dark_draft = Some(self.dark_preferences);
        self.panel = Some(Panel::DarkSettings(0));
        self.status = i18n::tr("Edition", "DarkHelp");
    }
    pub(super) fn sync_dark_preferences_history(&mut self) {
        if let Some(update) = self.workspace.take_dark_preferences_update() {
            self.dark_preferences = update.preferences;
            if let Some(file) = update.file {
                if file.path().as_os_str().is_empty() {
                    self.audio_preferences_path = None;
                    self.audio_preferences_file = None;
                    self.source_preferences_import.memory = Some(file);
                } else {
                    self.audio_preferences_path = Some(file.path().to_owned());
                    self.audio_preferences_file = Some(file);
                    self.source_preferences_import.memory = None;
                }
            }
        }
    }
    fn apply_dark_settings(&mut self, after: Preferences) -> Result<(), deadsync_editor::Error> {
        let file = self.capture_application_preferences_file()?;
        self.workspace
            .set_dark_preferences(self.dark_preferences, after, Some(&file))?;
        self.sync_dark_preferences_history();
        Ok(())
    }
    pub(super) fn toggle_dark(&mut self) {
        let before = self.dark_preferences;
        let result = (|| -> Result<(), deadsync_editor::Error> {
            let current = self.capture_application_preferences_file()?;
            let mut owner = Some(current.clone());
            let mut requested = before;
            requested.set_show(!before.show());
            let preferences = deadsync_editor::audio_preferences::publication::publish(
                &mut owner,
                Some(&current),
                requested,
                |current| {
                    let mut file = current.clone();
                    let mut preferences = file.dark();
                    if preferences.show() != before.show() {
                        return Err(deadsync_editor::Error::ExternalChange);
                    }
                    preferences.set_show(!before.show());
                    file.set_dark(preferences)?;
                    Ok((preferences, file))
                },
            )?;
            self.configure_dark_preferences(preferences);
            if let Some(file) = owner {
                self.publish_application_preferences_owner(file);
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.status = i18n::editor_error(&error).into();
        }
    }
    pub(super) fn dark_settings_key(&mut self, index: usize, code: KeyCode) {
        let enter = matches!(code, KeyCode::Enter | KeyCode::NumpadEnter);
        if code == KeyCode::Escape || (index == 9 && enter) {
            self.dark_draft = None;
            self.panel = None;
            self.status = i18n::tr("Edition", "DarkSettingsDiscarded");
            return;
        }
        if matches!(code, KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::Tab) {
            let back = code == KeyCode::ArrowUp || (code == KeyCode::Tab && self.shift);
            self.panel = Some(Panel::DarkSettings((index + if back { 9 } else { 1 }) % 10));
            return;
        }
        if index == 7 && enter {
            self.dark_draft = Some(Preferences::default());
            return;
        }
        if index == 8 && enter {
            if let Some(after) = self.dark_draft {
                match self.apply_dark_settings(after) {
                    Ok(()) => {
                        self.dark_draft = None;
                        self.panel = None;
                        self.status = i18n::tr("Edition", "DarkSettingsSaved");
                    }
                    Err(error) => {
                        self.status = i18n::tr_fmt(
                            "Edition",
                            "Error",
                            &[("message", &crate::i18n::editor_error(&error))],
                        )
                    }
                }
            }
            return;
        }
        if index >= 7
            || !(enter
                || matches!(
                    code,
                    KeyCode::ArrowLeft
                        | KeyCode::ArrowRight
                        | KeyCode::Home
                        | KeyCode::Digit0
                        | KeyCode::Numpad0
                ))
        {
            return;
        }
        let Some(draft) = self.dark_draft.as_mut() else {
            return;
        };
        let home = code == KeyCode::Home;
        let defaults = Preferences::default();
        match index {
            0 => draft.set_show(if home { defaults.show() } else { !draft.show() }),
            1 => draft.set_size(if home {
                defaults.size()
            } else {
                match draft.size() {
                    Size::Window => Size::Charts,
                    Size::Charts | Size::Unknown(_) => Size::Window,
                }
            }),
            2 => draft.set_order(if home {
                defaults.order()
            } else {
                match draft.order() {
                    Order::AfterBackground => Order::AfterWaveForm,
                    Order::AfterWaveForm | Order::Unknown(_) => Order::AfterBackground,
                }
            }),
            _ => {
                let component = index - 3;
                let mut color = draft.color();
                let delta = if self.shift { 0.001 } else { 0.01 }
                    * if code == KeyCode::ArrowLeft {
                        -1.0
                    } else {
                        1.0
                    };
                color[component] = if home {
                    defaults.color()[component]
                } else if matches!(code, KeyCode::Digit0 | KeyCode::Numpad0) {
                    0.0
                } else {
                    (color[component] + delta).clamp(0.0, 1.0)
                };
                if let Err(error) = draft.set_color(color) {
                    self.status = i18n::tr_fmt(
                        "Edition",
                        "Error",
                        &[("message", &crate::i18n::editor_error(&error))],
                    );
                }
            }
        }
    }
    pub(super) fn dark_settings_actors(&self, index: usize, actors: &mut Vec<Actor>) {
        let p = self.dark_draft.unwrap_or(self.dark_preferences);
        let x = screen_width() * 0.5;
        let top = (screen_height() * 0.5 - 150.0).max(12.0);
        actors.push(
            act!(quad:xy(x,top+140.0):setsize(560.0,310.0):diffuse(0.04,0.06,0.1,0.98):z(8050)),
        );
        for (row, key) in KEYS.iter().enumerate() {
            let ui_row_start = actors.len();
            let value = match row {
                0 => i18n::tr(
                    "Edition",
                    if p.show() {
                        "WaveformYes"
                    } else {
                        "WaveformNo"
                    },
                )
                .to_string(),
                1 => match p.size() {
                    Size::Charts => i18n::tr("Edition", "DarkCharts").to_string(),
                    Size::Window => i18n::tr("Edition", "DarkWindow").to_string(),
                    Size::Unknown(value) => i18n::tr_fmt(
                        "Edition",
                        "DarkUnknownSize",
                        &[("value", &value.to_string())],
                    )
                    .to_string(),
                },
                2 => match p.order() {
                    Order::AfterBackground => {
                        i18n::tr("Edition", "DarkAfterBackground").to_string()
                    }
                    Order::AfterWaveForm => i18n::tr("Edition", "DarkAfterWaveForm").to_string(),
                    Order::Unknown(value) => i18n::tr_fmt(
                        "Edition",
                        "DarkUnknownOrder",
                        &[("value", &value.to_string())],
                    )
                    .to_string(),
                },
                3..=6 => format!("{:.3}", p.color()[row - 3]),
                _ => String::new(),
            };
            let label = if row < 7 {
                i18n::tr_fmt("Edition", key, &[("value", &value)])
            } else {
                i18n::tr("Edition", key)
            };
            actors.push(act!(text:font("miso"):settext(label):xy(x,top+row as f32*29.0):zoom(0.65):maxwidth(520.0):diffuse(1.0,if row==index{0.8}else{1.0},if row==index{0.3}else{1.0},1.0):z(8051)));

            self.ui_row(actors, ui_row_start, row == index, 31.);
        }
    }
    pub(super) fn dark_background_actors(&self, actors: &mut Vec<Actor>, order: Order) {
        let p = self.dark_draft.unwrap_or(self.dark_preferences);
        if !p.show() || p.order() != order || matches!(order, Order::Unknown(_)) {
            return;
        }
        let width = screen_width();
        let height = screen_height();
        let (left, right) = match p.size() {
            Size::Window => (0.0, width),
            Size::Unknown(_) => return,
            Size::Charts => {
                let geometry = self.visible_view_geometry();
                if geometry.is_empty() {
                    return;
                }
                let left = geometry
                    .iter()
                    .map(|g| g.left - 42.0)
                    .fold(f32::INFINITY, f32::min)
                    .clamp(0.0, width);
                let right = geometry
                    .iter()
                    .map(|g| g.right() + 30.0)
                    .fold(f32::NEG_INFINITY, f32::max)
                    .clamp(0.0, width);
                (left, right)
            }
        };
        if left >= right || height <= 0.0 {
            return;
        }
        let [r, g, b, a] = p.rendered_color();
        // Background is 8000 and waveform is 8001. Stable insertion at
        // equal z places this overlay after waveforms and before chart guides;
        // note sprites and UI use higher z values.
        let z = if order == Order::AfterBackground {
            8000
        } else {
            8001
        };
        let mut actor = act!(quad:xy((left+right)*0.5,height*0.5):zoomto(right-left,height):diffuse(r,g,b,a):z(z));
        if let Actor::Sprite { blend, .. } = &mut actor {
            *blend = deadlib_render_core::BlendMode::PremultipliedAlpha;
        }
        actors.push(actor);
    }
}

#[cfg(test)]
mod source_consumer_tests {
    use super::*;
    use deadlib_present::{compose, space::Metrics};
    use deadsync_editor::workspace::Command;
    struct WhiteTextureContext;
    impl deadlib_present::texture::TextureContext for WhiteTextureContext {
        fn texture_registry_generation(&self) -> u64 {
            1
        }
        fn texture_dims(&self, key: &str) -> Option<deadlib_present::texture::TextureMeta> {
            (key == "__white").then_some(deadlib_present::texture::TextureMeta { w: 1, h: 1 })
        }
        fn sprite_sheet_dims(&self, _: &str) -> (u32, u32) {
            (1, 1)
        }
        fn texture_handle(&self, key: &str) -> deadlib_render_core::TextureHandle {
            if key == "__white" {
                deadlib_render::test_util::WHITE_TEXTURE_HANDLE
            } else {
                deadlib_render_core::INVALID_TEXTURE_HANDLE
            }
        }
    }
    #[test]
    fn real_dark_quad_rgb_with_alpha_zero_survives_compose_and_actual_software_blending() {
        i18n::init_for_tests();
        let dir = tempfile::tempdir().unwrap();
        let mut state = State::without_song(dir.path()).unwrap();
        let metrics = Metrics::centered(32., 32.);
        deadlib_present::space::set_current_metrics(metrics);
        for (alpha, source_alpha) in [(0., 0u32), (0.5, 127), (1., 255)] {
            let mut p = Preferences::default();
            p.set_show(true);
            p.set_color([0.251, 0.647, 0.419, alpha]).unwrap();
            state.configure_dark_preferences(p);
            let mut actors = Vec::new();
            state.dark_background_actors(&mut actors, Order::AfterBackground);
            assert_eq!(actors.len(), 1);
            assert!(matches!(
                actors[0],
                Actor::Sprite {
                    blend: deadlib_render_core::BlendMode::PremultipliedAlpha,
                    ..
                }
            ));
            let clear = [26. / 255., 51. / 255., 77. / 255., 1.];
            let frame = compose::build_screen_with_texture_context(
                &actors,
                clear,
                &metrics,
                &Default::default(),
                0.,
                &WhiteTextureContext,
            );
            assert!(!frame.ops.is_empty());
            let pixels =
                deadlib_render::test_util::render_solid_frame(&frame, metrics.projection(), 32, 32);
            let expected = [64u32, 164, 106]
                .into_iter()
                .zip([26u32, 51, 77])
                .map(|(s, d)| ((s * 255 + d * (255 - source_alpha) + 127) / 255).min(255) as u8)
                .collect::<Vec<_>>();
            assert_eq!(&pixels[16 * 32 + 16][..3], expected.as_slice());
            assert_eq!(pixels[16 * 32 + 16][3], 255);
            assert_ne!(&pixels[16 * 32 + 16][..3], &[26, 51, 77]);
        }
    }
    #[test]
    fn source_unknown_dark_enums_have_no_draw_and_keyboard_restore_cancel_history_preserve_raw_en_fr()
     {
        for language in ["en", "fr"] {
            i18n::init(deadsync_assets::language::load_for_tests(language));
            i18n::set_locale(deadsync_assets::language::load_for_tests(language));
            let dir = tempfile::tempdir().unwrap();
            let mut state = State::without_song(dir.path()).unwrap();
            state.panel = None;
            let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../deadsync-editor/tests/fixtures/dark-source.json"
            )))
            .unwrap();
            let mut session = deadsync_editor::history::Session::new(
                deadsync_editor::document::Document::parse(
                    b"#TITLE:Dark;".as_slice(),
                    deadsync_editor::document::Format::Sm,
                )
                .unwrap(),
            );
            for case in cases {
                let current = state.capture_application_preferences_file().unwrap();
                let draft = deadsync_editor::audio_preferences::source_import::Draft::prepare(
                    &mut session,
                    None,
                    &current,
                    case["raw"].as_str().unwrap().as_bytes(),
                    deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
                )
                .unwrap();
                state.configure_dark_preferences(draft.target().dark());
                for order in [Order::AfterBackground, Order::AfterWaveForm] {
                    let mut actors = Vec::new();
                    state.dark_background_actors(&mut actors, order);
                    let expected = &case["draw"][0];
                    let key = if order == Order::AfterBackground {
                        "after_background"
                    } else {
                        "after_waveform"
                    };
                    assert_eq!(
                        !actors.is_empty(),
                        expected[key].as_bool().unwrap(),
                        "{}",
                        case["name"]
                    );
                }
            }
            let mut p = Preferences::default();
            p.set_show(true);
            p.set_size(Size::Unknown(99));
            p.set_order(Order::Unknown(-99));
            p.set_color([0.251, 0.647, 0.419, 0.]).unwrap();
            state.configure_dark_preferences(p);
            state.open_dark_settings();
            let mut actors = Vec::new();
            state.dark_settings_actors(1, &mut actors);
            for key in ["DarkSettingsDefaults", "DarkSettingsSave"] {
                let expected = i18n::tr("Edition", key);
                assert_ne!(expected.as_ref(), key, "localized dark action must exist");
                assert!(actors.iter().any(|actor| matches!(actor,
                    Actor::Text { content, .. } if content.as_ref() == expected.as_ref())));
            }
            assert!(actors.iter().any(
                |actor| matches!(actor,Actor::Text{content,..}if content.as_ref().contains("99"))
            ));
            state.dark_settings_key(7, KeyCode::Enter);
            state.dark_settings_key(9, KeyCode::Enter);
            assert_eq!(state.status, i18n::tr("Edition", "DarkSettingsDiscarded"));
            assert_eq!(state.dark_preferences(), p);
            state.open_dark_settings();
            state.dark_settings_key(7, KeyCode::Enter);
            state.dark_settings_key(8, KeyCode::Enter);
            assert_eq!(state.status, i18n::tr("Edition", "DarkSettingsSaved"));
            assert_eq!(state.dark_preferences(), Preferences::default());
            state.command(Command::Undo);
            assert_eq!(state.dark_preferences(), p);
            state.command(Command::Redo);
            assert_eq!(state.dark_preferences(), Preferences::default());
            assert!(state.audio_preferences_file.is_none());
            assert!(state.audio_preferences_path.is_none());
            assert!(state.source_preferences_import.memory.is_some());
        }
    }
}

#[cfg(test)]
mod direct_toggle_tests {
    use super::*;
    fn state(language: &str, disk: bool, show: bool) -> (tempfile::TempDir, State) {
        i18n::init(deadsync_assets::language::load_for_tests(language));
        let root = tempfile::tempdir().unwrap();
        let mut state = State::without_song(root.path()).unwrap();
        if disk {
            let path = root.path().join("prefs.json");
            std::fs::write(
                &path,
                br#"{"Version":1,"Dark":{"Future":7},"Vendor":{"keep":true}}"#,
            )
            .unwrap();
            state.configure_audio_preferences(&path).unwrap();
        }
        let file = state.capture_application_preferences_file().unwrap();
        let input=serde_json::json!({"PreferencesDark":{"ShowDarkBg":show},"PreferencesKeyBinds":{"ToggleDark":[["F24"]]},"VendorSource":{"keep":[1,9]}}).to_string();
        let draft = state
            .workspace
            .prepare_source_preferences(
                &file,
                input.as_bytes(),
                deadsync_editor::key_binding_preferences::source_profile::Platform::Mac,
            )
            .unwrap();
        state
            .workspace
            .import_source_preferences(draft, &file, true)
            .unwrap();
        state.sync_source_preferences_history().unwrap();
        (root, state)
    }
    fn press(state: &mut State) {
        for pressed in [true, false] {
            state.raw_key(&RawKeyboardEvent {
                code: KeyCode::F24,
                pressed,
                repeat: false,
                timestamp: std::time::Instant::now(),
                host_nanos: 0,
            });
        }
    }
    #[test]
    fn source_direct_toggle_keeps_previous_disjoint_color_undo_and_redo() {
        for language in ["en", "fr"] {
            for disk in [false, true] {
                let (_root, mut state) = state(language, disk, true);
                let before = state.dark_preferences;
                let song = state.workspace.file().session.serialize().unwrap();
                state.open_dark_settings();
                let mut changed = state.dark_preferences;
                changed.set_color([0.2, 0.3, 0.4, 0.5]).unwrap();
                state.dark_draft = Some(changed);
                state.dark_settings_key(8, KeyCode::Enter);
                assert_eq!(state.dark_preferences, changed);
                assert!(state.dark_draft.is_none());
                let revision = state.workspace.file().session.revision();
                press(&mut state);
                assert!(!state.dark_preferences.show(), "{}", state.status);
                assert_eq!(state.dark_preferences.color(), changed.color());
                assert_eq!(state.workspace.file().session.revision(), revision);
                assert!(
                    !state
                        .capture_application_preferences_file()
                        .unwrap()
                        .has_changes()
                );
                state.command(Command::Undo);
                assert_eq!(state.dark_preferences.color(), before.color());
                assert!(
                    !state.dark_preferences.show(),
                    "Undo Color must preserve direct Show change"
                );
                state.command(Command::Redo);
                assert_eq!(state.dark_preferences.color(), changed.color());
                assert!(!state.dark_preferences.show());
                let file = state.capture_application_preferences_file().unwrap();
                assert!(!file.has_changes());
                assert!(file.source_archive().is_some());
                if disk {
                    let reopened =
                        deadsync_editor::audio_preferences::File::open(file.path()).unwrap();
                    assert_eq!(reopened.dark(), state.dark_preferences);
                    let out: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(file.path()).unwrap()).unwrap();
                    assert_eq!(out["Dark"]["Future"], 7);
                    assert_eq!(out["Vendor"]["keep"], true);
                }
                // Panel Restore still creates its own undoable transaction.
                let before_restore = state.dark_preferences;
                state.open_dark_settings();
                state.dark_settings_key(7, KeyCode::Enter);
                state.dark_settings_key(8, KeyCode::Enter);
                assert_eq!(state.dark_preferences, Preferences::default());
                state.command(Command::Undo);
                assert_eq!(state.dark_preferences, before_restore);
                state.command(Command::Redo);
                assert_eq!(state.dark_preferences, Preferences::default());
                assert_eq!(state.workspace.file().session.serialize().unwrap(), song);
            }
        }
    }
    #[test]
    fn original_dark_toggle_transcript_matches_direct_shortcut_without_history() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/dark-toggle-source.json"
        )))
        .unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 8);
        for disk in [false, true] {
            for stages in cases.chunks_exact(4) {
                let (_root, mut state) = state("en", disk, stages[0]["show"].as_bool().unwrap());
                let revision = state.workspace.file().session.revision();
                for (stage, case) in stages.iter().take(3).enumerate() {
                    if stage > 0 {
                        press(&mut state);
                    }
                    assert_eq!(
                        state.dark_preferences.show(),
                        case["show"].as_bool().unwrap()
                    );
                    assert_eq!(state.workspace.file().session.revision(), revision);
                }
                let file = state.capture_application_preferences_file().unwrap();
                assert!(!file.has_changes());
                assert_eq!(file.dark().show(), stages[3]["show"].as_bool().unwrap());
                if disk {
                    assert_eq!(
                        deadsync_editor::audio_preferences::File::open(file.path())
                            .unwrap()
                            .dark()
                            .show(),
                        stages[3]["show"].as_bool().unwrap()
                    );
                }
            }
        }
    }
    #[test]
    fn direct_toggle_competing_leaf_is_atomic_and_external_disjoint_fields_survive() {
        let (_root, mut state) = state("en", true, false);
        let path = state.audio_preferences_path.clone().unwrap();
        let mut external = deadsync_editor::audio_preferences::File::open(&path).unwrap();
        let mut p = external.dark();
        p.set_color([0.7, 0.6, 0.5, 0.4]).unwrap();
        external.set_dark(p).unwrap();
        external.save().unwrap();
        let live_color = state.dark_preferences.color();
        state.toggle_dark();
        assert!(state.dark_preferences.show());
        assert_eq!(state.dark_preferences.color(), live_color);
        let file = deadsync_editor::audio_preferences::File::open(&path).unwrap();
        assert_eq!(file.dark().color(), p.color());
        assert!(file.dark().show());
        let mut external = file;
        let mut p = external.dark();
        p.set_show(false);
        external.set_dark(p).unwrap();
        external.save().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let before = state.dark_preferences;
        let revision = state.workspace.file().session.revision();
        state.toggle_dark();
        assert_eq!(state.dark_preferences, before);
        assert_eq!(state.workspace.file().session.revision(), revision);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
