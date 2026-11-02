//! Serialize whole test scenarios that share the process-wide profile runtime.
//! Production mutexes protect individual operations, not a fixture's multi-step
//! ownership of session, profiles and the profile-directory cache.
use super::*;
static SERIAL: Mutex<()> = Mutex::new(());
pub(super) fn lock() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .expect("profile runtime test scenario poisoned")
}
/// Restore known fixture snapshots even during assertion unwinding. Recovering
/// a poisoned data mutex here does not suppress the test's original failure;
/// it prevents a second panic in Drop from aborting the entire test process.
pub(super) fn restore(
    session: SessionState,
    profiles: [Profile; PLAYER_SLOTS],
    cache: Option<RuntimeProfileDirCache>,
) {
    restore_slot(&RUNTIME_SESSION, session);
    restore_slot(&RUNTIME_PROFILES, profiles);
    restore_slot(&RUNTIME_PROFILE_DIR_CACHE, cache);
    RUNTIME_PROFILE_GENERATION.fetch_add(1, Ordering::Release);
}

fn restore_slot<T>(slot: &Mutex<T>, value: T) {
    *slot
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = value;
    slot.clear_poison();
}
#[test]
fn cleanup_restores_known_snapshot_after_an_assertion_poisons_a_data_lock() {
    let slot = Mutex::new(vec![1, 2]);
    let failure = std::panic::catch_unwind(|| {
        let mut current = slot.lock().unwrap();
        current.push(3);
        assert_eq!(current.len(), 2, "fixture failure remains observable");
    });
    assert!(failure.is_err());
    assert!(slot.is_poisoned());
    restore_slot(&slot, vec![1, 2]);
    assert!(!slot.is_poisoned());
    assert_eq!(*slot.lock().unwrap(), [1, 2]);
}
