//! Reconcile window modifier snapshots when native raw input is unavailable.
use winit::keyboard::{KeyCode, ModifiersState};

pub fn changes(held: [bool; 4], state: ModifiersState) -> Vec<(KeyCode, bool)> {
    [
        (KeyCode::ShiftLeft, state.shift_key()),
        (KeyCode::ControlLeft, state.control_key()),
        (KeyCode::AltLeft, state.alt_key()),
        (KeyCode::SuperLeft, state.super_key()),
    ]
    .into_iter()
    .zip(held)
    .filter_map(|((code, pressed), previous)| (pressed != previous).then_some((code, pressed)))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_snapshot_recovers_missing_key_event_and_releases_cleanly() {
        assert_eq!(
            changes([false; 4], ModifiersState::SUPER),
            [(KeyCode::SuperLeft, true)]
        );
        assert!(changes([false, false, false, true], ModifiersState::SUPER).is_empty());
        assert_eq!(
            changes([false, false, false, true], ModifiersState::empty()),
            [(KeyCode::SuperLeft, false)]
        );
    }

    #[test]
    fn snapshot_reconciles_multiple_modifiers_without_repeating_held_keys() {
        assert_eq!(
            changes(
                [true, false, false, true],
                ModifiersState::SHIFT | ModifiersState::CONTROL
            ),
            [(KeyCode::ControlLeft, true), (KeyCode::SuperLeft, false)]
        );
    }
}
