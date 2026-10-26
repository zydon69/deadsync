//! Source default large-chart palette. Custom note-color preferences are a
//! separate UI contract; callers explicitly select source multiplayer/PIU modes.
#[path = "hold_colors.rs"]
mod colors;
#[path = "piu_fill.rs"]
mod piu;
pub use colors::Activity;
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub multiplayer: bool,
    pub color_multiplayer_holds: bool,
    pub row_based_piu: bool,
    pub light_holds: bool,
    pub hide_arrows: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            multiplayer: false,
            color_multiplayer_holds: true,
            row_based_piu: false,
            light_holds: true,
            hide_arrows: true,
        }
    }
}
#[derive(Debug, PartialEq)]
pub struct Palette {
    pub head: [u8; 4],
    pub start: [u8; 4],
    pub body: [u8; 4],
    pub cap: [u8; 4],
    pub activity: Activity,
}
const ROW_COLORS: [[f32; 3]; 9] = [
    [0.7137255, 0.0941176, 0.0941176],
    [0.0941176, 0.2078431, 0.7137255],
    [0.2117647, 0.6784314, 0.2156863],
    [0.8745098, 0.8470588, 0.0588235],
    [0.5176471, 0.0941176, 0.7137255],
    [0.0941176, 0.7137255, 0.5960784],
    [0.7137255, 0.0941176, 0.5019607],
    [0.3098039, 0.4352941, 0.3450980],
    [0.3098039, 0.4352941, 0.3450980],
];
pub fn row_color(row: u64) -> [f32; 3] {
    fn gcd(mut a: u64, mut b: u64) -> u64 {
        while b != 0 {
            (a, b) = (b, a % b);
        }
        a
    }
    let denominator = 48 / gcd(row % 48, 48);
    ROW_COLORS[match denominator {
        1 => 0,
        2 => 1,
        3 => 2,
        4 => 3,
        6 => 4,
        8 => 5,
        12 => 6,
        16 => 7,
        _ => 8,
    }]
}
pub fn palette(
    layout: &str,
    row: u64,
    lane: u16,
    player: usize,
    selected: bool,
    roll: bool,
    active: bool,
    options: Options,
) -> Result<Palette, &'static str> {
    let columns =
        deadsync_editor::notes::layout_columns(layout).ok_or("Unsupported hold palette layout")?;
    if lane >= columns {
        return Err("Hold palette lane outside layout");
    }
    let activity = colors::activity(active, options.light_holds, options.hide_arrows);
    let pump = layout.starts_with("pump-");
    let index = (usize::from(lane) + if layout == "pump-halfdouble" { 2 } else { 0 }) % 5;
    let lane_piu = pump && !options.row_based_piu;
    let note_rgb = if lane_piu {
        piu::lane_color(index)
    } else {
        row_color(row)
    };
    let note_rgb = if options.multiplayer {
        colors::multiplayer(note_rgb, player, 0.9)?
    } else {
        note_rgb
    };
    let base = if lane_piu {
        piu::lane_color(index)
    } else if roll {
        colors::ROLL
    } else {
        colors::HOLD
    };
    // Lane-based PIU always colors multiplayer bodies, unlike optional row-based
    // PIU and Dance/SMX (ArrowGraphicManagerPIURoutine vs ...RoutineRowBased).
    let base = if options.multiplayer && (lane_piu || options.color_multiplayer_holds) {
        colors::multiplayer(base, player, 0.9)?
    } else {
        base
    };
    let base = if lane_piu {
        piu::desaturate(base, 0.8)?
    } else {
        base
    };
    let variant = |rgb, held| colors::variant(rgb, held, selected, Default::default());
    let head = variant(note_rgb, false)?;
    let start = variant(base, activity.start_held)?;
    let body = variant(base, activity.body_held)?;
    let cap = if pump { head } else { body };
    Ok(Palette {
        head,
        start,
        body,
        cap,
        activity,
    })
}
