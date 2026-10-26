//! Editor.DrawPlayerIndicator (48e18f71, MIT): selected editing participant,
//! beside the focused field at the receptor line. This is not a per-note badge.
use deadlib_present::actors::Actor;
pub fn source_multiplayer(layout: &str) -> bool {
    matches!(
        layout,
        "dance-couple" | "dance-routine" | "pump-couple" | "pump-routine" | "smx-team"
    )
}
pub fn actors(
    layout: &str,
    focused: bool,
    lanes_end: f32,
    focal_y: f32,
    scale: f32,
    raw_player_rgb: [f32; 3],
) -> Result<Vec<Actor>, &'static str> {
    if ![lanes_end, focal_y, scale]
        .into_iter()
        .chain(raw_player_rgb)
        .all(|n| n.is_finite())
        || scale <= 0.0
    {
        return Err("Invalid player indicator projection");
    }
    if !focused || !source_multiplayer(layout) {
        return Ok(Vec::new());
    }
    let size = 40.0 * scale;
    let x = lanes_end + size * 0.5;
    // Source PreferencesNoteColor.PlayerColor.XnaColor, with the original
    // MonoGame float-to-byte conversion; UI multipliers do not apply here.
    let fill = raw_player_rgb.map(|c| f32::from((c.clamp(0.0, 1.0) * 255.0) as u8) / 255.0);
    let mut out = Vec::with_capacity(2);
    for (rect, color) in [
        ([589u32, 1215, 40, 40], [fill[0], fill[1], fill[2], 1.0]),
        ([783, 717, 40, 40], [1.0; 4]),
    ] {
        let [a, b, w, h] = rect;
        out.push(deadlib_present::__act_from_builder!(
   (xy(x,focal_y): setsize(size,size): customtexturerect(a as f32/1280.0,b as f32/1280.0,(a+w) as f32/1280.0,(b+h) as f32/1280.0): diffuse(color[0],color[1],color[2],color[3]): z(8002))
   ::deadsync_assets::present_dsl::SpriteBuilder::static_texture("editor-grooveauthor/atlas.png")
  ));
    }
    Ok(out)
}
