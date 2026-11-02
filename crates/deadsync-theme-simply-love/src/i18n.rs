mod editor_errors;
pub(crate) use editor_errors::{EditorDiagnostic, editor_error};

pub use crate::i18n_runtime::{
    LookupKey, current_locale, lookup_key, revision, tr, tr_fmt, tr_fmt_into,
};

use deadsync_assets::language::LanguageBundle;

/// Initialize render-time localization from a prepared asset bundle.
pub fn init(bundle: LanguageBundle) {
    crate::i18n_runtime::init(bundle);
}

/// Switch render-time localization to a prepared asset bundle.
pub fn set_locale(bundle: LanguageBundle) {
    crate::i18n_runtime::set_locale(bundle);
}

/// Test initializer that loads `en.ini` through the asset boundary.
/// Restore English on every call: editor tests deliberately switch languages,
/// so a process-wide Once would leak their last locale into later tests.
#[doc(hidden)]
pub fn init_for_tests() {
    init(deadsync_assets::language::load_for_tests("en"));
}

#[cfg(test)]
mod test_initialization {
    #[test]
    fn initializer_restores_english_after_editor_language_switch() {
        super::init_for_tests();
        let english = super::tr("Edition", "Save");
        super::set_locale(deadsync_assets::language::load_for_tests("fr"));
        assert_ne!(super::tr("Edition", "Save"), english);
        super::init_for_tests();
        assert_eq!(super::tr("Edition", "Save"), english);
    }
}

/// Translate fixed UI labels; callers must keep song and profile metadata separate.
pub(crate) fn tr_ui(source: &str) -> std::sync::Arc<str> {
    let key = match source {
        "Close" => "Close",
        "  LOCKED" => "LOCKED",
        "Refresh List" => "RefreshList",
        "Create Lobby" => "CreateLobby",
        "Leave Lobby" => "LeaveLobby",
        "Selected Song\n" => "SelectedSong",
        "&MENULEFT;/&MENURIGHT;: PICK    &START;: CHOOSE    &BACK;: CANCEL" => {
            "MENULEFTMENURIGHTPICKSTARTCHOOSEBACKCANCEL"
        }
        "START: SELECT ACTION    BACK/SELECT: CLOSE" => "STARTSELECTACTIONBACKSELECTCLOSE",
        "ARROWS: MOVE    START: SELECT    BACK/SELECT: CLOSE" => {
            "ARROWSMOVESTARTSELECTBACKSELECTCLOSE"
        }
        "Disconnected from online service." => "DisconnectedFromOnlineService",
        "Connecting to online service..." => "ConnectingToOnlineService",
        "Connected to online service." => "ConnectedToOnlineService",
        "Select a lobby or create one." => "SelectALobbyOrCreateOne",
        "Enter the lobby password." => "EnterTheLobbyPassword",
        "Create Lobby Password (Optional)" => "CreateLobbyPasswordOptional",
        "Use &MENULEFT;/&MENURIGHT; to pick characters, then press &START;." => {
            "UseMENULEFTMENURIGHTToPickCharactersThenPressSTART"
        }
        "Press &START; to dismiss." => "PressSTARTToDismiss",
        "Press F5 to retry failed downloads.  Press &START; to dismiss." => {
            "PressF5ToRetryFailedDownloadsPressSTARTToDismiss"
        }
        "No Downloads to view" => "NoDownloadsToView",
        "Player 1" => "Player1",
        "Player 2" => "Player2",
        "Waiting for raw input" => "WaitingForRawInput",
        "0 Hz latest / 0 Hz max" => "0HzLatest0HzMax",
        "Hold &SELECT; to return to Options." => "HoldSELECTToReturnToOptions",
        "Hold &BACK; to return to Options." => "HoldBACKToReturnToOptions",
        "Insert Coin" => "InsertCoin",
        "KEYBOARD/PAD MAPPINGS" => "KEYBOARDPADMAPPINGS",
        "CONFIGURE PADS  -  ADVANCED" => "CONFIGUREPADSADVANCED",
        "CONFIGURE PADS" => "CONFIGUREPADS",
        _ => return std::sync::Arc::from(source),
    };
    tr("UiMessages", key)
}
