//! Canonical format chart types and aliases from Fumen SMCommon,
//! pin 0fe292f9c0ef70fb77d61cb54316a7e78dbf3c89.
pub const ALL: &[&str] = &[
    "dance-single",
    "dance-double",
    "dance-couple",
    "dance-solo",
    "dance-threepanel",
    "dance-routine",
    "pump-single",
    "pump-halfdouble",
    "pump-double",
    "pump-couple",
    "pump-routine",
    "kb7-single",
    "ez2-single",
    "ez2-double",
    "ez2-real",
    "para-single",
    "ds3ddx-single",
    "bm-single5",
    "bm-versus5",
    "bm-double5",
    "bm-single7",
    "bm-versus7",
    "bm-double7",
    "maniax-single",
    "maniax-double",
    "techno-single4",
    "techno-single5",
    "techno-single8",
    "techno-double4",
    "techno-double5",
    "techno-double8",
    "pnm-five",
    "pnm-nine",
    "lights-cabinet",
    "kickbox-human",
    "kickbox-quadarm",
    "kickbox-insect",
    "kickbox-arachnid",
    "smx-beginner",
    "smx-single",
    "smx-dual",
    "smx-full",
    "smx-team",
];
pub fn canonical(value: &str) -> Option<&'static str> {
    let value = value.trim().replace('_', "-");
    if let Some(layout) = ALL.iter().find(|layout| **layout == value) {
        return Some(*layout);
    }
    if let Ok(index) = value.parse::<usize>() {
        return ALL.get(index).copied();
    }
    match value.as_str() {
        "smx-double6" => Some("smx-dual"),
        "smx-double10" => Some("smx-full"),
        "smx-couple" | "smx-routine" => Some("smx-team"),
        _ => None,
    }
}
