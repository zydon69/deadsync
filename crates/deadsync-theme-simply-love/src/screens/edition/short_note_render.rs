//! Source DrawTap/DrawFakeMarker and Mine/Lift composition (GrooveAuthor MIT).
//! Colors and fake-region classification are supplied by the owning chart.
#[path = "hold_atlas.rs"]
mod atlas;
#[path = "hold_graphics.rs"]
mod graphics;
use deadlib_present::actors::Actor;

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Tap,
    Mine,
    Lift,
}
pub struct Request<'a> {
    pub layout: &'a str,
    pub lane: u16,
    pub kind: Kind,
    pub selected: bool,
    /// Includes explicit fake notes and notes inside a fake region.
    pub fake: bool,
    pub center: [f32; 2],
    pub width: f32,
    pub fill: [u8; 4],
    pub alpha: f32,
}
fn rect(id: &str) -> Option<[u32; 4]> {
    match id {
        "mine-fill" => Some([849, 911, 128, 128]),
        "mine-rim" => Some([913, 781, 128, 128]),
        "mine-rim-selected" => Some([1109, 781, 128, 128]),
        "fake-marker" => Some([523, 1239, 40, 40]),
        "lift-marker" => Some([589, 1173, 40, 40]),
        _ => atlas::rect(id),
    }
}
fn sprite(
    id: &str,
    center: [f32; 2],
    scale: f32,
    rotation: f32,
    color: [f32; 4],
) -> Result<Actor, &'static str> {
    let [x, y, w, h] = rect(id).ok_or("Unknown original short-note texture")?;
    Ok(deadlib_present::__act_from_builder!(
        (xy(center[0],center[1]):setsize(w as f32*scale,h as f32*scale):
         customtexturerect(x as f32/1280.0,y as f32/1280.0,(x+w) as f32/1280.0,(y+h) as f32/1280.0):
         rotationz(rotation):diffuse(color[0],color[1],color[2],color[3]):z(8004))
        ::deadsync_assets::present_dsl::SpriteBuilder::static_texture("editor-grooveauthor/atlas.png")
    ))
}
/// Fill, rim, then an unselected and unrotated fake/lift marker. Fake wins over
/// lift. Source fill alpha is byte-quantized; rim/marker use the float alpha.
pub fn render(r: Request<'_>) -> Result<Vec<Actor>, &'static str> {
    if !r
        .center
        .into_iter()
        .chain([r.width, r.alpha])
        .all(f32::is_finite)
        || r.width <= 0.0
        || !(0.0..=1.0).contains(&r.alpha)
    {
        return Err("Invalid short-note projection");
    }
    let graphic = graphics::graphic(r.layout, r.lane, r.selected, false)?;
    if r.alpha == 0.0 {
        return Ok(Vec::new());
    }
    let scale = r.width / 128.0;
    let mut color = r.fill.map(|v| f32::from(v) / 255.0);
    let source_alpha = if matches!(r.kind, Kind::Mine) {
        255
    } else {
        r.fill[3]
    };
    color[3] = f32::from((f32::from(source_alpha) * r.alpha) as u8) / 255.0;
    let (fill, rim, rotation) = match r.kind {
        Kind::Mine => (
            "mine-fill",
            if r.selected {
                "mine-rim-selected"
            } else {
                "mine-rim"
            },
            0.0,
        ),
        _ => (
            graphic.head_fill.as_str(),
            graphic.head_rim.as_str(),
            graphic.head_rotation,
        ),
    };
    let mut actors = vec![
        sprite(fill, r.center, scale, rotation, color)?,
        sprite(rim, r.center, scale, rotation, [1.0, 1.0, 1.0, r.alpha])?,
    ];
    let marker = if r.fake {
        Some("fake-marker")
    } else if matches!(r.kind, Kind::Lift) {
        Some("lift-marker")
    } else {
        None
    };
    if let Some(marker) = marker {
        actors.push(sprite(
            marker,
            r.center,
            scale,
            0.0,
            [1.0, 1.0, 1.0, r.alpha],
        )?);
    }
    Ok(actors)
}
