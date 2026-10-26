//! ArrowGraphicManager hold texture resolution (48e18f71, MIT).
//! Color is resolved separately: PIU caps use note color rather than body color.
#[derive(Debug, PartialEq)]
pub struct Graphic {
    pub head_fill: String,
    pub head_rim: String,
    pub head_rotation: f32,
    pub start_fill: Option<String>,
    pub start_rim: Option<String>,
    pub body_fill: String,
    pub body_rim: String,
    pub body_mirrored: bool,
    pub cap_fill: String,
    pub cap_rim: String,
    pub cap_rotation: f32,
    pub cap_centered: bool,
    pub flip_cap_in_reverse: bool,
}
fn selected(name: String, selected: bool) -> String {
    if selected { name + "-selected" } else { name }
}
pub fn graphic(
    layout: &str,
    lane: u16,
    selection: bool,
    roll: bool,
) -> Result<Graphic, &'static str> {
    let columns =
        deadsync_editor::notes::layout_columns(layout).ok_or("Unsupported hold layout")?;
    if lane >= columns {
        return Err("Hold lane outside layout");
    }
    let i = usize::from(lane);
    let kind = if roll { "roll" } else { "hold" };
    let (head, rotation, body, start, pump, index) = match layout {
        "dance-single" | "dance-double" | "dance-couple" | "dance-routine" => (
            "itg-down",
            [90.0, 0.0, 180.0, 270.0][i % 4],
            "itg-down",
            false,
            false,
            0,
        ),
        "dance-solo" | "dance-threepanel" => {
            let diagonal = if layout == "dance-solo" {
                i == 1 || i == 4
            } else {
                i == 0 || i == 2
            };
            let rotation = if layout == "dance-solo" {
                [90.0, 0.0, 0.0, 180.0, 90.0, 270.0][i]
            } else {
                [0.0, 0.0, 90.0][i]
            };
            (
                if diagonal { "itg-solo" } else { "itg-down" },
                rotation,
                "itg-solo",
                diagonal,
                false,
                0,
            )
        }
        "smx-beginner" | "smx-dual" | "smx-single" | "smx-full" | "smx-team" => {
            let three = layout == "smx-beginner" || layout == "smx-dual";
            let center = if three { i % 3 == 1 } else { i % 5 == 2 };
            let rotation = if three {
                [90.0, 0.0, 270.0][i % 3]
            } else {
                [90.0, 0.0, 0.0, 180.0, 270.0][i % 5]
            };
            (
                if center { "itg-center" } else { "itg-down" },
                rotation,
                "itg-center",
                false,
                false,
                0,
            )
        }
        _ => {
            let j = (i + if layout == "pump-halfdouble" { 2 } else { 0 }) % 5;
            let shape = if j == 2 { "piu-center" } else { "piu-diagonal" };
            (
                shape,
                [270.0, 0.0, 0.0, 90.0, 180.0][j],
                shape,
                false,
                true,
                j,
            )
        }
    };
    Ok(Graphic {
        head_fill: format!("{head}-fill"),
        head_rim: selected(format!("{head}-rim"), selection),
        head_rotation: rotation,
        start_fill: start.then(|| format!("itg-solo-{kind}-start-fill")),
        start_rim: start.then(|| selected("itg-solo-hold-start-rim".into(), selection)),
        body_fill: format!("{body}-{kind}-fill"),
        body_rim: selected(format!("{body}-hold-rim"), selection),
        body_mirrored: pump && index >= 3,
        cap_fill: if pump {
            format!("{head}-fill")
        } else {
            format!("{body}-{kind}-end-fill")
        },
        cap_rim: selected(
            if pump {
                format!("{head}-rim")
            } else {
                format!("{body}-hold-end-rim")
            },
            selection,
        ),
        cap_rotation: if pump { rotation } else { 0.0 },
        cap_centered: pump,
        flip_cap_in_reverse: !pump,
    })
}
