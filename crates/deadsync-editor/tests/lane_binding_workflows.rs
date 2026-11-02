use deadsync_editor::{key_binding_preferences::{Action,Preferences,source_profile::{self,Platform}},key_commands::{Effect,Key,Registry}};
fn keys(names:&[&str])->Vec<Key>{names.iter().map(|name|Key::named(name)).collect()}
#[test]
fn eleven_entry_callbacks_have_real_nonrepeat_registration_and_nonblocking_modifier() {
    let preferences=Preferences::default();preferences.validate().unwrap();
    for lane in 0..10 {
        let id=format!("Arrow{lane}");let action=Action::from_id(&id).unwrap();
        assert_eq!(action.lane(),Some(lane));assert!(!action.repeats());
        assert_eq!(preferences.commands[&id],vec![vec![format!("D{}",(lane+1)%10)]]);
        let source=deadsync_editor::key_commands::source::COMMANDS.iter().find(|entry|entry.id==id).unwrap();assert!(source.release);assert!(!source.repeat);
    }
    let bindings=preferences.bindings();
    assert!(!bindings.iter().find(|binding|binding.id=="ArrowModification").unwrap().blocks);
    assert!(deadsync_editor::key_binding_preferences::reservations::legacy().is_empty());
    let mut registry=Registry::new(bindings);
    assert_eq!(registry.update(0.0,keys(&["LeftShift"])),vec![Effect::Press("ArrowModification".into())]);
    let pressed=registry.update(0.1,keys(&["LeftShift","D1"]));assert!(pressed.contains(&Effect::Press("Arrow0".into())));
    assert_eq!(registry.update(0.2,keys(&["D1"])),vec![Effect::Release("ArrowModification".into())]);
    assert_eq!(registry.update(0.3,keys(&[])),vec![Effect::Release("Arrow0".into())]);
}
#[test]
fn modifier_only_validation_and_source_import_keep_original_physical_semantics() {
    let mut preferences=Preferences::default();
    preferences.commands.insert("ArrowModification".into(),vec![vec!["RightControl".into(),"RightShift".into()]]);preferences.validate().unwrap();
    preferences.commands.insert("Save".into(),vec![vec!["LeftShift".into()]]);preferences.validate().unwrap();
    let raw=r#"{"Arrow0":[["LeftControl","F13"],["RightWindows","F13"]],"ArrowModification":[["RightShift"]]}"#;
    let prepared=source_profile::import(&Preferences::default(),raw,Platform::Mac).unwrap();
    assert_eq!(prepared.preferences.commands["Arrow0"],vec![vec!["LeftControl","F13"],vec!["LeftWindows","F13"]]);
    assert_eq!(prepared.preferences.commands["ArrowModification"],vec![vec!["LeftShift"]]);
    assert_eq!(prepared.preferences.source_import_raw.as_deref(),Some(raw));
    assert!(prepared.report.applied.iter().any(|id|id=="ArrowModification"));
    assert!(!prepared.report.unavailable_commands.iter().any(|id|id.starts_with("Arrow")));
    assert!(prepared.report.rejected.is_empty(),"{:?}",prepared.report.rejected);
}
#[test]
fn native_alias_ownership_releases_only_after_last_control_or_super_alternative() {
    use deadsync_editor::key_binding_preferences::physical::Effects;
    let mut preferences=Preferences::default();preferences.commands.insert("Arrow0".into(),vec![vec!["Ctrl".into(),"F13".into()]]);
    let mut registry=Registry::new(preferences.bindings());let mut effects=Effects::default();
    let mut evaluate=|time,down:&[&str]|{let raw=registry.update(time,keys(down));effects.coalesce(&registry,raw)};
    assert!(evaluate(0.0,&["LeftControl","LeftWindows"]).is_empty());
    assert_eq!(evaluate(0.1,&["LeftControl","LeftWindows","F13"]),vec![Effect::Press("Arrow0".into())]);
    assert!(evaluate(0.2,&["LeftWindows","F13"]).is_empty());
    assert_eq!(evaluate(0.3,&["F13"]),vec![Effect::Release("Arrow0".into())]);
    assert!(evaluate(0.4,&[]).is_empty());
}
