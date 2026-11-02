use deadlib_present::{
    actors::{Actor, SpriteSource},
    space::{Metrics, set_current_metrics},
};
use deadsync_theme_simply_love::{i18n, screens::edition::State};
fn setup() -> (tempfile::TempDir, State) {
    i18n::init(deadsync_assets::language::load_for_tests(super::LANGUAGE));
    set_current_metrics(Metrics::centered(854., 480.));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.ssc");
    std::fs::write(&path,"#VERSION:0.83;#BPMS:0=120;#OFFSET:0;#SAMPLESTART:0;#SAMPLELENGTH:0.5;#LASTSECONDHINT:0.5;#GASongDataVersion:1;#GASongData:{\"SyncOffset\":0};#NOTEDATA:;#STEPSTYPE:dance-single;#NOTES:1000\n0100\n0010\n0001;").unwrap();
    let s = State::open(&path, 0).unwrap();
    (dir, s)
}
fn draw(s: &State) -> Vec<Actor> {
    fn walk(a: Actor, v: &mut Vec<Actor>) {
        match a {
            Actor::Frame { children, .. } => {
                for a in children {
                    walk(a, v)
                }
            }
            a => v.push(a),
        }
    }
    let mut actors = Vec::new();
    s.actors(&mut actors);
    let mut flat = Vec::new();
    for a in actors {
        walk(a, &mut flat)
    }
    flat
}
fn atlas(a: &Actor, z: i16) -> bool {
    matches!(a,Actor::Sprite{z:az,visible:true,source:SpriteSource::TextureStatic(k),..} if *az==z&&k.ends_with("editor-grooveauthor/atlas.png"))
        || matches!(a,Actor::Sprite{z:az,visible:true,source:SpriteSource::Texture(k)|SpriteSource::TextureHandle{key:k,..},..} if *az==z&&k.ends_with("editor-grooveauthor/atlas.png"))
}
fn sample_fill(a: &Actor) -> bool {
    matches!(a,Actor::Sprite{source:SpriteSource::Solid,tint,..} if *tint==[0.2,0.85,0.65,0.14])
}
fn measure(a: &Actor) -> bool {
    matches!(a,Actor::Sprite{source:SpriteSource::Solid,tint,..} if *tint==[0.6,0.7,0.8,0.4]||*tint==[0.6,0.7,0.8,0.15])
}
#[test]
fn hiding_notes_keeps_receptors_and_non_note_layers() {
    let (_dir, mut s) = setup();
    let original = s.workspace().file().session.serialize().unwrap();
    let before = draw(&s);
    assert!(before.iter().any(|a| atlas(a, 8004)));
    let receptors = before.iter().filter(|a| atlas(a, 8001)).count();
    assert!(receptors >= 4);
    let mut p = deadsync_editor::options_preferences::Display::default();
    p.render.notes = false;
    s.configure_display_options(p);
    let after = draw(&s);
    assert!(!after.iter().any(|a| atlas(a, 8004)));
    assert_eq!(after.iter().filter(|a| atlas(a, 8001)).count(), receptors);
    assert!(after.iter().any(sample_fill));
    assert_eq!(s.workspace().file().session.serialize().unwrap(), original);
    assert!(!s.workspace().file().session.is_dirty());
}
#[test]
fn regions_headers_and_measure_markers_are_independent() {
    let (_dir, mut s) = setup();
    let before = draw(&s);
    assert!(before.iter().any(sample_fill));
    assert!(before.iter().any(measure));
    let header = i18n::tr("Edition", "SongSampleRegion");
    assert!(
        before
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..} if content.as_str()==&*header))
    );
    let mut p = deadsync_editor::options_preferences::Display::default();
    p.render.regions = false;
    s.configure_display_options(p);
    let regions = draw(&s);
    assert!(!regions.iter().any(sample_fill));
    assert!(
        regions
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..} if content.as_str()==&*header))
    );
    assert!(regions.iter().any(measure));
    p.render.regions = true;
    p.render.misc_events = false;
    p.render.markers = false;
    s.configure_display_options(p);
    let misc = draw(&s);
    assert!(misc.iter().any(sample_fill));
    assert!(!misc.iter().any(measure));
    assert!(
        !misc
            .iter()
            .any(|a| matches!(a,Actor::Text{content,..} if content.as_str()==&*header))
    );
    assert!(misc.iter().any(|a| atlas(a, 8004)));
}
