use deadsync_editor::generation::profiles::{Config, Kind, Profile, TravelSpeed};

#[test]
fn all_new_editor_profiles_start_valid_and_roundtrip_complete_editor_defaults() {
    for kind in [Kind::Pattern, Kind::Performed, Kind::Expressed] {
        let profile = Profile::new_editor_default(kind).unwrap();
        let second = Profile::new_editor_default(kind).unwrap();
        assert_ne!(profile.guid(), second.guid());
        assert_eq!(profile.config(), &Config::editor_defaults(kind));
        assert_eq!(profile.name(), None);
        let bytes = profile.serialize().unwrap();
        let mut restored = Profile::parse(kind, &bytes).unwrap();
        assert_eq!(restored.guid(), profile.guid());
        assert_eq!(restored.config(), profile.config());
        assert!(!restored.restore_defaults().unwrap());
        if kind == Kind::Performed {
            assert_eq!(restored.travel_speed(), Some(TravelSpeed::editor_defaults()));
        }
    }
}
