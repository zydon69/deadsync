//! Explicit geometry for fixtures originally authored at one pixel per chart row.
use deadsync_theme_simply_love::screens::edition::State;
#[allow(dead_code)]
pub fn configure(state: &mut State, directory: &std::path::Path) {
    let path = directory.join("geometry-prefs.json");
    std::fs::write(
        &path,
        br#"{"Version":1,"Scroll":{"SpacingMode":"ConstantRow","RowBasedPixelsPerRow":1}}"#,
    )
    .unwrap();
    state.configure_audio_preferences(&path).unwrap();
}

#[allow(dead_code)]
pub fn configure_centered(state: &mut State, directory: &std::path::Path) {
    let path = directory.join("centered-geometry-prefs.json");
    std::fs::write(&path, br#"{"Version":1,"Scroll":{"SpacingMode":"ConstantRow","RowBasedPixelsPerRow":1},"Receptors":{"CenterHorizontally":true}}"#).unwrap();
    state.configure_audio_preferences(&path).unwrap();
}
