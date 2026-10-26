// Original GrooveAuthor root atlas.json rectangles (MIT, pinned 48e18f71).
pub(super) fn rect(name: &str) -> Option<[u32; 4]> {
    Some(match name {
        "itg-down-receptor" => [1, 1, 128, 128],
        "itg-down-receptor-held" => [197, 1, 128, 128],
        "itg-down-receptor-glow" => [393, 1, 128, 128],
        "itg-solo-receptor" => [131, 131, 128, 128],
        "itg-solo-receptor-held" => [327, 131, 128, 128],
        "itg-solo-receptor-glow" => [523, 131, 128, 128],
        "itg-center-receptor" => [197, 1121, 128, 128],
        "itg-center-receptor-held" => [327, 261, 128, 128],
        "itg-center-receptor-glow" => [457, 261, 128, 128],
        "piu-diagonal-receptor" => [913, 391, 128, 128],
        "piu-diagonal-receptor-held" => [1043, 391, 128, 128],
        "piu-diagonal-receptor-glow" => [393, 521, 128, 128],
        "piu-center-receptor" => [1045, 521, 128, 128],
        "piu-center-receptor-held" => [523, 651, 128, 128],
        "piu-center-receptor-glow" => [523, 847, 128, 128],
        _ => return None,
    })
}
