use deadlib_present::actors::Actor;
use deadsync_theme_simply_love::{i18n, screens::edition::State};
#[test]
fn unsupported_imported_scale_is_preserved_and_reports_localized_safe_ui() {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    deadlib_present::space::set_current_metrics(deadlib_present::space::Metrics::centered(
        854., 480.,
    ));
    let root = tempfile::tempdir().unwrap();
    let prefs = root.path().join("prefs.json");
    std::fs::write(
        &prefs,
        r#"{"Version":1,"Options":{"UseCustomDpiScale":true,"DpiScale":1e308}}"#,
    )
    .unwrap();
    let original = std::fs::read(&prefs).unwrap();
    let mut state = State::without_song(root.path()).unwrap();
    state.configure_audio_preferences(&prefs).unwrap();
    assert!(state.bind_ui_dpi(1.).is_err());
    assert!(state.ui_dpi_is_bound());
    assert!(state.bind_ui_dpi(1.).is_err());
    assert_eq!(state.active_ui_dpi(), 1.);
    assert_eq!(state.general_options().dpi_scale, 1e308);
    let mut actors = Vec::new();
    state.actors(&mut actors);
    let message = i18n::tr("Edition", "UiDpiUnsupported");
    assert!(
        actors
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..}if content.as_str()==&*message))
    );
    assert_eq!(std::fs::read(&prefs).unwrap(), original);
}
