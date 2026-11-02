//! Recover the logical source center from the renderer's cropped-quad compensation.
use deadlib_present::actors::{Actor, SizeSpec};
pub fn center(actor: &Actor) -> Option<[f32; 2]> {
    let Actor::Sprite {
        offset,
        size: [SizeSpec::Px(width), SizeSpec::Px(height)],
        rot_z_deg,
        cropleft,
        cropright,
        croptop,
        cropbottom,
        ..
    } = actor
    else {
        return None;
    };
    let delta = [
        (cropleft - cropright) * width * 0.5,
        (croptop - cropbottom) * height * 0.5,
    ];
    let (sin, cos) = rot_z_deg.rem_euclid(360.0).to_radians().sin_cos();
    Some([
        offset[0] - (cos * delta[0] + sin * delta[1] - delta[0]),
        offset[1] - (-sin * delta[0] + cos * delta[1] - delta[1]),
    ])
}
