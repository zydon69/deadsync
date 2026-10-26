//! Large-chart XNA fill colors; these are not the packed UI/minimap colors.
//! PreferencesNoteColor.ArrowColor (48e18f71, MIT), MonoGame Color(float) truncates.
#[derive(Clone, Copy, Debug)]
pub struct Multipliers {
    pub held: f32,
    pub selected: f32,
}
impl Default for Multipliers {
    fn default() -> Self {
        Self {
            held: 1.5,
            selected: 1.7,
        }
    }
}
pub fn variant(
    rgb: [f32; 3],
    held: bool,
    selected: bool,
    m: Multipliers,
) -> Result<[u8; 4], &'static str> {
    if !rgb
        .iter()
        .chain([&m.held, &m.selected])
        .all(|n| n.is_finite())
    {
        return Err("Nonfinite hold color");
    }
    let mut rgba = [0, 0, 0, 255];
    for (i, mut c) in rgb.into_iter().enumerate() {
        // Preserve the original float multiplication order before byte packing.
        if held {
            c *= m.held;
        }
        if selected {
            c *= m.selected;
        }
        rgba[i] = (c.clamp(0.0, 1.0) * 255.0) as u8;
    }
    Ok(rgba)
}
/// Source HoldRenderState uses raw active for the start, but LightHolds for
/// body/end. Hiding/cutoff is independent of fill lighting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Activity {
    pub start_held: bool,
    pub body_held: bool,
    pub cutoff: bool,
}
pub fn activity(active: bool, light_holds: bool, hide_arrows: bool) -> Activity {
    Activity {
        start_held: active,
        body_held: active && light_holds,
        cutoff: active && hide_arrows,
    }
}
pub const HOLD: [f32; 3] = [0.2666667, 0.2666667, 0.2666667];
pub const ROLL: [f32; 3] = [0.7803921, 0.4666667, 0.1607843];
pub const PLAYERS: [[f32; 3]; 10] = [
    [0.7109375, 0.09375, 0.7109375],
    [0.09375, 0.7109375, 0.7109375],
    [0.7109375, 0.7109375, 0.09375],
    [0.09375, 0.7109375, 0.09375],
    [0.7109375, 0.390625, 0.09375],
    [0.7109375, 0.7109375, 0.7109375],
    [0.09375, 0.09375, 0.09375],
    [0.7109375, 0.390625, 0.390625],
    [0.390625, 0.390625, 0.7109375],
    [0.390625, 0.7109375, 0.390625],
];
/// Source default RoutineNoteColorAlpha=0.90, shared holds optionally uncolored.
/// The renderer chooses when its source layout uses the multiplayer palette.
pub fn multiplayer(rgb: [f32; 3], player: usize, alpha: f32) -> Result<[f32; 3], &'static str> {
    if !alpha.is_finite() || !rgb.iter().all(|n| n.is_finite()) {
        return Err("Nonfinite multiplayer hold color");
    }
    let owner = PLAYERS[player % PLAYERS.len()];
    Ok(std::array::from_fn(|i| {
        (rgb[i] * (1.0 - alpha) + owner[i] * alpha).clamp(0.0, 1.0)
    }))
}
