//! PreferencesReceptors source effects, shared by autoplay policy and renderer.
//! Placement is expressed in the source chart-space; native projects it onto its centered canvas.
pub(crate) mod history;
use crate::{Error, receptors};
pub use history::Update;
use serde_json::{Map, Value};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preferences {
    pub hide_arrows: bool,
    pub light_holds: bool,
    pub animation: receptors::Preferences,
    pub placement: Placement,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            hide_arrows: true,
            light_holds: true,
            animation: Default::default(),
            placement: Default::default(),
        }
    }
}
impl Preferences {
    pub fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut p = Self::default();
        for (name, target) in [
            ("AutoPlayHideArrows", &mut p.hide_arrows),
            ("AutoPlayLightHolds", &mut p.light_holds),
            ("AutoPlayRimEffect", &mut p.animation.autoplay_rim),
            ("AutoPlayGlowEffect", &mut p.animation.autoplay_glow),
            ("AutoPlayShrinkEffect", &mut p.animation.autoplay_shrink),
            ("TapRimEffect", &mut p.animation.tap_rim),
            ("TapShrinkEffect", &mut p.animation.tap_shrink),
            ("PulseReceptorsWithTempo", &mut p.animation.pulse),
        ] {
            if let Some(value) = fields.get(name) {
                *target = value.as_bool().ok_or_else(|| {
                    Error::InvalidFormat(format!("Invalid receptor preference {name}"))
                })?;
            }
        }
        p.placement = Placement::read(fields)?;
        Ok(p)
    }
    pub fn write(self, fields: &mut Map<String, Value>) {
        self.placement.write(fields);
        for (name, value) in [
            ("AutoPlayHideArrows", self.hide_arrows),
            ("AutoPlayLightHolds", self.light_holds),
            ("AutoPlayRimEffect", self.animation.autoplay_rim),
            ("AutoPlayGlowEffect", self.animation.autoplay_glow),
            ("AutoPlayShrinkEffect", self.animation.autoplay_shrink),
            ("TapRimEffect", self.animation.tap_rim),
            ("TapShrinkEffect", self.animation.tap_shrink),
            ("PulseReceptorsWithTempo", self.animation.pulse),
        ] {
            fields.insert(name.into(), Value::Bool(value));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub center: bool,
    pub lock_x: bool,
    pub lock_y: bool,
    pub allow_header: bool,
}
impl Default for Placement {
    fn default() -> Self {
        Self {
            x: 842,
            y: 110,
            center: false,
            lock_x: false,
            lock_y: false,
            allow_header: true,
        }
    }
}
impl Placement {
    fn read(fields: &Map<String, Value>) -> Result<Self, Error> {
        let mut p = Self::default();
        for (key, target) in [("PositionX", &mut p.x), ("PositionY", &mut p.y)] {
            if let Some(value) = fields.get(key) {
                *target = value
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(|| {
                        Error::InvalidFormat(format!("Invalid receptor preference {key}"))
                    })?;
            }
        }
        for (key, target) in [
            ("CenterHorizontally", &mut p.center),
            ("LockPositionX", &mut p.lock_x),
            ("LockPositionY", &mut p.lock_y),
            ("AllowChartMoveWhenPositionLocked", &mut p.allow_header),
        ] {
            if let Some(value) = fields.get(key) {
                *target = value.as_bool().ok_or_else(|| {
                    Error::InvalidFormat(format!("Invalid receptor preference {key}"))
                })?;
            }
        }
        Ok(p)
    }
    fn write(self, fields: &mut Map<String, Value>) {
        for (key, value) in [("PositionX", self.x), ("PositionY", self.y)] {
            fields.insert(key.into(), value.into());
        }
        for (key, value) in [
            ("CenterHorizontally", self.center),
            ("LockPositionX", self.lock_x),
            ("LockPositionY", self.lock_y),
            ("AllowChartMoveWhenPositionLocked", self.allow_header),
        ] {
            fields.insert(key.into(), value.into());
        }
    }
    /// Adapt source dock-space origin (842,110) to the native centered chart and
    /// its 55-unit receptor inset. Raw coordinates never change during projection.
    /// Reverse reflects the same unreflected Y distance within the chart viewport.
    pub fn project(
        self,
        center: f32,
        left: f32,
        right: f32,
        top: f32,
        bottom: f32,
        reverse: bool,
    ) -> (f32, f32) {
        let x = if self.center {
            center
        } else {
            center + (f64::from(self.x) - 842.) as f32
        }
        .clamp(left, right);
        let distance = (55. + (f64::from(self.y) - 110.) as f32).clamp(0., (bottom - top).max(0.));
        (
            x,
            if reverse {
                bottom - distance
            } else {
                top + distance
            },
        )
    }
    /// A two-dimensional receptor move, with the source movement permissions
    /// and dominant-axis held query. `requested` is the unconstrained cursor
    /// position, not the previous constrained result. Ties choose vertical.
    pub fn moved_to(self, requested: (i32, i32), axis_lock: bool, header: bool) -> Self {
        let can_x = !self.center && (!self.lock_x || header && self.allow_header);
        let can_y = !self.lock_y && !header;
        let (x, y) = (requested.0.max(0), requested.1.max(0));
        let (x, y) = if !can_x {
            (self.x, y)
        } else if !can_y {
            (x, self.y)
        } else if axis_lock {
            let dx = (i64::from(x) - i64::from(self.x)).abs();
            let dy = (i64::from(y) - i64::from(self.y)).abs();
            if dx > dy { (x, self.y) } else { (self.x, y) }
        } else {
            (x, y)
        };
        Self {
            x: if can_x { x } else { self.x },
            y: if can_y { y } else { self.y },
            ..self
        }
    }
    pub fn move_x(&mut self, delta: i32, header: bool) {
        if self.center || self.lock_x && (!header || !self.allow_header) {
            return;
        }
        self.x = self.x.saturating_add(delta).max(0);
    }
    pub fn move_y(&mut self, delta: i32) {
        if !self.lock_y {
            self.y = self.y.saturating_add(delta).max(0);
        }
    }
}

#[cfg(test)]
mod placement_tests {
    use super::*;
    #[test]
    fn actual_source_focal_and_lock_permissions_match_explicit_native_canvas_transform() {
        let cases: Value = serde_json::from_str(include_str!(
            "../tests/fixtures/receptor-placement/source.expected.json"
        ))
        .unwrap();
        assert_eq!(cases.as_array().unwrap().len(), 576);
        for row in cases.as_array().unwrap() {
            let bits = row["bits"].as_i64().unwrap();
            let mut p = Placement {
                x: row["x"].as_i64().unwrap() as i32,
                y: row["y"].as_i64().unwrap() as i32,
                center: bits & 1 != 0,
                lock_x: bits & 2 != 0,
                lock_y: bits & 4 != 0,
                allow_header: bits & 8 != 0,
            };
            let reverse = bits & 16 != 0;
            let actual = p.project(427., 0., 854., 105., 395., reverse);
            let sx = row["sourceX"].as_f64().unwrap();
            let sy = row["sourceY"].as_f64().unwrap();
            let unreflected = if reverse { 900. - sy } else { sy };
            let expected_x = if p.center {
                427.
            } else {
                (427. + sx - 842.).clamp(0., 854.)
            };
            let distance = (55. + unreflected - 110.).clamp(0., 290.);
            assert_eq!(
                actual,
                (
                    expected_x as f32,
                    if reverse {
                        395. - distance as f32
                    } else {
                        105. + distance as f32
                    }
                )
            );
            let header = row["header"].as_bool().unwrap();
            let old = p;
            p.move_x(1, header);
            assert_eq!(
                p.x,
                old.x
                    + if row["moveX"].as_bool().unwrap() {
                        1
                    } else {
                        0
                    }
            );
            if !header {
                p.move_y(1);
                assert_eq!(
                    p.y,
                    old.y
                        + if row["moveY"].as_bool().unwrap() {
                            1
                        } else {
                            0
                        }
                );
            }
        }
    }
    #[test]
    fn raw_position_survives_clamped_projection_and_preferences_roundtrip() {
        let mut fields =
            serde_json::json!({"PositionX":2147483647,"PositionY":-2147483648,"Vendor":7})
                .as_object()
                .unwrap()
                .clone();
        let p = Preferences::read(&fields).unwrap();
        assert_eq!(
            p.placement.project(427., 0., 854., 105., 395., false),
            (854., 105.)
        );
        p.write(&mut fields);
        assert_eq!(fields["PositionX"], i32::MAX);
        assert_eq!(fields["PositionY"], i32::MIN);
        assert_eq!(fields["Vendor"], 7);
        assert_eq!(Preferences::read(&fields).unwrap(), p);
    }
}
