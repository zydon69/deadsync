//! Executable native subset, separate from the source callback inventory.
#[path = "key_binding_preferences/history.rs"]
pub(crate) mod history;
pub use history::Update;
#[path = "key_binding_preferences/inspector.rs"]
pub mod inspector;
#[path = "key_binding_preferences/physical.rs"]
pub mod physical;
#[path = "key_binding_preferences/reservations.rs"]
pub mod reservations;
#[path = "key_binding_preferences/source_profile.rs"]
pub mod source_profile;
use crate::key_commands::{Binding, Key};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    AddEventTempo,
    AddEventInterpolatedScrollRate,
    AddEventScrollRate,
    AddEventStop,
    AddEventDelay,
    AddEventWarp,
    AddEventFakeRegion,
    AddEventTicks,
    AddEventComboMultipliers,
    AddEventTimeSignature,
    AddEventLabel,
    AddEventAttack,
    AddEventPattern,
    Mirror,
    Flip,
    MirrorAndFlip,
    ShiftLeft,
    ShiftRight,
    ShiftLeftAndWrap,
    ShiftRightAndWrap,
    ConvertSelectedNotesToPlayer1,
    ConvertSelectedNotesToPlayer2,
    ConvertSelectedNotesToPlayer3,
    ConvertSelectedNotesToPlayer4,
    SwapSelectedPlayer1And2Notes,
    SwapSelectedPlayer1And3Notes,
    SwapSelectedPlayer1And4Notes,
    SwapSelectedPlayer2And3Notes,
    SwapSelectedPlayer2And4Notes,
    SwapSelectedPlayer3And4Notes,
    ConvertSelectedTapsToMines,
    ConvertSelectedTapsToFakes,
    ConvertSelectedTapsToLifts,
    ConvertSelectedMinesToTaps,
    ConvertSelectedMinesToFakes,
    ConvertSelectedMinesToLifts,
    ConvertSelectedFakesToTaps,
    ConvertSelectedLiftsToTaps,
    ConvertSelectedHoldsToRolls,
    ConvertSelectedHoldsToTaps,
    ConvertSelectedHoldsToMines,
    ConvertSelectedRollsToHolds,
    ConvertSelectedRollsToTaps,
    ConvertSelectedRollsToMines,
    SwapSelectedTapsAndMines,
    SwapSelectedTapsAndFakes,
    SwapSelectedTapsAndLifts,
    SwapSelectedMinesAndFakes,
    SwapSelectedMinesAndLifts,
    SwapSelectedHoldsAndRolls,
    SwapSelectedHoldsAndTaps,
    SwapSelectedHoldsAndMines,
    SwapSelectedRollsAndTaps,
    SwapSelectedRollsAndMines,
    ShiftAllEarlier,
    ShiftAllLater,

    Save,
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    Delete,
    PreviousMeasure,
    NextMeasure,
    SelectRowRange,
    TogglePlayer,
    PlayPause,
    ChartStart,
    DecreaseSnap,
    IncreaseSnap,
    ChartEnd,
    PreviousLabel,
    NextLabel,
    PreviousChart,
    NextChart,
    PreviousTab,
    NextTab,
    ToggleWaveform,
    WaveformSettings,
    DecreaseRate,
    IncreaseRate,
    SaveAs,
    OpenMenu,
    OpenAnalysis,
    OpenSongProperties,
    OpenTiming,
    OpenPatternSet,
    OpenBatch,
    OpenChartProperties,
    OpenMarker,
    RegeneratePattern,
    OpenPattern,
    OpenGeneration,
    NewChart,
    DeleteChart,
    SaveRecovery,
    RecoverDraft,
    DuplicateChart,
    OpenChartList,
    OpenKeyBindings,
    OpenReceptors,
    OpenNoteColors,
    CopyEvents,
    CutEvents,
    PasteEvents,
    OpenEventSelection,
    OpenChartEventCopy,
    InspectAttack,
    PreviousAttack,
    NextAttack,
    OpenScrollSettings,
    OpenZoomSettings,
    ToggleReverse,
    ToggleSpacing,
    ToggleVariableSpacing,
    ZoomIn,
    ZoomOut,
    ResetZoom,
    ToggleAutoAdvance,
    NextTool,
    PreviousTool,
    MoveUp,
    MoveDown,
    CloseTab,
    KeepTab,
    MoveTabLeft,
    MoveTabRight,
    OpenNoteAttributes,
    OpenNoteConversion,
    OpenNoteOwners,
    OpenTransforms,
    FileNew,
    FileOpen,
    FileReload,
    FileRecent,
}
impl Action {
    pub const ALL: [Self; 136] = [
        Self::Save,
        Self::Undo,
        Self::Redo,
        Self::Copy,
        Self::Cut,
        Self::Paste,
        Self::Delete,
        Self::PreviousMeasure,
        Self::NextMeasure,
        Self::SelectRowRange,
        Self::TogglePlayer,
        Self::PlayPause,
        Self::ChartStart,
        Self::DecreaseSnap,
        Self::IncreaseSnap,
        Self::ChartEnd,
        Self::PreviousLabel,
        Self::NextLabel,
        Self::PreviousChart,
        Self::NextChart,
        Self::PreviousTab,
        Self::NextTab,
        Self::ToggleWaveform,
        Self::WaveformSettings,
        Self::DecreaseRate,
        Self::IncreaseRate,
        Self::SaveAs,
        Self::OpenMenu,
        Self::OpenAnalysis,
        Self::OpenSongProperties,
        Self::OpenTiming,
        Self::OpenPatternSet,
        Self::OpenBatch,
        Self::OpenChartProperties,
        Self::OpenMarker,
        Self::RegeneratePattern,
        Self::OpenPattern,
        Self::OpenGeneration,
        Self::NewChart,
        Self::DeleteChart,
        Self::SaveRecovery,
        Self::RecoverDraft,
        Self::DuplicateChart,
        Self::OpenChartList,
        Self::OpenKeyBindings,
        Self::OpenReceptors,
        Self::OpenNoteColors,
        Self::CopyEvents,
        Self::CutEvents,
        Self::PasteEvents,
        Self::OpenEventSelection,
        Self::OpenChartEventCopy,
        Self::InspectAttack,
        Self::PreviousAttack,
        Self::NextAttack,
        Self::OpenScrollSettings,
        Self::OpenZoomSettings,
        Self::ToggleReverse,
        Self::ToggleSpacing,
        Self::ToggleVariableSpacing,
        Self::ZoomIn,
        Self::ZoomOut,
        Self::ResetZoom,
        Self::ToggleAutoAdvance,
        Self::NextTool,
        Self::PreviousTool,
        Self::MoveUp,
        Self::MoveDown,
        Self::CloseTab,
        Self::KeepTab,
        Self::MoveTabLeft,
        Self::MoveTabRight,
        Self::OpenNoteAttributes,
        Self::OpenNoteConversion,
        Self::OpenNoteOwners,
        Self::OpenTransforms,
        Self::FileNew,
        Self::FileOpen,
        Self::FileReload,
        Self::FileRecent,
        Self::AddEventTempo,
        Self::AddEventInterpolatedScrollRate,
        Self::AddEventScrollRate,
        Self::AddEventStop,
        Self::AddEventDelay,
        Self::AddEventWarp,
        Self::AddEventFakeRegion,
        Self::AddEventTicks,
        Self::AddEventComboMultipliers,
        Self::AddEventTimeSignature,
        Self::AddEventLabel,
        Self::AddEventAttack,
        Self::AddEventPattern,
        Self::Mirror,
        Self::Flip,
        Self::MirrorAndFlip,
        Self::ShiftLeft,
        Self::ShiftRight,
        Self::ShiftLeftAndWrap,
        Self::ShiftRightAndWrap,
        Self::ConvertSelectedNotesToPlayer1,
        Self::ConvertSelectedNotesToPlayer2,
        Self::ConvertSelectedNotesToPlayer3,
        Self::ConvertSelectedNotesToPlayer4,
        Self::SwapSelectedPlayer1And2Notes,
        Self::SwapSelectedPlayer1And3Notes,
        Self::SwapSelectedPlayer1And4Notes,
        Self::SwapSelectedPlayer2And3Notes,
        Self::SwapSelectedPlayer2And4Notes,
        Self::SwapSelectedPlayer3And4Notes,
        Self::ConvertSelectedTapsToMines,
        Self::ConvertSelectedTapsToFakes,
        Self::ConvertSelectedTapsToLifts,
        Self::ConvertSelectedMinesToTaps,
        Self::ConvertSelectedMinesToFakes,
        Self::ConvertSelectedMinesToLifts,
        Self::ConvertSelectedFakesToTaps,
        Self::ConvertSelectedLiftsToTaps,
        Self::ConvertSelectedHoldsToRolls,
        Self::ConvertSelectedHoldsToTaps,
        Self::ConvertSelectedHoldsToMines,
        Self::ConvertSelectedRollsToHolds,
        Self::ConvertSelectedRollsToTaps,
        Self::ConvertSelectedRollsToMines,
        Self::SwapSelectedTapsAndMines,
        Self::SwapSelectedTapsAndFakes,
        Self::SwapSelectedTapsAndLifts,
        Self::SwapSelectedMinesAndFakes,
        Self::SwapSelectedMinesAndLifts,
        Self::SwapSelectedHoldsAndRolls,
        Self::SwapSelectedHoldsAndTaps,
        Self::SwapSelectedHoldsAndMines,
        Self::SwapSelectedRollsAndTaps,
        Self::SwapSelectedRollsAndMines,
        Self::ShiftAllEarlier,
        Self::ShiftAllLater,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Self::AddEventTempo => "AddEventTempo",
            Self::AddEventInterpolatedScrollRate => "AddEventInterpolatedScrollRate",
            Self::AddEventScrollRate => "AddEventScrollRate",
            Self::AddEventStop => "AddEventStop",
            Self::AddEventDelay => "AddEventDelay",
            Self::AddEventWarp => "AddEventWarp",
            Self::AddEventFakeRegion => "AddEventFakeRegion",
            Self::AddEventTicks => "AddEventTicks",
            Self::AddEventComboMultipliers => "AddEventComboMultipliers",
            Self::AddEventTimeSignature => "AddEventTimeSignature",
            Self::AddEventLabel => "AddEventLabel",
            Self::AddEventAttack => "AddEventAttack",
            Self::AddEventPattern => "AddEventPattern",
            Self::Mirror => "Mirror",
            Self::Flip => "Flip",
            Self::MirrorAndFlip => "MirrorAndFlip",
            Self::ShiftLeft => "ShiftLeft",
            Self::ShiftRight => "ShiftRight",
            Self::ShiftLeftAndWrap => "ShiftLeftAndWrap",
            Self::ShiftRightAndWrap => "ShiftRightAndWrap",
            Self::ConvertSelectedNotesToPlayer1 => "ConvertSelectedNotesToPlayer1",
            Self::ConvertSelectedNotesToPlayer2 => "ConvertSelectedNotesToPlayer2",
            Self::ConvertSelectedNotesToPlayer3 => "ConvertSelectedNotesToPlayer3",
            Self::ConvertSelectedNotesToPlayer4 => "ConvertSelectedNotesToPlayer4",
            Self::SwapSelectedPlayer1And2Notes => "SwapSelectedPlayer1And2Notes",
            Self::SwapSelectedPlayer1And3Notes => "SwapSelectedPlayer1And3Notes",
            Self::SwapSelectedPlayer1And4Notes => "SwapSelectedPlayer1And4Notes",
            Self::SwapSelectedPlayer2And3Notes => "SwapSelectedPlayer2And3Notes",
            Self::SwapSelectedPlayer2And4Notes => "SwapSelectedPlayer2And4Notes",
            Self::SwapSelectedPlayer3And4Notes => "SwapSelectedPlayer3And4Notes",
            Self::ConvertSelectedTapsToMines => "ConvertSelectedTapsToMines",
            Self::ConvertSelectedTapsToFakes => "ConvertSelectedTapsToFakes",
            Self::ConvertSelectedTapsToLifts => "ConvertSelectedTapsToLifts",
            Self::ConvertSelectedMinesToTaps => "ConvertSelectedMinesToTaps",
            Self::ConvertSelectedMinesToFakes => "ConvertSelectedMinesToFakes",
            Self::ConvertSelectedMinesToLifts => "ConvertSelectedMinesToLifts",
            Self::ConvertSelectedFakesToTaps => "ConvertSelectedFakesToTaps",
            Self::ConvertSelectedLiftsToTaps => "ConvertSelectedLiftsToTaps",
            Self::ConvertSelectedHoldsToRolls => "ConvertSelectedHoldsToRolls",
            Self::ConvertSelectedHoldsToTaps => "ConvertSelectedHoldsToTaps",
            Self::ConvertSelectedHoldsToMines => "ConvertSelectedHoldsToMines",
            Self::ConvertSelectedRollsToHolds => "ConvertSelectedRollsToHolds",
            Self::ConvertSelectedRollsToTaps => "ConvertSelectedRollsToTaps",
            Self::ConvertSelectedRollsToMines => "ConvertSelectedRollsToMines",
            Self::SwapSelectedTapsAndMines => "SwapSelectedTapsAndMines",
            Self::SwapSelectedTapsAndFakes => "SwapSelectedTapsAndFakes",
            Self::SwapSelectedTapsAndLifts => "SwapSelectedTapsAndLifts",
            Self::SwapSelectedMinesAndFakes => "SwapSelectedMinesAndFakes",
            Self::SwapSelectedMinesAndLifts => "SwapSelectedMinesAndLifts",
            Self::SwapSelectedHoldsAndRolls => "SwapSelectedHoldsAndRolls",
            Self::SwapSelectedHoldsAndTaps => "SwapSelectedHoldsAndTaps",
            Self::SwapSelectedHoldsAndMines => "SwapSelectedHoldsAndMines",
            Self::SwapSelectedRollsAndTaps => "SwapSelectedRollsAndTaps",
            Self::SwapSelectedRollsAndMines => "SwapSelectedRollsAndMines",
            Self::ShiftAllEarlier => "ShiftAllEarlier",
            Self::ShiftAllLater => "ShiftAllLater",

            Self::Save => "Save",
            Self::Undo => "Undo",
            Self::Redo => "Redo",
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",
            Self::Delete => "Delete",
            Self::PreviousMeasure => "MoveToPreviousMeasure",
            Self::NextMeasure => "MoveToNextMeasure",
            Self::SelectRowRange => "SelectRowRange",
            Self::TogglePlayer => "TogglePlayer",
            Self::PlayPause => "PlayPause",
            Self::ChartStart => "MoveToChartStart",
            Self::DecreaseSnap => "DecreaseSnap",
            Self::IncreaseSnap => "IncreaseSnap",
            Self::ChartEnd => "MoveToChartEnd",
            Self::PreviousLabel => "MoveToPreviousLabel",
            Self::NextLabel => "MoveToNextLabel",
            Self::PreviousChart => "OpenPreviousChart",
            Self::NextChart => "OpenNextChart",
            Self::PreviousTab => "FocusPreviousChart",
            Self::NextTab => "FocusNextChart",
            Self::ToggleWaveform => "ToggleWaveForm",
            Self::WaveformSettings => "OpenWaveformSettings",
            Self::DecreaseRate => "DecreaseMusicRate",
            Self::IncreaseRate => "IncreaseMusicRate",
            Self::SaveAs => "SaveAs",
            Self::OpenMenu => "OpenEditorMenu",
            Self::OpenAnalysis => "OpenChartAnalysis",
            Self::OpenSongProperties => "OpenSongProperties",
            Self::OpenTiming => "OpenTimingEvents",
            Self::OpenPatternSet => "OpenPatternSet",
            Self::OpenBatch => "OpenChartBatch",
            Self::OpenChartProperties => "OpenChartProperties",
            Self::OpenMarker => "OpenPatternMarker",
            Self::RegeneratePattern => "RegenerateSavedPattern",
            Self::OpenPattern => "OpenPatternGeneration",
            Self::OpenGeneration => "OpenGenerationSettings",
            Self::NewChart => "CreateEmptyChart",
            Self::DeleteChart => "DeleteCurrentChart",
            Self::SaveRecovery => "SaveRecoveryDraft",
            Self::RecoverDraft => "RecoverDraft",
            Self::DuplicateChart => "DuplicateCurrentChart",
            Self::OpenChartList => "OpenChartList",
            Self::OpenKeyBindings => "OpenKeyBindingSettings",
            Self::OpenReceptors => "OpenReceptorSettings",
            Self::OpenNoteColors => "OpenNoteColorSettings",
            Self::CopyEvents => "CopySelectedEvents",
            Self::CutEvents => "CutSelectedEvents",
            Self::PasteEvents => "PasteSelectedEvents",
            Self::OpenEventSelection => "OpenEventSelection",
            Self::OpenChartEventCopy => "OpenChartEventCopy",
            Self::InspectAttack => "InspectAttackAtCursor",
            Self::PreviousAttack => "MoveToPreviousAttack",
            Self::NextAttack => "MoveToNextAttack",
            Self::OpenScrollSettings => "OpenScrollSettings",
            Self::OpenZoomSettings => "OpenZoomSettings",
            Self::ToggleReverse => "ToggleReverseDisplay",
            Self::ToggleSpacing => "ToggleTimeRowDisplay",
            Self::ToggleVariableSpacing => "ToggleVariableDisplay",
            Self::ZoomIn => "IncreaseKeyboardZoom",
            Self::ZoomOut => "DecreaseKeyboardZoom",
            Self::ResetZoom => "ResetKeyboardZoom",
            Self::ToggleAutoAdvance => "ToggleAutoAdvance",
            Self::NextTool => "NextNoteTool",
            Self::PreviousTool => "PreviousNoteTool",
            Self::MoveUp => "MoveUp",
            Self::MoveDown => "MoveDown",
            Self::CloseTab => "CloseFocusedChart",
            Self::KeepTab => "KeepChartOpen",
            Self::MoveTabLeft => "MoveFocusedChartLeft",
            Self::MoveTabRight => "MoveFocusedChartRight",
            Self::OpenNoteAttributes => "OpenNoteAttributes",
            Self::OpenNoteConversion => "OpenNoteConversion",
            Self::OpenNoteOwners => "OpenNoteOwners",
            Self::OpenTransforms => "OpenTransforms",
            Self::FileNew => "New",
            Self::FileOpen => "Open",
            Self::FileReload => "Reload",
            Self::FileRecent => "OpenRecentSongs",
        }
    }
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
    /// Source selected-note callbacks usable from the checked-selection panel.
    pub fn is_selected_review(self) -> bool {
        matches!(
            self,
            Self::Mirror
                | Self::Flip
                | Self::MirrorAndFlip
                | Self::ShiftLeft
                | Self::ShiftRight
                | Self::ShiftLeftAndWrap
                | Self::ShiftRightAndWrap
                | Self::ConvertSelectedNotesToPlayer1
                | Self::ConvertSelectedNotesToPlayer2
                | Self::ConvertSelectedNotesToPlayer3
                | Self::ConvertSelectedNotesToPlayer4
                | Self::SwapSelectedPlayer1And2Notes
                | Self::SwapSelectedPlayer1And3Notes
                | Self::SwapSelectedPlayer1And4Notes
                | Self::SwapSelectedPlayer2And3Notes
                | Self::SwapSelectedPlayer2And4Notes
                | Self::SwapSelectedPlayer3And4Notes
                | Self::ConvertSelectedTapsToMines
                | Self::ConvertSelectedTapsToFakes
                | Self::ConvertSelectedTapsToLifts
                | Self::ConvertSelectedMinesToTaps
                | Self::ConvertSelectedMinesToFakes
                | Self::ConvertSelectedMinesToLifts
                | Self::ConvertSelectedFakesToTaps
                | Self::ConvertSelectedLiftsToTaps
                | Self::ConvertSelectedHoldsToRolls
                | Self::ConvertSelectedHoldsToTaps
                | Self::ConvertSelectedHoldsToMines
                | Self::ConvertSelectedRollsToHolds
                | Self::ConvertSelectedRollsToTaps
                | Self::ConvertSelectedRollsToMines
                | Self::SwapSelectedTapsAndMines
                | Self::SwapSelectedTapsAndFakes
                | Self::SwapSelectedTapsAndLifts
                | Self::SwapSelectedMinesAndFakes
                | Self::SwapSelectedMinesAndLifts
                | Self::SwapSelectedHoldsAndRolls
                | Self::SwapSelectedHoldsAndTaps
                | Self::SwapSelectedHoldsAndMines
                | Self::SwapSelectedRollsAndTaps
                | Self::SwapSelectedRollsAndMines
        )
    }
    pub fn repeats(self) -> bool {
        matches!(
            self,
            Self::ShiftLeft
                | Self::ShiftRight
                | Self::ShiftLeftAndWrap
                | Self::ShiftRightAndWrap
                | Self::PreviousMeasure
                | Self::NextMeasure
                | Self::DecreaseSnap
                | Self::IncreaseSnap
                | Self::PreviousLabel
                | Self::NextLabel
                | Self::PreviousTab
                | Self::NextTab
                | Self::DecreaseRate
                | Self::IncreaseRate
                | Self::ZoomIn
                | Self::ZoomOut
                | Self::MoveUp
                | Self::MoveDown
                | Self::MoveTabLeft
                | Self::MoveTabRight
        )
    }
    fn native(self) -> Vec<Vec<&'static str>> {
        match self {
            Self::AddEventTempo
            | Self::AddEventInterpolatedScrollRate
            | Self::AddEventScrollRate
            | Self::AddEventStop
            | Self::AddEventDelay
            | Self::AddEventWarp
            | Self::AddEventFakeRegion
            | Self::AddEventTicks
            | Self::AddEventComboMultipliers
            | Self::AddEventTimeSignature
            | Self::AddEventLabel
            | Self::AddEventAttack
            | Self::AddEventPattern
            | Self::Mirror
            | Self::Flip
            | Self::MirrorAndFlip
            | Self::ShiftLeft
            | Self::ShiftRight
            | Self::ShiftLeftAndWrap
            | Self::ShiftRightAndWrap
            | Self::ConvertSelectedNotesToPlayer1
            | Self::ConvertSelectedNotesToPlayer2
            | Self::ConvertSelectedNotesToPlayer3
            | Self::ConvertSelectedNotesToPlayer4
            | Self::SwapSelectedPlayer1And2Notes
            | Self::SwapSelectedPlayer1And3Notes
            | Self::SwapSelectedPlayer1And4Notes
            | Self::SwapSelectedPlayer2And3Notes
            | Self::SwapSelectedPlayer2And4Notes
            | Self::SwapSelectedPlayer3And4Notes
            | Self::ConvertSelectedTapsToMines
            | Self::ConvertSelectedTapsToFakes
            | Self::ConvertSelectedTapsToLifts
            | Self::ConvertSelectedMinesToTaps
            | Self::ConvertSelectedMinesToFakes
            | Self::ConvertSelectedMinesToLifts
            | Self::ConvertSelectedFakesToTaps
            | Self::ConvertSelectedLiftsToTaps
            | Self::ConvertSelectedHoldsToRolls
            | Self::ConvertSelectedHoldsToTaps
            | Self::ConvertSelectedHoldsToMines
            | Self::ConvertSelectedRollsToHolds
            | Self::ConvertSelectedRollsToTaps
            | Self::ConvertSelectedRollsToMines
            | Self::SwapSelectedTapsAndMines
            | Self::SwapSelectedTapsAndFakes
            | Self::SwapSelectedTapsAndLifts
            | Self::SwapSelectedMinesAndFakes
            | Self::SwapSelectedMinesAndLifts
            | Self::SwapSelectedHoldsAndRolls
            | Self::SwapSelectedHoldsAndTaps
            | Self::SwapSelectedHoldsAndMines
            | Self::SwapSelectedRollsAndTaps
            | Self::SwapSelectedRollsAndMines
            | Self::ShiftAllEarlier
            | Self::ShiftAllLater => vec![vec![]],

            Self::Save => vec![vec!["Ctrl", "S"]],
            Self::Undo => vec![vec!["Ctrl", "Z"]],
            Self::Redo => vec![vec!["Ctrl", "Y"], vec!["Ctrl", "LeftShift", "Z"]],
            Self::Copy => vec![vec!["Ctrl", "C"]],
            Self::Cut => vec![vec!["Ctrl", "X"]],
            Self::Paste => vec![vec!["Ctrl", "V"]],
            Self::Delete => vec![vec!["Delete"], vec!["Back"]],
            Self::PreviousMeasure => vec![vec!["PageUp"]],
            Self::NextMeasure => vec![vec!["PageDown"]],
            Self::SelectRowRange => vec![vec!["Space"]],
            Self::TogglePlayer => vec![vec!["P"]],
            Self::PlayPause => vec![vec!["Enter"], vec!["NumPadEnter"]],
            Self::ChartStart => vec![vec!["Home"]],
            Self::DecreaseSnap => vec![vec!["Left"]],
            Self::IncreaseSnap => vec![vec!["Right"]],
            Self::ChartEnd => vec![vec!["End"]],
            Self::PreviousLabel => vec![vec!["Ctrl", "LeftShift", "L"]],
            Self::NextLabel => vec![vec!["Ctrl", "L"]],
            Self::PreviousChart => vec![vec!["Ctrl", "LeftShift", "Tab"]],
            Self::NextChart => vec![vec!["Ctrl", "Tab"]],
            Self::PreviousTab => vec![vec!["Ctrl", "PageUp"]],
            Self::NextTab => vec![vec!["Ctrl", "PageDown"]],
            Self::ToggleWaveform => vec![vec!["W"]],
            Self::WaveformSettings => vec![vec!["LeftShift", "W"]],
            Self::DecreaseRate => vec![vec!["LeftShift", "Left"]],
            Self::IncreaseRate => vec![vec!["LeftShift", "Right"]],
            Self::SaveAs => vec![vec!["Ctrl", "F12"]],
            Self::OpenMenu => vec![vec!["F1"]],
            Self::OpenAnalysis => vec![vec!["F12"]],
            Self::OpenSongProperties => vec![vec!["F11"]],
            Self::OpenTiming => vec![vec!["F10"]],
            Self::OpenPatternSet => vec![vec!["F9"]],
            Self::OpenBatch => vec![vec!["F8"]],
            Self::OpenChartProperties => vec![vec!["F7"]],
            Self::OpenMarker => vec![vec!["F6"]],
            Self::RegeneratePattern => vec![vec!["F5"]],
            Self::OpenPattern => vec![vec!["F4"]],
            Self::OpenGeneration => vec![vec!["F3"]],
            Self::NewChart => vec![vec!["Ctrl", "N"]],
            Self::DeleteChart => vec![vec!["Ctrl", "LeftShift", "Back"]],
            Self::SaveRecovery => vec![vec!["Ctrl", "LeftShift", "S"]],
            Self::RecoverDraft => vec![vec!["Ctrl", "R"]],
            Self::DuplicateChart => vec![vec!["Ctrl", "D"]],
            Self::OpenChartList => vec![vec!["Ctrl", "B"]],
            Self::OpenKeyBindings => vec![vec!["Ctrl", "LeftShift", "K"]],
            Self::OpenReceptors => vec![vec!["Ctrl", "LeftShift", "H"]],
            Self::OpenNoteColors => vec![vec!["Ctrl", "LeftShift", "U"]],
            Self::CopyEvents => vec![vec!["Ctrl", "LeftShift", "C"]],
            Self::CutEvents => vec![vec!["Ctrl", "LeftShift", "X"]],
            Self::PasteEvents => vec![vec!["Ctrl", "LeftShift", "V"]],
            Self::OpenEventSelection => vec![vec!["Ctrl", "LeftShift", "E"]],
            Self::OpenChartEventCopy => vec![vec!["Ctrl", "LeftShift", "Y"]],
            Self::InspectAttack => vec![
                vec!["Ctrl", "Enter"],
                vec!["Ctrl", "NumPadEnter"],
                vec!["Ctrl", "LeftShift", "Enter"],
                vec!["Ctrl", "LeftShift", "NumPadEnter"],
            ],
            Self::PreviousAttack => vec![vec!["Ctrl", "LeftShift", "J"]],
            Self::NextAttack => vec![vec!["Ctrl", "J"]],
            Self::OpenScrollSettings => vec![vec!["LeftShift", "S"]],
            Self::OpenZoomSettings => vec![vec!["LeftShift", "Z"]],
            Self::ToggleReverse => vec![vec!["LeftShift", "V"]],
            Self::ToggleSpacing => vec![vec!["Ctrl", "T"]],
            Self::ToggleVariableSpacing => vec![vec!["Ctrl", "LeftShift", "T"]],
            Self::ZoomIn => vec![vec!["Ctrl", "Up"]],
            Self::ZoomOut => vec![vec!["Ctrl", "Down"]],
            Self::ResetZoom => vec![vec!["Ctrl", "D0"], vec!["Ctrl", "NumPad0"]],
            Self::ToggleAutoAdvance => vec![vec!["A"]],
            Self::NextTool => vec![vec!["Tab"]],
            Self::PreviousTool => vec![vec!["LeftShift", "Tab"]],
            Self::MoveUp => vec![vec!["Up"]],
            Self::MoveDown => vec![vec!["Down"]],
            Self::CloseTab => vec![vec!["Ctrl", "W"], vec!["Ctrl", "LeftShift", "W"]],
            Self::KeepTab => vec![vec!["Ctrl", "LeftShift", "B"]],
            Self::MoveTabLeft => vec![vec!["Ctrl", "LeftShift", "Left"]],
            Self::MoveTabRight => vec![vec!["Ctrl", "LeftShift", "Right"]],
            Self::OpenNoteAttributes => vec![vec![]],
            Self::OpenNoteConversion => vec![vec![]],
            Self::OpenNoteOwners => vec![vec![]],
            Self::OpenTransforms => vec![vec![]],
            Self::FileNew => vec![vec![]],
            Self::FileOpen => vec![vec![]],
            Self::FileReload => vec![vec![]],
            Self::FileRecent => vec![vec![]],
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub commands: BTreeMap<String, Vec<Vec<String>>>,
    /// Exact imported source text, including unknown fields/comments.
    pub source_import_raw: Option<std::sync::Arc<str>>,
    /// Source callback declaration order for equal-priority chords.
    pub source_order: bool,
    /// Resolved physical chords for unavailable source callbacks; never executable.
    pub source_blockers: BTreeMap<String, Vec<Vec<String>>>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            source_import_raw: None,
            source_order: false,
            source_blockers: BTreeMap::new(),
            commands: Action::ALL
                .into_iter()
                .map(|action| {
                    (
                        action.id().into(),
                        action
                            .native()
                            .into_iter()
                            .map(|chord| chord.into_iter().map(str::to_owned).collect())
                            .collect(),
                    )
                })
                .collect(),
        }
    }
}
impl Preferences {
    /// Ctrl represents the native existing accelerator (Control or Super),
    /// deliberately broader than GrooveAuthor's platform-specific default.
    pub fn bindings(&self) -> Vec<Binding> {
        let mut bindings: Vec<Binding> = Action::ALL
            .into_iter()
            .map(|action| Binding {
                id: action.id().into(),
                alternatives: self
                    .commands
                    .get(action.id())
                    .into_iter()
                    .flatten()
                    .flat_map(|keys| physical::expand(keys))
                    .collect(),
                repeat: action.repeats(),
                blocks: true,
            })
            .collect();
        if self.source_order {
            bindings.sort_by_key(|binding| {
                crate::key_commands::source::COMMANDS
                    .iter()
                    .position(|command| command.id == binding.id)
                    .unwrap_or(usize::MAX)
            });
        }
        if self.source_order {
            bindings.extend(source_profile::blockers(self));
        }
        bindings
    }
}

