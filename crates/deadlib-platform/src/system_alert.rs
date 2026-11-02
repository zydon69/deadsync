//! Host system notification, independent of the gameplay/editor audio engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemAlert {
    Exclamation,
}
/// GrooveAuthor's platform contract: user-preferred Mac alert, Windows
/// Exclamation, and intentionally no sound on Linux/other platforms.
pub fn play(alert: SystemAlert) {
    match alert {
        SystemAlert::Exclamation => exclamation(),
    }
}
#[cfg(target_os = "macos")]
fn exclamation() {
    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn AudioServicesPlaySystemSound(sound: u32);
    }
    // kSystemSoundID_UserPreferredAlert, identical to EditorMacOsInterface.
    unsafe {
        AudioServicesPlaySystemSound(0x1000);
    }
}
#[cfg(target_os = "windows")]
fn exclamation() {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBeep(kind: u32) -> i32;
    }
    // MB_ICONEXCLAMATION, the SystemSounds.Exclamation sound alias.
    unsafe {
        MessageBeep(0x30);
    }
}
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn exclamation() {}
