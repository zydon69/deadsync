//! Prepared source snap contract; module declaration intentionally deferred.
use deadsync_editor::{
    snap::{Level, Preferences},
    timing_edit::signature::Signatures,
};
#[test]
fn native_indices_remain_compatible_and_source_off_is_explicit() {
    for native in 0..=9 {
        let l = Level::native(native).unwrap();
        assert_eq!(l.native_index(), native);
    }
    assert_eq!(Level::native(0).unwrap().rows(), 48);
    let off = Level::native(9).unwrap();
    assert_eq!(off.rows(), 0);
    assert_eq!(off.denominator(), None);
    assert_eq!(Level::source(0).unwrap(), off);
    assert!(Level::native(10).is_err());
    assert!(Level::source(10).is_err());
}
#[test]
fn off_navigation_uses_departure_signature_and_preserves_fractional_phase() {
    let sig = Signatures::parse("0=4=4,1=3=8").unwrap();
    let off = Level::source(0).unwrap();
    assert_eq!(off.navigation(24.25, true, &sig).unwrap(), 216.25);
    assert_eq!(off.navigation(48.25, true, &sig).unwrap(), 120.25);
    assert_eq!(off.navigation(48.25, false, &sig).unwrap(), -23.75);
    assert!(off.navigation(f64::NAN, true, &sig).is_err());
}
#[test]
fn snapped_navigation_retains_source_grid_boundary_rules() {
    let sig = Signatures::parse("0=4=4").unwrap();
    let quarter = Level::source(1).unwrap();
    assert_eq!(quarter.navigation(24.25, true, &sig).unwrap(), 48.0);
    assert_eq!(quarter.navigation(24.25, false, &sig).unwrap(), 0.0);
    assert_eq!(quarter.navigation(48.0, false, &sig).unwrap(), 0.0);
    assert_eq!(quarter.denominator(), Some(4));
}
#[test]
fn source_lock_cycles_through_off_without_changing_legacy_index_meaning() {
    let lock = Level::source(4).unwrap(); // 1/16, subdivision four.
    let off = Level::source(0).unwrap();
    assert!(off.allowed(lock));
    assert_eq!(off.cycle(true, lock), Level::source(1).unwrap());
    assert_eq!(off.cycle(false, lock), lock);
    assert!(!Level::source(3).unwrap().allowed(lock)); // triplet is not divisor.
}
#[test]
fn source_preference_indices_roundtrip_and_invalid_values_normalize_to_off() {
    let mut map = serde_json::json!({"SnapIndex":-1,"SnapLockIndex":100,"Vendor":42})
        .as_object()
        .unwrap()
        .clone();
    assert_eq!(Preferences::read(&map), Preferences::default());
    let p = Preferences {
        level: Level::source(7).unwrap(),
        lock: Level::source(4).unwrap(),
    };
    p.write(&mut map);
    assert_eq!(Preferences::read(&map), p);
    assert_eq!(map["Vendor"], 42);
}