impl Preferences {
    pub fn read(fields: &serde_json::Map<String, serde_json::Value>) -> Result<Self, crate::Error> {
        let mut prefs = Self::default();
        if let Some(value) = fields.get(source_profile::BLOCKERS_KEY) {
            prefs.source_blockers = serde_json::from_value(value.clone()).map_err(|_| {
                crate::Error::InvalidFormat("Invalid unavailable source chords".into())
            })?;
        }
        if let Some(value) = fields.get(source_profile::ORDER_KEY) {
            prefs.source_order = value.as_bool().ok_or_else(|| {
                crate::Error::InvalidFormat("Source binding order flag must be boolean".into())
            })?;
        }
        if let Some(raw) = fields.get(source_profile::ARCHIVE_KEY) {
            let raw = raw.as_str().ok_or_else(|| {
                crate::Error::InvalidFormat("Source binding import archive must be text".into())
            })?;
            prefs.source_import_raw = Some(std::sync::Arc::from(raw));
        }
        for action in Action::ALL {
            let Some(value) = fields.get(action.id()) else {
                continue;
            };
            let alternatives: Vec<Vec<String>> =
                serde_json::from_value(value.clone()).map_err(|_| {
                    crate::Error::InvalidFormat(format!("Invalid key binding {}", action.id()))
                })?;
            prefs.commands.insert(action.id().into(), alternatives);
        }
        prefs.validate()?;
        Ok(prefs)
    }
    pub fn write(
        &self,
        fields: &mut serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), crate::Error> {
        self.validate()?;
        fields.insert(
            source_profile::BLOCKERS_KEY.into(),
            serde_json::to_value(&self.source_blockers).expect("String arrays serialize"),
        );
        fields.insert(
            source_profile::ORDER_KEY.into(),
            serde_json::Value::Bool(self.source_order),
        );
        if let Some(raw) = &self.source_import_raw {
            fields.insert(
                source_profile::ARCHIVE_KEY.into(),
                serde_json::Value::String(raw.to_string()),
            );
        } else {
            fields.remove(source_profile::ARCHIVE_KEY);
        }
        for action in Action::ALL {
            fields.insert(
                action.id().into(),
                serde_json::to_value(&self.commands[action.id()]).expect("String arrays serialize"),
            );
        }
        Ok(())
    }
    pub fn validate(&self) -> Result<(), crate::Error> {
        source_profile::validate_blockers(self)?;
        if self
            .source_import_raw
            .as_ref()
            .is_some_and(|raw| raw.len() > source_profile::MAX_SOURCE_BYTES)
        {
            return Err(crate::Error::InvalidEdit(
                "Source binding import archive exceeds input limit".into(),
            ));
        }
        if self.commands.keys().any(|id| Action::from_id(id).is_none()) {
            return Err(crate::Error::InvalidEdit(
                "Unknown executable key binding action".into(),
            ));
        }
        for action in Action::ALL {
            let alternatives = self.commands.get(action.id()).ok_or_else(|| {
                crate::Error::InvalidEdit(format!("Missing key binding {}", action.id()))
            })?;
            if alternatives.is_empty() || alternatives.len() > 16 {
                return Err(crate::Error::InvalidEdit(format!(
                    "Invalid alternatives for {}",
                    action.id()
                )));
            }
            for chord in alternatives {
                if chord.iter().any(|key| key == "Ctrl")
                    && chord.iter().any(|key| {
                        matches!(
                            key.as_str(),
                            "LeftControl" | "RightControl" | "LeftWindows" | "RightWindows"
                        )
                    })
                {
                    return Err(crate::Error::InvalidEdit(
                        "Do not mix native Ctrl alias and physical Control/Super in one chord"
                            .into(),
                    ));
                }
                if chord.len() > 8 {
                    return Err(crate::Error::InvalidEdit(format!(
                        "Key chord too long for {}",
                        action.id()
                    )));
                }
                let mut seen = std::collections::BTreeSet::new();
                for key in chord {
                    if !supported_key(key) || !seen.insert(Key::named(key)) {
                        return Err(crate::Error::InvalidEdit(format!(
                            "Unsupported or duplicate key {key} for {}",
                            action.id()
                        )));
                    }
                }
                if !reservations::simple_chord(chord) || reservations::conflicts(chord) {
                    return Err(crate::Error::InvalidEdit(format!(
                        "Key chord is reserved by an unmigrated native action or needs unsupported multi-key dispatch for {}",
                        action.id()
                    )));
                }
            }
        }
        Ok(())
    }
}
/// Explicit portable key names accepted by the first executable subset.
/// Unsupported source names remain untouched in unknown command fields.
pub fn supported_key(key: &str) -> bool {
    matches!(
        key,
        "Ctrl"
            | "LeftControl"
            | "RightControl"
            | "LeftWindows"
            | "RightWindows"
            | "OemQuestion"
            | "LeftShift"
            | "RightShift"
            | "LeftAlt"
            | "RightAlt"
            | "Space"
            | "Enter"
            | "NumPadEnter"
            | "NumPad0"
            | "Escape"
            | "Tab"
            | "Back"
            | "Delete"
            | "Insert"
            | "Home"
            | "End"
            | "PageUp"
            | "PageDown"
            | "Up"
            | "Down"
            | "Left"
            | "Right"
    ) || (key.len() == 1 && key.as_bytes()[0].is_ascii_uppercase())
        || key
            .strip_prefix('D')
            .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
        || key
            .strip_prefix('F')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n))
}
